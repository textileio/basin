use wasm_bindgen::prelude::*;

use adm_sdk::network::Network;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn initialize_network() {
    Network::Testnet.init();
    alert("initialized network");
}
