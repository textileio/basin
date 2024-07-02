use std::convert::Infallible;
use std::error::Error;

use ethers::prelude::TransactionReceipt;
use fendermint_crypto::SecretKey;
use fvm_shared::{address::Address, econ::TokenAmount};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use adm_provider::util::parse_address;
use adm_sdk::{account::Account, network::Network as SdkNetwork};
use adm_signer::{AccountKind, Wallet};

use crate::Cli;

pub async fn handle_server(cli: Cli) -> anyhow::Result<()> {
    let faucet_pk = cli.faucet_private_key;
    let port = cli.faucet_port.unwrap_or_default();

    let register = warp::post()
        .and(warp::path("register"))
        .and(warp::header::exact("content-type", "application/json"))
        .and(warp::body::json())
        .and(with_private_key(faucet_pk.clone()))
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
    Ok(())
}

#[derive(Deserialize)]
struct RegisterRequest {
    network: SdkNetwork,
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
    private_key: SecretKey,
) -> impl Filter<Extract = (SecretKey,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || private_key.clone())
}

/// Get the faucet wallet that will send transactions on the subnet.
fn get_faucet_wallet(private_key: SecretKey, network: SdkNetwork) -> anyhow::Result<Wallet> {
    let signer = Wallet::new_secp256k1(private_key, AccountKind::Ethereum, network.subnet_id()?)?;
    Ok(signer)
}

/// Handles the `/register` request, first initializing the network.
async fn handle_register(
    req: RegisterRequest,
    private_key: SecretKey,
) -> Result<impl Reply, Rejection> {
    req.network.init();
    let res = register(req.address, req.network, private_key)
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
    network: SdkNetwork,
    private_key: SecretKey,
) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
    let signer = get_faucet_wallet(private_key, network)?;
    let config = network.subnet_config(Default::default())?;
    let amount = TokenAmount::from_whole(0);
    let tx = Account::transfer(&signer, address, config, amount).await?;
    Ok(tx)
}
