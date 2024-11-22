use candid::{decode_args, decode_one, encode_args, encode_one, types::principal, Principal};
use pocket_ic::{PocketIc, UserError, WasmResult};

use std::{cell::RefCell, time::Duration};

use std::fs;

use crate::close_position;
use crate::{
    //  corelib::order_lib::LimitOrder,
    types::{Asset, AssetClass, MarketDetails, StateDetails, Tick},
    Amount, // OrderType, PositionDetails,
    OrderType,
    PositionDetails,
};

type Subaccount = [u8; 32];

const _BACKEND_WASM: &str = "../../target/wasm32-unknown-unknown/release/perp.wasm";

thread_local! {
    static CANISTER_ID:RefCell<Principal> = RefCell::new(Principal::anonymous())
}

#[test]
fn test_setting_state() {
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let init_state_details = _get_state(&pic);

    assert_eq!(init_state_details.not_paused, false);

    let init_tick = 100000 * 199;

    _set_state(&pic, admin, 100000 * 199, 20, 10000000000);

    let new_state_details = _get_state(&pic);
    assert_eq!(new_state_details.current_tick, init_tick);
    assert_eq!(new_state_details.not_paused, true);
}

#[test]
fn test_open_position_failing() {
    // Reason
    // Max Leverage Exceeded
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let init_tick = 100000 * 199;

    _set_state(&pic, admin, init_tick, 15, 0);

    let caller = _get_principals()[1];

    let result = _open_position(
        &pic,
        caller,
        1_000_000_000,
        false,
        OrderType::Limit,
        20, // leverage exceeds
        Some(100000 * 200),
    );

    if let Err(reason) = result {
        assert_eq!(
            reason,
            "Max leverage exceeded or collateral is too small".to_string()
        );
    };
}

#[test]
fn test_open_position_limit_order_type_short() {
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let caller = _get_principals()[0];

    let init_tick = 100000 * 199;

    // assert position opens
    _set_state(&pic, admin, init_tick, 100, 0);

    let reference_tick_1 = 100000 * 200;

    let _ = _open_position(
        &pic,
        caller,
        1_000_000_000,
        false,
        OrderType::Limit,
        20,
        Some(100000 * 200),
    );

    let state_details = _get_state(&pic);

    assert_eq!(state_details.current_tick, reference_tick_1);

    // Opening a position by a second user

    let second_caller = _get_principals()[1]; //test caller

    let reference_tick_2 = 100000 * 201;

    let _ = _open_position(
        &pic,
        second_caller,
        1_000_000_000,
        false,
        OrderType::Limit,
        20,
        Some(reference_tick_2),
    );

    let state_details = _get_state(&pic);

    assert_ne!(state_details.current_tick, reference_tick_2);

    // Opening third position

    let third_caller = Principal::anonymous();

    let reference_tick3 = 100000 * 199;

    let _ = _open_position(
        &pic,
        third_caller,
        1_000_000_000,
        false,
        OrderType::Limit,
        20,
        Some(reference_tick3),
    );

    let state_details = _get_state(&pic);

    assert_eq!(state_details.current_tick, reference_tick3);
}

#[test]
fn _test_open_limit_order_long() {
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let caller = _get_principals()[0];

    let init_tick = 100000 * 199;

    _set_state(&pic, admin, init_tick, 100, 0);

    let reference_tick1 = 100000 * 198;

    let _ = _open_position(
        &pic,
        caller,
        1_000_000_000,
        true,
        OrderType::Limit,
        20,
        Some(reference_tick1),
    );

    let best_buy_offer = _get_best_offer(&pic, true);

    assert_eq!(
        reference_tick1, best_buy_offer,
        "testing that best buy offer is at reference tick1 ",
    );

    let caller2 = _get_principals()[1];

    let reference_tick2 = 100000 * 197;

    let _ = _open_position(
        &pic,
        caller2,
        1_000_000_000,
        true,
        OrderType::Limit,
        20,
        Some(reference_tick2),
    );

    let current_best_buy_offer = _get_best_offer(&pic, true);
    // best buy offer does not change cus the previous was higher
    assert_eq!(
        reference_tick1, current_best_buy_offer,
        "testing that best buy offer does not change"
    );
}

