use std::convert::Infallible;
use std::env;
use std::error::Error;

use adm_provider::util::parse_address;
use adm_sdk::{account::Account, network::Network};
use adm_signer::{key::parse_secret_key, AccountKind, Wallet};
use dotenv::dotenv;
use ethers::prelude::TransactionReceipt;
use fvm_shared::econ::TokenAmount;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{http::StatusCode, Filter, Rejection, Reply};

#[tokio::main]
async fn main() {
    dotenv().ok();

    let state = Arc::new(Mutex::new(State::new()));

    let register = warp::post()
        .and(warp::path!(Network / "register" / String))
        .and(with_state(state.clone()))
        .and_then(handle_register);
    let fund = warp::post()
        .and(warp::path!(Network / "fund" / String))
        .and(with_state(state.clone()))
        .and_then(handle_fund);

    let router = register
        .or(fund)
        .with(
            warp::cors()
                .allow_any_origin()
                .allow_headers(vec!["Content-Type"])
                .allow_methods(vec!["POST"]),
        )
        .recover(handle_rejection);

    warp::serve(router).run(([127, 0, 0, 1], 8081)).await;
}

/// State for the API
struct State {
    // TODO: rate limit data?
}

impl State {
    /// Create new state.
    pub fn new() -> Self {
        State {}
    }
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

/// Rejection handler for the API.
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

/// Get the network configuration based on the API path parameter.
fn get_network(network: &Network) -> Result<&'static Network, &'static str> {
    return match network {
        Network::Testnet => Ok(Network::Testnet.init()),
        // TODO: if a parent-child subnet setup is possible, then localnet or
        // devnet can also benefit from this API. Also, future mainnet, too.
        // Network::Mainnet => Ok(Network::Mainnet.init()),
        // Network::Localnet => Ok(Network::Localnet.init()),
        // Network::Devnet => Ok(Network::Devnet.init()),
        _ => return Err("Invalid network"),
    };
}

/// Get the admin wallet for sending transactions to register or fund accounts.
fn get_admin_wallet(network: &Network) -> anyhow::Result<Wallet> {
    let env_pk = match env::var("ADMIN_TESTNET_PRIVATE_KEY") {
        Ok(pk) => pk,
        Err(_) => return Err(anyhow::anyhow!("ADMIN_TESTNET_PRIVATE_KEY not set")),
    };
    let pk = parse_secret_key(&env_pk)?;
    let signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?.parent()?)?;
    Ok(signer)
}

/// Filter to pass the state to the request handlers.
#[allow(unused_variables)]
fn with_state(
    state: Arc<Mutex<State>>,
) -> impl Filter<Extract = (Arc<Mutex<State>>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Handles the `/<network>/register/<address>` request.
// TODO: this creates an account on the rootnet (Filecoin) but not the subnet...
// so is it worthwhile keeping, or should we just have the `fund` endpoint?
#[allow(unused_variables)]
async fn handle_register(
    network: Network,
    address: String,
    state: Arc<Mutex<State>>,
) -> Result<impl Reply, Rejection> {
    let res = register(&address, &network).await.map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("register error: {}", e),
        })
    })?;

    let json = json!({"tx": res});
    Ok(warp::reply::json(&json))
}

/// Registers an account on Filecoin rootnet (via `InvokeEVM`), creating an
/// delegate address for the EVM address by sending 0 FIL to it.
async fn register(
    address: &str,
    network: &Network,
) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
    // TODO: only `testnet` is valid, but this allows for future path params like
    // `mainnet`, `devnet`, or `localnet`
    let net = get_network(network)?;
    let admin = get_admin_wallet(net)?;

    match Account::transfer(
        &admin,
        parse_address(address)?,
        net.parent_subnet_config(Default::default())?,
        TokenAmount::from_whole(0),
    )
    .await
    {
        Ok(tx) => Ok(tx),
        // TODO: although `InvokeEVM` is called to create the delegate address,
        // this method never returns a receipt, so we catch the error here. Is
        // this because it's doing a transfer of 0 FIL, which is not a valid
        // transfer? Or, perhaps it's the subnet transfer that's invalid?
        Err(e) if e.to_string() == "transfer did not return receipt" => {
            Ok(TransactionReceipt::default())
        }
        Err(e) => Err(e.into()),
    }
}

/// Handles the `/<network>/fund/<address>` request.
#[allow(unused_variables)]
async fn handle_fund(
    network: Network,
    address: String,
    state: Arc<Mutex<State>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let res = fund(&address, &network).await.map_err(|e| {
        Rejection::from(BadRequest {
            message: format!("fund error: {}", e),
        })
    })?;

    let json = json!({"tx": res});
    Ok(warp::reply::json(&json))
}

/// Funds an account on the subnet (sends 1 FIL), thus, initializing it on both
/// the rootnet and subnet.
async fn fund(
    address: &str,
    network: &Network,
) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
    // TODO: only `testnet` is valid, but this allows for future path params like
    // `mainnet`, `devnet`, or `localnet`
    let net = get_network(network)?;
    let admin = get_admin_wallet(net)?;
    // Send 0 FIL to the new account to initialize it
    let tx = Account::deposit(
        &admin,
        parse_address(address)?,
        net.parent_subnet_config(Default::default())?,
        TokenAmount::from_whole(1),
    )
    .await?;

    Ok(tx)
}
