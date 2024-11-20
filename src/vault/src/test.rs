use candid::{encode_one, Principal};
use pocket_ic::PocketIc;

use std::fs;

const _BACKEND_WASM: &str = "../../target/wasm32-unknown-unknown/release/store.wasm";

fn _setup() -> (PocketIc, Principal) {
    let pic = PocketIc::new();

    let perp_canister = pic.create_canister_with_settings(Some(Principal::anonymous()), None);
    pic.add_cycles(perp_canister, 2_000_000_000_000); // 2T Cycles
    let wasm = fs::read(_BACKEND_WASM).expect("Wasm file not found, run 'dfx build'.");
    pic.install_canister(
        perp_canister,
        wasm,
        encode_one(Principal::anonymous()).unwrap(),
        None,
    );
    return (pic, perp_canister);
}

#[test]
fn testing() {
    let (pic, cansiter) = _setup();
}
