// use candid::{decode_args, decode_one, encode_args, encode_one, Principal};
// use pocket_ic::{PocketIc, WasmResult};

// use std::fs;

// use crate::{
//     corelib::order_lib::LimitOrder,
//     types::{Asset, AssetClass, MarketDetails, StateDetails, Tick},
//     OrderType, PositionDetails, ID,
// };

// const _BACKEND_WASM: &str = "../../target/wasm32-unknown-unknown/release/perp.wasm";

// #[test]
// fn test_installation() {
//     let (pic, perp_canister, admin) = _setup_market();

//     set_state(&pic, perp_canister, admin, 100000 * 199);

//     let new_state_details = get_state(&pic, perp_canister);

//     let market_details = get_market_details(&pic, perp_canister);

//     println!(" The address is {}", perp_canister.to_text());

//     let state_details = StateDetails {
//         not_paused: true,
//         current_tick: 100000 * 199,
//         max_leveragex10: 100,
//         min_collateral: 0,
//         base_token_multiple: 1,
//     };

//     assert_eq!(state_details, new_state_details);

//     println!("The market details is {:?}", market_details)

//     // assert_eq!(result, "Hello, ICP!");
// }

// #[test]

// fn test_open_limit_order() {
//     let (pic, canister, admin) = _setup_market();

//     set_state(&pic, canister, admin, 199 * 100_000);

//     let collateral = 10000000000;

//     if let Ok(position) =
//         _open_limit_position(&pic, canister, admin, collateral, 20, 200 * 100000, false)
//     {
//         println!("the position is {:?}", position)
//     }
// }

// #[test]
// fn test_close_limit_order() {
//     let (pic, canister, admin) = _setup_market();

//     set_state(&pic, canister, admin, 199 * 100_000);

//     let collateral = 10000000000;

//     if let Ok(position) =
//         _open_limit_position(&pic, canister, admin, collateral, 20, 200 * 100000, false)
//     {
//         println!("the position is {:?}", position)
//     }

//     let profit = _close_position(&pic, canister, admin);

//     assert_eq!(profit, collateral);

//     println!("The value of collateral is {}", profit);
// }

// #[test]
// fn test_open_market_order() {
//     let (pic, canister, admin) = _setup_market();

//     set_state(&pic, canister, admin, 200 * 100_000);

//     let limit_collateral = 10000000000;

//     if let Ok(l_position) = _open_limit_position(
//         &pic,
//         canister,
//         admin,
//         limit_collateral,
//         20,
//         199 * 100_000,
//         true,
//     ) {
//         println!("the position is {:?}", l_position)
//     };

//     let market_collateral = 1000000;

//     if let Ok(m_position) = _open_market_position(
//         &pic,
//         canister,
//         Principal::anonymous(),
//         market_collateral,
//         20,
//         199 * 100_000,
//         false,
//     ) {
//         println!("the position is {:?}", m_position)
//     };

//     let new_state_details = get_state(&pic, canister);

//     assert_eq!(new_state_details.current_tick, 199 * 100_000);

//     let limit_collateral2 = 10000000000;

//     let princpal1 = _get_principals()[0];

//     if let Ok(l_position) = _open_limit_position(
//         &pic,
//         canister,
//         princpal1,
//         limit_collateral2,
//         20,
//         200 * 100_000,
//         false,
//     ) {
//         //  println!("the position is {:?}", l_position)
//     };

//     let profit = _close_position(&pic, canister, Principal::anonymous());

//     println!("The profit is {}", profit);
// }

// #[test]
// fn test_open_close_market_order() {
//     let (pic, canister, admin) = _setup_market();

//     set_state(&pic, canister, admin, 199 * 100_000);

//     let limit_collateral = 10000000000;

//     if let Ok(l_position) = _open_limit_position(
//         &pic,
//         canister,
//         admin,
//         limit_collateral,
//         20,
//         200 * 100_000,
//         false,
//     ) {
//         //  println!("the position is {:?}", l_position)
//     };

//     let princpal1 = _get_principals()[0];

//     let principal2 = _get_principals()[1];

//     let market_collateral = 1000000;

//     if let Ok(m_position) = _open_market_position(
//         &pic,
//         canister,
//         princpal1,
//         market_collateral,
//         20,
//         200 * 100_000,
//         true,
//     ) {
//         //   println!("the position is {:?}", m_position)
//     };

//     let new_state_details = get_state(&pic, canister);

//     assert_eq!(new_state_details.current_tick, 200 * 100_000);

//     // update state to reflect the changes
//     set_state(&pic, canister, admin, 300 * 100_000);

//     // close position by placing an order  below it

//     if let Ok(l_position2) = _open_limit_position(
//         &pic,
//         canister,
//         principal2,
//         limit_collateral,
//         20,
//         29930_000,
//         true,
//     ) {
//         println!("the position is {:?}", l_position2)
//     };

//     let profit = _close_position(&pic, canister, princpal1);

//     //   let market_close_profit = _close_position(&pic, canister, admin);

