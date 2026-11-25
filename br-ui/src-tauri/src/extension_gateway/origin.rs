/// Browsers always send `Origin` on page-initiated WebSockets, so requiring an extension scheme
/// when it is present keeps websites out. A missing `Origin` only comes from local non-browser
/// processes, which already run as the user.
pub fn is_allowed_origin(origin: Option<&str>) -> bool {
    match origin {
        None => true,
        Some(origin) => origin.starts_with("chrome-extension://") || origin.starts_with("moz-extension://"),
    }
}
