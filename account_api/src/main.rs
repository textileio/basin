use std::env;
use std::error::Error;

use adm_provider::util::parse_address;
use adm_sdk::{account::Account, network::Network};
use adm_signer::{key::parse_secret_key, AccountKind, Wallet};
use dotenv::dotenv;
use ethers::prelude::TransactionReceipt;
use ethers::utils::hex;
use fvm_shared::econ::TokenAmount;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Filter, Rejection, Reply};

/// Request for registering or funding an account.
#[derive(Deserialize)]
struct AccountRequest {
    /// Hex-prefixed public key (EVM-style address).
    address: String,
}

/// Response for registering or funding an account.
#[derive(Serialize)]
struct AccountResponse {
    /// Status of the request.
    status: String,
    /// Message for the request.
    message: String,
    /// Transaction receipt for the request.
    tx: Option<TransactionReceipt>,
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

#[tokio::main]
async fn main() {
    dotenv().ok();

    let state = Arc::new(Mutex::new(State::new()));

    let register = warp::post()
        .and(warp::path!(String / "register"))
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .and_then(handle_register);

    let fund = warp::post()
        .and(warp::path!(String / "fund"))
        .and(with_state(state.clone()))
        .and(warp::body::json())
        .and_then(handle_fund);

    let routes = register.or(fund).recover(handle_reject);

    warp::serve(routes).run(([127, 0, 0, 1], 8081)).await;
}

/// Get the network configuration based on the API path parameter.
fn get_network(network: &str) -> Result<&'static Network, &'static str> {
    return match network {
        "testnet" => Ok(Network::Testnet.init()),
        // TODO: if a parent-child subnet setup is possible, then localnet or
        // devnet can also benefit from this API. Also, future mainnet, too.
        // "mainnet" => Ok(Network::Mainnet.init()),
        // "localnet" => Network::Localnet.init(),
        // "devnet" => Network::Localnet.init(),
        _ => return Err("Invalid network"),
    };
}

/// Get the admin wallet for sending transactions to register or fund accounts.
fn get_admin_wallet(network: &Network) -> anyhow::Result<Wallet> {
    let env_pk = match env::var("ACCOUNT_API_PRIVATE_KEY") {
        Ok(pk) => pk,
        Err(_) => return Err(anyhow::anyhow!("ACCOUNT_API_PRIVATE_KEY not set")),
    };
    let pk = parse_secret_key(&env_pk)?;
    let signer = Wallet::new_secp256k1(pk, AccountKind::Ethereum, network.subnet_id()?.parent()?)?;
    Ok(signer)
}

/// Filter to pass the state to the request handlers
#[allow(unused_variables)]
fn with_state(
    state: Arc<Mutex<State>>,
) -> impl Filter<Extract = (Arc<Mutex<State>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// Handles the `/<network>/register` request.
// TODO: this creates an account on the rootnet (Filecoin) but not the subnet...
// so is it worthwhile keeping, or should we just have the `fund` endpoint?
#[allow(unused_variables)]
async fn handle_register(
    network: String,
    state: Arc<Mutex<State>>,
    req: AccountRequest,
) -> Result<impl Reply, Rejection> {
    match register(&req.address, &network).await {
        Ok(tx) => Ok(warp::reply::json(&AccountResponse {
            status: "success".to_string(),
            message: "Account registered on network".to_string(),
            tx: Some(tx),
        })),
        Err(e) => Ok(warp::reply::json(&AccountResponse {
            status: "error".to_string(),
            message: e.to_string(),
            tx: None,
        })),
    }
}

/// Registers an account on Filecoin rootnet (via `InvokeEVM`), creating an
/// f410 address for the EVM address by sending 0 FIL to it.
async fn register(
    address: &str,
    network: &str,
) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
    // TODO: only `testnet` is valid, but this allows for future path params like
    // `mainnet`, `devnet`, or `localnet`
    let net = get_network(network)?;
    let admin = get_admin_wallet(net)?;

    let tx = Account::transfer(
        &admin,
        parse_address(address)?,
        net.parent_subnet_config(Default::default())?,
        TokenAmount::from_whole(0),
    )
    .await?;

    Ok(tx)
}

/// Handles the `/<network>/fund` request.
#[allow(unused_variables)]
async fn handle_fund(
    network: String,
    state: Arc<Mutex<State>>,
    req: AccountRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    match fund(&req.address, &network).await {
        Ok(tx) => Ok(warp::reply::json(&AccountResponse {
            status: "success".to_string(),
            message: "Account funded successfully".to_string(),
            tx: Some(tx),
        })),
        Err(e) => Ok(warp::reply::json(&AccountResponse {
            status: "error".to_string(),
            message: e.to_string(),
            tx: None,
        })),
    }
}

/// Funds an account on the subnet (sends 1 FIL), thus, initializing it on both
/// the rootnet and subnet.
async fn fund(address: &str, network: &str) -> anyhow::Result<TransactionReceipt, Box<dyn Error>> {
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

    println!("Deposited 1 tFIL to {}", address);
    println!(
        "Transaction hash: 0x{}",
        hex::encode(tx.transaction_hash.to_fixed_bytes())
    );

    Ok(tx)
}

/// Custom rejection handler for the API.
async fn handle_reject(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.is_not_found() {
        Ok(warp::reply::with_status(
            "404 not found",
            warp::http::StatusCode::NOT_FOUND,
        ))
    } else {
        Ok(warp::reply::with_status(
            "Internal server error",
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
