use futures_util::{Sink, SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_util::sync::CancellationToken;

pub(super) fn json_frame(value: Value) -> Message {
    Message::text(value.to_string())
}

pub(super) fn service_stopped_frame() -> Message {
    json_frame(json!({"type": "disconnected", "code": "service_stopped"}))
}

/// Pipes data frames both ways until either side goes away. When the daemon side ends, the
/// extension is told why before its socket closes, so it reconnects instead of showing an error.
pub(super) async fn run<CT, CR, UT, UR>(
    client_tx: &mut CT,
    client_rx: &mut CR,
    up_tx: &mut UT,
    up_rx: &mut UR,
    cancel: &CancellationToken,
) where
    CT: Sink<Message, Error = WsError> + Unpin,
    CR: Stream<Item = Result<Message, WsError>> + Unpin,
    UT: Sink<Message, Error = WsError> + Unpin,
    UR: Stream<Item = Result<Message, WsError>> + Unpin,
{
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = client_tx.send(Message::Close(None)).await;
                let _ = up_tx.send(Message::Close(None)).await;
                return;
            }
            msg = client_rx.next() => match msg {
                Some(Ok(msg @ (Message::Text(_) | Message::Binary(_)))) => {
                    if up_tx.send(msg).await.is_err() {
                        let _ = client_tx.send(service_stopped_frame()).await;
                        let _ = client_tx.send(Message::Close(None)).await;
                        return;
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                    let _ = up_tx.send(Message::Close(None)).await;
                    return;
                }
                Some(Ok(_)) => {}
            },
            msg = up_rx.next() => match msg {
                Some(Ok(msg @ (Message::Text(_) | Message::Binary(_)))) => {
                    if client_tx.send(msg).await.is_err() {
                        let _ = up_tx.send(Message::Close(None)).await;
                        return;
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                    let _ = client_tx.send(service_stopped_frame()).await;
                    let _ = client_tx.send(Message::Close(None)).await;
                    return;
                }
                Some(Ok(_)) => {}
            },
        }
    }
}