#[test]
fn test_open_market_order_long() {
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let caller = _get_principals()[0];

    let init_tick = 100000 * 199;

    _set_state(&pic, admin, init_tick, 100, 0);
    // open limit order
    let reference_tick_1 = 100000 * 200;

    let _ = _open_position(
        &pic,
        caller,
        1_000_000_000,
        false,
        OrderType::Limit,
        20,
        Some(reference_tick_1),
    );

    let StateDetails { current_tick, .. } = _get_state(&pic);

    assert_eq!(current_tick, reference_tick_1);

    let second_caller = _get_principals()[1];

    let result = _open_position(
        &pic,
        second_caller,
        1_000_000,
        true,
        OrderType::Market,
        20,
        Some(reference_tick_1),
    );

    if let Ok(_position) = result {
        let _ = _open_position(
            &pic,
            Principal::anonymous(),
            1_000_000_000,
            true,
            OrderType::Limit,
            20,
            Some(init_tick),
        );
        let collateral = _close_position(&pic, second_caller);

        println!("The pnl of this position is {}", collateral)
    }
}

#[test]
fn test_open_market_short() {
    let admin = _get_principals()[0];
    let pic = _setup_market(admin);

    let caller = _get_principals()[0];

    let init_tick = 100000 * 200;

    _set_state(&pic, admin, init_tick, 100, 0);

    // open limit order
    let reference_tick_1 = 19940_000;

    let _ = _open_position(
        &pic,
        caller,
        1_000_000_000_000,
        true,
        OrderType::Limit,
        20,
        Some(reference_tick_1),
    );

    let second_caller = _get_principals()[1];

    let result = _open_position(
        &pic,
        second_caller,
        1_000_000,
        false,
        OrderType::Market,
        20,
        Some(reference_tick_1),
    );

    println!("The position is {:?}", result)
}

///////////////////////////////////////////////////////////////////////
/// Position Function
///////////////////////////////////////////////////////////////////////
fn _open_position(
    pic: &PocketIc,
    principal: Principal,
    collateral: Amount,
    long: bool,
    order_type: OrderType,
    leverage: u8,
    max_tick: Option<Tick>,
) -> Result<PositionDetails, String> {
    let canister_id = _get_canister_id();

    let returns;

    match pic.update_call(
        canister_id,
        principal,
        "openPosition",
        encode_args((collateral, long, order_type, leverage, max_tick, 1u64, 1u64)).unwrap(),
    ) {
        Ok(reply) => {
            if let WasmResult::Reply(val) = reply {
                returns = val
            } else {
                panic!("returned error")
            }
        }
        Err(error) => {
            println!("{:?}", error);
            panic!("Could not open position")
        }
    }

    let reply = decode_one(&returns).unwrap();

    return reply;
}

fn _close_position(pic: &PocketIc, sender: Principal) -> u128 {
    let canister_id = _get_canister_id();

    let max_tick: Option<Tick> = Option::None;
    let Ok(WasmResult::Reply(res)) = pic.update_call(
        canister_id,
        sender,
        "closePosition",
        encode_one(max_tick).unwrap(),
    ) else {
        panic!("failed to close position")
    };

    decode_one(&res).unwrap()
}

////////////////////////////////////////////////////////////////////////////////////
/// Getters
/////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////
fn _get_state(pic: &PocketIc) -> StateDetails {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        Principal::anonymous(),
        "getStateDetails",
        encode_one(()).unwrap(),
    ) else {
        panic!("error occured")
    };

    decode_one(&val).unwrap()
}

///
fn _get_best_offer(pic: &PocketIc, buy: bool) -> Tick {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        Principal::anonymous(),
        "getBestOfferTick",
        encode_one(buy).unwrap(),
    ) else {
        panic!("best offer could not be found")
    };
    let reply = decode_one(&val).unwrap();

    return reply;
}

