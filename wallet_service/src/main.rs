use std::convert::Infallible;
use std::env;
use std::error::Error;

use adm_provider::util::parse_address;
use adm_sdk::{account::Account, network::Network};
use adm_signer::{key::parse_secret_key, AccountKind, Wallet};
use dotenv::dotenv;
use ethers::prelude::TransactionReceipt;
use fvm_shared::{address::Address, econ::TokenAmount};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use warp::{http::StatusCode, Filter, Rejection, Reply};

#[tokio::main]
async fn main() {
    dotenv().ok();

    let admin_wallet_pk = env::var("WALLET_SERVICE_TESTNET_PRIVATE_KEY")
        .expect("WALLET_SERVICE_TESTNET_PRIVATE_KEY not set");

    let port = env::var("WALLET_SERVICE_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(8081);

    let register = warp::post()
        .and(warp::path("register"))
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_private_key(admin_wallet_pk.clone()))
        .and_then(handle_register);

    let router = register
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .recover(handle_rejection);

    warp::serve(router).run(([127, 0, 0, 1], port)).await;
}

#[derive(Deserialize)]
struct RegisterRequest {
    network: Network,
    #[serde(deserialize_with = "deserialize_address")]
    address: Address,
}

/// Custom deserializer to allow for FVM or EVM addresses to be used as input.
fn deserialize_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;
    parse_address(s.as_str()).map_err(serde::de::Error::custom)
}

/// Generic request error.
#[derive(Clone, Debug)]
struct BadRequest {
    message: String,
}

impl warp::reject::Reject for BadRequest {}

/// Custom error message with status code.
#[derive(Clone, Debug, Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

/// Rejection handler.
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<BadRequest>() {
        let err = e.to_owned();
        (StatusCode::BAD_REQUEST, err.message)
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err))
    };

    let reply = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message,
    });
    Ok(warp::reply::with_status(reply, code))
}

/// Filter to pass the private key to the request handler.
fn with_private_key(
    private_key: String,
) -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || private_key.clone())
}

/// Get the admin wallet that will send transactions on the subnet.
fn get_admin_wallet(private_key: &str, network: Network) -> anyhow::Result<Wallet> {
    let pk = parse_secret_key(&private_key)?;
    let signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?)?;
    Ok(signer)
}

/// Handles the `/register` request, first initializing the network.
async fn handle_register(
    req: RegisterRequest,
    private_key: String,
) -> Result<impl Reply, Rejection> {
    req.network.init();
    let res = register(req.address, req.network, &private_key)
        .await
        .map_err(|e| {
            Rejection::from(BadRequest {
                message: format!("register error: {}", e.to_string()),
            })
        })?;
    let json = json!(res);
    Ok(warp::reply::json(&json))
}

/// Registers an account on the subnet, creating the delegated EVM address (by
/// transferring 0 FIL).
async fn register(
    address: Address,
    network: Network,
    private_key: &str,
) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
    let signer = get_admin_wallet(private_key, network)?;
    let config = network.subnet_config(Default::default())?;
    let amount = TokenAmount::from_whole(0);
    let tx = Account::transfer(&signer, address, config, amount).await?;
    Ok(tx)
}
