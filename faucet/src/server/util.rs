use log::{error, info, Level};
use warp::log::Info;

/// Helper function to log details for each request at specific verbosity levels
/// ([Level::Info] or [Level::Error]).
pub fn log_request_details(request: Info) {
    let level = if request.status().as_u16() >= 500 {
        Level::Error
    } else {
        Level::Info
    };
    let addr = request
        .remote_addr()
        .unwrap_or_else(|| ([0, 0, 0, 0], 0).into());
    let duration = request.elapsed().as_millis();
    let err = format!(
        "{} {} {} - {} - {}ms",
        request.method().as_str(),
        request.path(),
        request.status(),
        addr,
        duration
    );
    match level {
        Level::Error => error!("{}", err),
        Level::Info => info!("{}", err),
        // Only Error & Info are used (Trace, Debug, Warn also possible)
        _ => {}
    }
}

/// Helper function to log the incoming request body for a route when
/// [`Level::Info`] logging is enabled.
pub fn log_request_body(route: &str, body: &str) {
    info!("incoming /{} request: {}", route, body);
}