/// Get Position PNL
fn _get_pnl(pic: &PocketIc, position: PositionDetails) -> i64 {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        Principal::anonymous(),
        "getPositionPNL",
        encode_one(position).unwrap(),
    ) else {
        panic!("Account could not be gotten")
    };
    let reply = decode_one(&val).unwrap();

    return reply;
}

/// Get Position Status
fn _get_user_account(pic: &PocketIc, principal: Principal) -> Subaccount {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        principal,
        "getUserAccount",
        encode_one(principal).unwrap(),
    ) else {
        panic!("Account could not be gotten")
    };
    let reply = decode_one(&val).unwrap();

    return reply;
}

/// Get Account Position
fn _get_account_position(pic: &PocketIc, account: Subaccount) -> PositionDetails {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        Principal::anonymous(),
        "getAccountPosition",
        encode_one(account).unwrap(),
    ) else {
        panic!("Account could not be gotten")
    };
    let reply = decode_one(&val).unwrap();

    return reply;
}

fn _get_position_status(pic: &PocketIc, account: Subaccount) -> (bool, bool) {
    let canister_id = _get_canister_id();
    let Ok(WasmResult::Reply(val)) = pic.query_call(
        canister_id,
        Principal::anonymous(),
        "positionStatus",
        encode_one(account).unwrap(),
    ) else {
        panic!("Account could not be gotten")
    };
    let reply = decode_one(&val).unwrap();

    return reply;
}

//////////////////////////////////////////////////////////////////////////////////////////

/////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////
///  Setters Function
/////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////
fn _set_state(
    pic: &PocketIc,
    caller: Principal,
    current_tick: Tick,
    max_leveragex10: u8,
    min_collateral: Amount,
) {
    let canister_id = _get_canister_id();
    let state_details = StateDetails {
        not_paused: true,
        current_tick,
        max_leveragex10,
        min_collateral,
        base_token_multiple: 1,
    };

    let Ok(WasmResult::Reply(_)) = pic.update_call(
        canister_id,
        caller,
        "updateStateDetails",
        encode_one(state_details).unwrap(),
    ) else {
        panic!("error occured")
    };
}

fn _setup_market(admin: Principal) -> PocketIc {
    let pic = PocketIc::new();

    let perp_canister = pic.create_canister_with_settings(Some(admin), None);
    //
    pic.add_cycles(perp_canister, 2_000_000_000_000); // 2T Cycles
                                                      //
    let wasm = fs::read(_BACKEND_WASM).expect("Wasm file not found, run 'dfx build'.");

    let market_detais = MarketDetails {
        base_asset: Asset {
            class: AssetClass::Cryptocurrency,
            symbol: "ETH".to_string(),
        },
        quote_asset: Asset {
            class: AssetClass::Cryptocurrency,
            symbol: "ICP".to_string(),
        },
        xrc_id: admin,
        vault_id: admin,
        collateral_decimal: 1,
    };

    pic.install_canister(
        perp_canister,
        wasm,
        encode_one(market_detais).unwrap(),
        Some(admin),
    );

    _set_canister_id(perp_canister);
    return pic;
}

fn _get_principals() -> Vec<Principal> {
    return vec![
        Principal::from_text("hpp6o-wqx72-gol5b-3bmzw-lyryb-62yoi-pjoll-mtsh7-swdzi-jkf2v-rqe")
            .unwrap(),
        Principal::from_text("cvwul-djb3r-e6krd-nbnfl-tuhox-n4omu-kejey-3lku7-ae3bx-icbu7-yae")
            .unwrap(),
    ];
}

fn _get_canister_id() -> Principal {
    CANISTER_ID.with_borrow(|reference| reference.clone())
}

fn _set_canister_id(id: Principal) {
    CANISTER_ID.with_borrow_mut(|reference| *reference = id)
}
