//! One prompt on screen at a time. A connection asking the same question (same kind, browser
//! and origin) as the open prompt shares its answer, so an auto-reconnecting extension cannot
//! stack dialogs; a different question waits for the open one to close.
//!
//! Every prompt gets its own timer task, so a prompt whose requester disconnected still closes
//! and cannot block the queue forever.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::time::Instant;

use super::{Decision, GatewayHost, PromptEvent, PromptKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PromptOutcome {
    Allowed,
    Denied,
    Failed(String),
    TimedOut,
}

impl From<Decision> for PromptOutcome {
    fn from(decision: Decision) -> Self {
        match decision {
            Decision::Allow => Self::Allowed,
            Decision::Deny => Self::Denied,
            Decision::Failed(reason) => Self::Failed(reason),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct PromptKey {
    kind: PromptKind,
    identifier: Option<String>,
    origin: Option<String>,
}

struct Pending {
    event: PromptEvent,
    key: PromptKey,
    outcome: watch::Sender<Option<PromptOutcome>>,
}

struct Inner {
    current: Mutex<Option<Pending>>,
    /// Bumped whenever the open prompt closes, waking connections queued behind it.
    closed: watch::Sender<u64>,
}

impl Default for Inner {
    fn default() -> Self {
        Self { current: Mutex::new(None), closed: watch::channel(0).0 }
    }
}

#[derive(Clone, Default)]
pub(super) struct PromptBroker {
    inner: Arc<Inner>,
}

enum Step {
    Join(watch::Receiver<Option<PromptOutcome>>),
    Open(watch::Receiver<Option<PromptOutcome>>, PromptEvent),
    Wait,
}

impl PromptBroker {
    pub(super) async fn ask<H: GatewayHost>(
        &self,
        host: &H,
        kind: PromptKind,
        identifier: Option<String>,
        origin: Option<String>,
        timeout: Duration,
    ) -> PromptOutcome {
        let deadline = Instant::now() + timeout;
        let key = PromptKey { kind, identifier, origin };
        let mut closed = self.inner.closed.subscribe();

        loop {
            match self.step(&key) {
                Step::Join(rx) => return wait_for_outcome(rx, deadline).await,
                Step::Open(rx, event) => {
                    self.spawn_timer(event.request_id.clone(), timeout);
                    host.show_prompt(&event);
                    return wait_for_outcome(rx, deadline).await;
                }
                Step::Wait => {
                    match tokio::time::timeout_at(deadline, closed.changed()).await {
                        Ok(Ok(())) => continue,
                        _ => return PromptOutcome::TimedOut,
                    }
                }
            }
        }
    }

    pub(super) fn resolve(&self, request_id: &str, decision: Decision) -> bool {
        self.finish(request_id, decision.into())
    }

    /// The prompt on screen, for a UI that started listening after it was raised.
    pub(super) fn pending(&self) -> Option<PromptEvent> {
        let current = self.inner.current.lock().ok()?;
        current.as_ref().map(|pending| pending.event.clone())
    }

    fn step(&self, key: &PromptKey) -> Step {
        let Ok(mut current) = self.inner.current.lock() else {
            return Step::Wait;
        };
        match current.as_ref() {
            Some(pending) if pending.key == *key => Step::Join(pending.outcome.subscribe()),
            Some(_) => Step::Wait,
            None => {
                let (outcome, rx) = watch::channel(None);
                let event = PromptEvent {
                    request_id: uuid::Uuid::new_v4().to_string(),
                    kind: key.kind,
                    identifier: key.identifier.clone(),
                    origin: key.origin.clone(),
                };
                *current = Some(Pending {
                    event: event.clone(),
                    key: key.clone(),
                    outcome,
                });
                Step::Open(rx, event)
            }
        }
    }

    fn finish(&self, request_id: &str, outcome: PromptOutcome) -> bool {
        let finished = {
            let Ok(mut current) = self.inner.current.lock() else {
                return false;
            };
            match current.take() {
                Some(pending) if pending.event.request_id == request_id => {
                    pending.outcome.send_replace(Some(outcome));
                    true
                }
                other => {
                    *current = other;
                    false
                }
            }
        };
        if finished {
            self.inner.closed.send_modify(|generation| *generation = generation.wrapping_add(1));
        }
        finished
    }

    fn spawn_timer(&self, request_id: String, timeout: Duration) {
        let broker = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            broker.finish(&request_id, PromptOutcome::TimedOut);
        });
    }
}

async fn wait_for_outcome(mut rx: watch::Receiver<Option<PromptOutcome>>, deadline: Instant) -> PromptOutcome {
    loop {
        if let Some(outcome) = rx.borrow_and_update().clone() {
            return outcome;
        }
        match tokio::time::timeout_at(deadline, rx.changed()).await {
            Ok(Ok(())) => continue,
            _ => return PromptOutcome::TimedOut,
        }
    }
}
