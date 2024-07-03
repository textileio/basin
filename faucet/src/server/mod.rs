use log::{error, info, Level};
use warp::{log::Info, Filter};

use crate::Cli;

pub mod routes;

/// Server entrypoint for the faucet service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let faucet_pk = cli.faucet_private_key;
    let port = cli.faucet_port.unwrap_or_default();

    let register_route = routes::register::register_route(faucet_pk.clone());

    let log_request_details = warp::log::custom(log_request_details);

    let router = register_route
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .with(log_request_details)
        .recover(routes::handle_rejection);

    warp::serve(router).run(([127, 0, 0, 1], port)).await;
    Ok(())
}

/// Helper function to log details for each request at specific verbosity levels
/// ([Level::Info] or [Level::Error]).
fn log_request_details(request: Info) {
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
fn log_request_body(route: &str, body: &str) {
    info!("incoming /{} request: {}", route, body);
}