//     println!("The profit is {} ", profit);
// }

// fn get_market_details(pic: &PocketIc, canister: Principal) -> MarketDetails {
//     let Ok(WasmResult::Reply(res)) = pic.query_call(
//         canister,
//         Principal::anonymous(),
//         "get_market_details",
//         encode_one(()).unwrap(),
//     ) else {
//         panic!("error occured")
//     };

//     decode_one(&res).unwrap()
// }

// fn _open_limit_position(
//     pic: &PocketIc,
//     canister_id: Principal,
//     sender: Principal,
//     collateral: u128,
//     leverage: u8,
//     ref_tick: u64,
//     long: bool,
// ) -> Result<PositionDetails, String> {
//     let order = LimitOrder::default();
//     let Ok(WasmResult::Reply(res)) = pic.update_call(
//         canister_id,
//         sender,
//         "open_position",
//         encode_args((
//             collateral,
//             Some(ref_tick),
//             leverage,
//             long,
//             OrderType::Limit(order),
//             u64::default(),
//             u64::default(),
//         ))
//         .unwrap(),
//     ) else {
//         panic!("failed to open position")
//     };
//     decode_one(&res).unwrap()
// }

// fn _open_market_position(
//     pic: &PocketIc,
//     canister_id: Principal,
//     sender: Principal,
//     collateral: u128,
//     leverage: u8,
//     _ref_tick: u64,
//     long: bool,
// ) -> Result<PositionDetails, String> {
//     let max_tick: Option<Tick> = Option::None;
//     let Ok(WasmResult::Reply(res)) = pic.update_call(
//         canister_id,
//         sender,
//         "open_position",
//         encode_args((
//             collateral,
//             max_tick,
//             leverage,
//             long,
//             OrderType::Market,
//             u64::default(),
//             u64::default(),
//         ))
//         .unwrap(),
//     ) else {
//         panic!("failed to open position")
//     };
//     decode_one(&res).unwrap()
// }

// fn _close_position(pic: &PocketIc, canister_id: Principal, sender: Principal) -> u128 {
//     let max_tick: Option<Tick> = Option::None;
//     let Ok(WasmResult::Reply(res)) = pic.update_call(
//         canister_id,
//         sender,
//         "close_position",
//         encode_one(max_tick).unwrap(),
//     ) else {
//         panic!("failed to close position")
//     };

//     decode_one(&res).unwrap()
// }

// fn get_state(pic: &PocketIc, canister_id: Principal) -> StateDetails {
//     let Ok(WasmResult::Reply(val)) = pic.query_call(
//         canister_id,
//         Principal::anonymous(),
//         "get_state_details",
//         encode_one(()).unwrap(),
//     ) else {
//         panic!("error occured")
//     };

//     decode_one(&val).unwrap()
// }

// fn set_state(pic: &PocketIc, canister_id: Principal, caller: Principal, current_tick: Tick) {
//     let state_details = StateDetails {
//         not_paused: true,
//         current_tick,
//         max_leveragex10: 100,
//         min_collateral: 0,

//         base_token_multiple: 1,
//     };

//     let Ok(WasmResult::Reply(_)) = pic.update_call(
//         canister_id,
//         caller,
//         "update_state_details",
//         encode_one(state_details).unwrap(),
//     ) else {
//         panic!("error occured")
//     };
// }

// fn _setup_market() -> (PocketIc, Principal, Principal) {
//     let pic = PocketIc::new();

//     let admin = Principal::from_text("g4tto-rqaaa-aaaar-qageq-cai").unwrap();

//     let perp_canister = pic.create_canister_with_settings(Some(admin), None);
//     //
//     pic.add_cycles(perp_canister, 2_000_000_000_000); // 2T Cycles
//                                                       //
//     let wasm = fs::read(_BACKEND_WASM).expect("Wasm file not found, run 'dfx build'.");

//     let market_detais = MarketDetails {
//         vault_id: ID::from(admin),
//         perp_asset_details: Asset {
//             asset_class: AssetClass::Cryptocurrency,
//             symbol: "ETH".to_string(),
//         },
//         collateral_asset_details: Asset {
//             asset_class: AssetClass::Cryptocurrency,
//             symbol: "ICP".to_string(),
//         },
//         collateral_asset: ID::from(admin),
//         watcher_id: ID::from(admin),
//         collateral_decimal: 1,
//     };

//     pic.install_canister(
//         perp_canister,
//         wasm,
//         encode_one(market_detais).unwrap(),
//         Some(admin),
//     );
//     return (pic, perp_canister, admin);
// }

// fn _get_principals() -> Vec<Principal> {
//     return vec![
//         Principal::from_text("hpp6o-wqx72-gol5b-3bmzw-lyryb-62yoi-pjoll-mtsh7-swdzi-jkf2v-rqe")
//             .unwrap(),
//         Principal::from_text("cvwul-djb3r-e6krd-nbnfl-tuhox-n4omu-kejey-3lku7-ae3bx-icbu7-yae")
//             .unwrap(),
//     ];
// }
