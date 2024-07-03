use warp::Filter;

use crate::Cli;

use util::log_request_details;

pub mod register;
pub mod shared;
pub mod util;

/// Server entrypoint for the faucet service.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let faucet_pk = cli.faucet_private_key;
    let port = cli.faucet_port.unwrap_or_default();

    let register_route = register::register_route(faucet_pk.clone());

    let log_request_details = warp::log::custom(log_request_details);

    let router = register_route
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .with(log_request_details)
        .recover(shared::handle_rejection);

    warp::serve(router).run(([127, 0, 0, 1], port)).await;
    Ok(())
}
