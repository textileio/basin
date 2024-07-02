use clap::Parser;
use fendermint_crypto::SecretKey;
use stderrlog::Timestamp;

use adm_signer::key::parse_secret_key;

use crate::server::handle_server;

mod server;

#[derive(Clone, Debug, Parser)]
#[command(name = "adm_faucet", author, version, about, long_about = None)]
struct Cli {
    /// Wallet private key (ECDSA, secp256k1) for sending faucet funds.
    #[arg(short, long, env, value_parser = parse_secret_key)]
    faucet_private_key: SecretKey,
    /// Faucet HTTP server port.
    #[arg(long, env, default_value("8081"))]
    faucet_port: Option<u16>,
    /// Logging verbosity (repeat for more verbose logging).
    #[arg(short, long, env, action = clap::ArgAction::Count)]
    verbosity: u8,
    /// Silence logging.
    #[arg(short, long, env, default_value_t = false)]
    quiet: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .quiet(cli.quiet)
        .verbosity(cli.verbosity as usize)
        .timestamp(Timestamp::Millisecond)
        .init()
        .unwrap();

    handle_server(cli).await
}
