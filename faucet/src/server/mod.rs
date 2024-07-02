use warp::Filter;

use crate::Cli;

pub mod routes;

pub async fn handle_server(cli: Cli) -> anyhow::Result<()> {
    let faucet_pk = cli.faucet_private_key;
    let port = cli.faucet_port.unwrap_or_default();

    let router = routes::register::register_route(faucet_pk)
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .recover(routes::handle_rejection);

    warp::serve(router).run(([127, 0, 0, 1], port)).await;
    Ok(())
}
