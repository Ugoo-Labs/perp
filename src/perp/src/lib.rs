use candid::{CandidType, Decode, Encode, Principal};
use ic_cdk::{export_candid, storage};

use ic_cdk_timers::TimerId;
use sha2::{Digest, Sha256};

use corelib::calc_lib::{_calc_interest, _percentage128, _percentage64};
use corelib::constants::{_BASE_PRICE, _ONE_PERCENT};
use corelib::order_lib::{CloseOrderParams, LimitOrder, OpenOrderParams};
use corelib::price_lib::_equivalent;
use corelib::swap_lib::{SwapParams, _get_best_offer};
use corelib::tick_lib::{_def_max_tick, _tick_to_price};
use types::{
    FundingRateTracker, GetExchangeRateRequest, GetExchangeRateResult, MarketDetails, StateDetails,
    TickDetails,
};

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::Duration;

use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{storable::Bound, Storable};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, StableCell, StableVec};

type Time = u64;
type Amount = u128;
type Tick = u64;
type Subaccount = [u8; 32];

type Memory = VirtualMemory<DefaultMemoryImpl>;

const _ADMIN_MEMORY: MemoryId = MemoryId::new(1);

const _MARKET_DETAILS_MEMORY: MemoryId = MemoryId::new(2);

const _STATE_DETAILS_MEMORY: MemoryId = MemoryId::new(3);

const _FUNDING_RATE_TRACKER_MEMORY: MemoryId = MemoryId::new(4);

const _ACCOUNTS_POSITION_MEMORY: MemoryId = MemoryId::new(5);

const _ACCOUNT_ERROR_LOGS_MEMORY: MemoryId = MemoryId::new(6);

const _EXECUTABLE_ORDERS_MEMORY: MemoryId = MemoryId::new(7);

const ONE_SECOND: u64 = 1_000_000_000;

const ONE_HOUR: u64 = 3_600_000_000_000;

const DEFAULT_SWAP_SLIPPAGE: u64 = 30_000; //0.3%

thread_local! {

    static MEMORY_MANAGER:RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default())) ;

    static ADMIN:RefCell<StableCell<Principal,Memory>> = RefCell::new(StableCell::new(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_ADMIN_MEMORY)
    }),Principal::anonymous()).unwrap());


    static MARKET_DETAILS:RefCell<StableCell<MarketDetails,Memory>> = RefCell::new(StableCell::new(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_MARKET_DETAILS_MEMORY)
    }),MarketDetails::default()).unwrap());


        /// State details
    static STATE_DETAILS:RefCell<StableCell<StateDetails,Memory>> = RefCell::new(StableCell::new(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_STATE_DETAILS_MEMORY)
    }),StateDetails::default()).unwrap());

    static FUNDING_RATE_TRACKER:RefCell<StableCell<FundingRateTracker,Memory>> = RefCell::new(StableCell::new(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_FUNDING_RATE_TRACKER_MEMORY)
    }),FundingRateTracker::default()).unwrap());

    static ACCOUNTS_POSITION:RefCell<StableBTreeMap<Subaccount,PositionDetails,Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_ACCOUNTS_POSITION_MEMORY)
    })));


    static ACCOUNTS_ERROR_LOGS:RefCell<StableBTreeMap<Subaccount,PositionUpdateErrorLog,Memory>> = RefCell::new(
        StableBTreeMap::init(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_ACCOUNT_ERROR_LOGS_MEMORY)
    })));

    static EXECUTABLE_LIMIT_ORDERS_ACCOUNTS:RefCell<StableVec<Subaccount,Memory>> = RefCell::new(StableVec::new(MEMORY_MANAGER.with(|s|{
        s.borrow().get(_EXECUTABLE_ORDERS_MEMORY)
    })).unwrap());

    static PENDING_TIMER:RefCell<TimerId>= RefCell::new(TimerId::default());

    static INTEGRAL_BITMAPS:RefCell<HashMap<u64,u128>> = RefCell::new(HashMap::new());

    static TICKS_DETAILS :RefCell<HashMap<Tick,TickDetails>> = RefCell::new(HashMap::new());

    static LIMIT_ORDERS_RECORD :RefCell<HashMap<Tick,Vec<Subaccount>>> = RefCell::new(HashMap::new());

}

#[ic_cdk::init]
fn init(market_details: MarketDetails) {
    let caller = ic_cdk::api::caller();

    ADMIN.with(|ref_admin| ref_admin.borrow_mut().set(caller).unwrap());
    MARKET_DETAILS.with(|ref_market_details| {
        ref_market_details.borrow_mut().set(market_details).unwrap();
    });
}

/// Get State Details
///
/// Returns the Current State Details
#[ic_cdk::query(name = "getStateDetails")]
fn get_state_details() -> StateDetails {
    _get_state_details()
}

/// Get Market Details
///
///  Returns the Market Details
#[ic_cdk::query(name = "getMarketDetails")]
fn get_market_details() -> MarketDetails {
    _get_market_details()
}

///  get Tick Details
///
/// Returns the tick details of a particular tick if it is intialised else returns false
#[ic_cdk::query(name = "getTickDetails")]
fn get_tick_details(tick: Tick) -> TickDetails {
    TICKS_DETAILS.with(|ref_tick_details| ref_tick_details.borrow().get(&tick).unwrap().clone())
}

/// Try Close Function
///
/// Checks if a particular account's position of limit order type has been fully filled
///
/// Returns
/// - Is Fully Filled: true if position is limit order has been fully filled
/// - Is Partially Filled: true is position is partially filled
#[ic_cdk::query(name = "positionStatus")]
fn position_status(account: [u8; 32]) -> (bool, bool) {
    return _convert_account_limit_position(account);
}

/// Get Account Position
///
/// Gets an account position or panics if account has no position
#[ic_cdk::query(name = "getAccountPosition")]
fn get_account_position(account: [u8; 32]) -> PositionDetails {
    return _get_account_position(&account);
}

/// Open PositionDetails function
///
/// opens a new position for user (given that user has no existing position)
///
/// Params
/// - Collateral Value :: The amount in collatreal token to utilise as collateral
/// - Max Tick :: max executing tick ,also seen as max price fro the _swap ,if set to none or set outside the required range ,default max tick is used
/// - Leverage :: The leverage for the required position multiplies by 10 i.e a 1.5 levarage is 1.5 * 10 = 15
/// - Long :: Indicating if its a long position or not ,true if long and false otherwise
/// - Order Type :: the type of order to create
///  _
///
/// Returns
///  - Position:the details of the position
///
/// Note
///  - If Order type is a limit order ,max tick coinsides with the reference tick for the limit order
///  - ANON TICKS are for future purposes and have no effect for now
#[ic_cdk::update(name = "openPosition")]
async fn open_position(
    collateral_value: Amount,
    long: bool,
    order_type: OrderType,
    leveragex10: u8,
    max_tick: Option<Tick>,
    _anon_tick1: Tick,
    _anon_tick2: Tick,
) -> Result<PositionDetails, String> {
    let user = ic_cdk::caller();

    let account = user._to_subaccount();

    let failed_initial_check = _has_position_or_pending_error_log(&account);

    if failed_initial_check {
        return Err("Account has pending error or unclosed position ".to_string());
    }

    let mut state_details = _get_state_details();

    assert!(state_details.not_paused);

    // if leverage is greater than max leverage or collateral value is less than min collateral
    //returns
    if leveragex10 >= state_details.max_leveragex10
        || collateral_value < state_details.min_collateral
    {
        return Err("Max leverage exceeded or collateral is too small".to_string());
    }

    let market_details = _get_market_details();

    let vault = Vault::init(market_details.vault_id);

    // levarage is always given as a multiple of ten
    let debt_value = (u128::from(leveragex10 - 10) * collateral_value) / 10;

    // Checks if user has sufficient balance and vault contains free liquidity greater or equal to debt_value and then calculate interest rate

    let (valid, interest_rate) = vault
        .create_position_validity_check(user, collateral_value, debt_value)
        .await;

    if valid == false {
        return Err("Not enough liquidity for debt".to_string());
    };

    let stopping_tick = max_or_default_max(max_tick, state_details.current_tick, long);

    match _open_position(
        account,
        long,
        order_type,
        collateral_value,
        debt_value,
        interest_rate,
        state_details.current_tick,
        stopping_tick,
    ) {
        Some((position, resulting_tick, crossed_ticks)) => {
            // update current tick
            state_details.current_tick = resulting_tick;

            _set_state_details(state_details);

            if let OrderType::Limit = order_type {
                store_tick_order(stopping_tick, account);
            } else {
                _schedule_execution_for_ticks_orders(crossed_ticks);

                if position.debt_value != debt_value
                    || collateral_value != position.collateral_value
                {
                    let un_used_collateral = collateral_value - position.collateral_value;
                    vault.manage_position_update(
                        user,
                        un_used_collateral,
                        ManageDebtParams::init(
                            debt_value,
                            debt_value,
                            debt_value - position.debt_value,
                        ),
                    );
                }
            }

            return Ok(position);
        }
        None => {
            // send back
            vault.manage_position_update(
                user,
                collateral_value,
                ManageDebtParams::init(debt_value, debt_value, debt_value),
            );

            return Err("Failed to open position".to_string());
        }
    }
}

///Close PositionDetails Function
///
/// Closes user position and sends back collateral
///
/// Returns
///  - Profit :The amount to send to position owner
///
/// Note
///  -
/// If position order_type is a limit order and not fully filled ,two possibilities exists
///  - If not filled at all ,the collateral is sent back and the debt fully reapid without any interest
///  - If it is partially filled ,the position_type is converted into a market position with the amount filled as the entire position value and the ampount remaining is sent back    
#[ic_cdk::update(name = "closePosition")]
async fn close_position(max_tick: Option<Tick>) -> Amount {
    let user = ic_cdk::caller();

    let account = user._to_subaccount();

    let mut position = _get_account_position(&account);

    let market_details = _get_market_details();

    let vault = Vault::init(market_details.vault_id);

    match position.order_type {
        PositionOrderType::Market => {
            let mut state_details = _get_state_details();

            let current_tick = state_details.current_tick;

            let stopping_tick = max_or_default_max(max_tick, current_tick, !position.long);
            // if position type is market ,means the position is already active
            let (collateral_value, resulting_tick, crossed_ticks, manage_debt_params) = if position
                .long
            {
                _close_market_long_position(account, &mut position, current_tick, stopping_tick)
            } else {
                _close_market_short_position(account, &mut position, current_tick, stopping_tick)
            };
            // update current_tick
            state_details.current_tick = resulting_tick;

            _set_state_details(state_details);

            _schedule_execution_for_ticks_orders(crossed_ticks);

            if manage_debt_params.amount_repaid != 0 {
                vault.manage_position_update(user, collateral_value, manage_debt_params);
            }

            return collateral_value;
        }
        PositionOrderType::Limit(_) => {
            let (removed_collateral, manage_debt_params) = if position.long {
                _close_limit_long_position(account, &mut position)
            } else {
                _close_limit_short_position(account, &mut position)
            };

            remove_tick_order(position.entry_tick, account);

            if manage_debt_params.amount_repaid != 0 {
                vault.manage_position_update(user, removed_collateral, manage_debt_params);
            }

            return removed_collateral;
        }
    };
}

/// Liquidate Function
///
/// liquidates an account's position to avoid bad debt by checking if the current leverage exceeds the max leverage
///
/// Note : Position is closed at the current tick
#[ic_cdk::update(name = "liquidatePosition")]
fn liquidate_position(user: Principal) {
    let account = user._to_subaccount();
    let state_details = _get_state_details();

    let market_details = _get_market_details();

    let position = _get_account_position(&account);

    let (to_liquidate, collateral_remaining, net_debt_value) =
        _liquidation_status(position, state_details.max_leveragex10);

    if to_liquidate {
        let vault = Vault::init(market_details.vault_id);

        let (collateral, amount_repaid) = if collateral_remaining > 0 {
            (collateral_remaining.abs() as u128, net_debt_value)
        } else {
            (0, net_debt_value - (collateral_remaining.abs() as u128))
        };

        let manage_debt_params =
            ManageDebtParams::init(position.debt_value, net_debt_value, amount_repaid);

        vault.manage_position_update(user, collateral, manage_debt_params);

        _remove_account_position(&account);
    }
}

/// Open PositionDetails (Private)
///
/// opens a position for user if possible
/// Params
///  - Account :The owner of the position
///  - Long : Position direction ,true if long or false otherwise
///  - Limit : true for opening a limit order and false for a market order
///  - Collateral Value : amount of collateral asset being put in as collateral
///  - Debt Value : The amount of collateral_asset used as debt for opening position
///  - Interest Rate : The current interest rate for opening a position
///  - Entry Tick : The entry tick or the current state tick for this market
///  - Max Tick : The maximum tick to execute swap ,also seen as maximum price
///
/// Returns
///  - Option containing
///  - - Position Details :The details of the position created
///  - - Resulting Tick :The resuting tick from swapping
///  - - Crossed Ticks :A vector of all crossed ticks during swap
/// Note
///  - If position can not be opened it returns none and both collateral and debt gets refunded back and swap is reverted afterwards
///
fn _open_position(
    account: Subaccount,
    long: bool,
    order_type: OrderType,
    collateral_value: Amount,
    debt_value: Amount,
    interest_rate: u32,
    current_tick: Tick,
    max_tick: Tick,
) -> Option<(PositionDetails, Tick, Vec<Tick>)> {
    //
    let equivalent = |amount: Amount, tick: Tick, buy: bool| -> Amount {
        let tick_price = _tick_to_price(tick);
        _equivalent(amount, tick_price, buy)
    }; //(actual debt,resulting_tick,crossed_ticks);

    match order_type {
        OrderType::Limit => {
            let entry_tick = max_tick;
            // limit order's can't be placed at current tick
            if long && entry_tick >= current_tick {
                return None;
            } else if !long && entry_tick <= current_tick {
                return None;
            } else {
            };

            let (collateral, debt) = if long {
                (collateral_value, debt_value)
            } else {
                (
                    equivalent(collateral_value, entry_tick, true),
                    equivalent(debt_value, entry_tick, true),
                )
            };
            //
            let mut order = LimitOrder::new(collateral + debt, entry_tick, long);

            _open_order(&mut order);

            let position = PositionDetails {
                long,
                entry_tick,
                collateral_value,
                debt_value,
                interest_rate,
                volume_share: 0, // not initialised yet
                order_type: PositionOrderType::Limit(order),
                timestamp: 0, //not initialised
            };

            _insert_account_position(account, position);

            let new_current_tick = match get_best_offer(true, current_tick, Some(max_tick)) {
                Some(tick) => tick,
                None => current_tick,
            };

            return Some((position, new_current_tick, Vec::new()));
        }

        OrderType::Market => {
            if long {
                _open_market_long_position(
                    account,
                    collateral_value,
                    debt_value,
                    interest_rate,
                    current_tick,
                    max_tick,
                )
            } else {
                _open_market_short_position(
                    account,
                    collateral_value,
                    debt_value,
                    interest_rate,
                    current_tick,
                    max_tick,
                )
            }
        }
    }
}

/// Open Market Long Position'
///
/// Params :See Open Position for params definition
fn _open_market_long_position(
    account: Subaccount,
    collateral_value: Amount,
    debt_value: Amount,
    interest_rate: u32,
    current_tick: Tick,
    max_tick: Tick,
) -> Option<(PositionDetails, Tick, Vec<Tick>)> {
    let (collateral, debt) = (collateral_value, debt_value);

    let (amount_out, amount_remaining_value, resulting_tick, crossed_ticks) =
        _swap(collateral + debt, true, current_tick, max_tick);

    if amount_out == 0 {
        return None;
    }

    let (un_used_debt_value, un_used_collateral_value) = if amount_remaining_value >= debt_value {
        (debt_value, amount_remaining_value - debt_value)
    } else {
        (amount_remaining_value, 0)
    };

    let resulting_debt_value = debt_value - un_used_debt_value;
    let resulting_collateral_value = collateral_value - un_used_collateral_value;

    let position_value = collateral_value + debt_value - amount_remaining_value;

    let volume_share = _calc_position_volume_share(position_value, true);

    let position = PositionDetails {
        long: true,
        entry_tick: resulting_tick,
        collateral_value: resulting_collateral_value,
        debt_value: resulting_debt_value, //actual debt
        interest_rate,
        volume_share,
        order_type: PositionOrderType::Market,
        timestamp: ic_cdk::api::time(), //change to time()
    };
    _insert_account_position(account, position);

    // getthe best sell offer or lowest sell offer
    let new_current_tick = match get_best_offer(true, resulting_tick, None) {
        Some(tick) => tick,
        None => resulting_tick,
    };

    return Some((position, new_current_tick, crossed_ticks));
}

/// Open Market Short Position
///
/// Similar to Open Market Long position but for opening short positions
fn _open_market_short_position(
    account: Subaccount,
    collateral_value: Amount,
    debt_value: Amount,
    interest_rate: u32,
    initial_tick: Tick,
    max_tick: Tick,
) -> Option<(PositionDetails, Tick, Vec<Tick>)> {
    let equivalent = |amount: Amount, tick: Tick, buy: bool| -> Amount {
        let tick_price = _tick_to_price(tick);
        _equivalent(amount, tick_price, buy)
    };

    let best_buy_offer_tick = match get_best_offer(false, initial_tick, Some(max_tick)) {
        Some(tick) => tick,
        None => return None,
    };

    let (collateral, debt) = (
        equivalent(collateral_value, best_buy_offer_tick, true),
        equivalent(debt_value, best_buy_offer_tick, true),
    );

    let (amount_out_value, amount_remaining, resulting_tick, crossed_ticks) =
        _swap(collateral + debt, false, best_buy_offer_tick, max_tick);

    if amount_out_value == 0 {
        return None;
    }

    let amount_remaining_value = equivalent(amount_remaining, best_buy_offer_tick, false);

    let (un_used_debt_value, un_used_collateral_value) = if amount_remaining_value >= debt_value {
        (debt_value, amount_remaining_value - debt_value)
    } else {
        (amount_remaining_value, 0)
    };

    let resulting_debt_value = debt_value - un_used_debt_value;
    let resulting_collateral_value = collateral_value - un_used_collateral_value;

    let position_value = amount_out_value;

    let volume_share = _calc_position_volume_share(position_value, false);

    let position = PositionDetails {
        long: false,
        entry_tick: resulting_tick,
        collateral_value: resulting_collateral_value,
        debt_value: resulting_debt_value, //actual debt
        interest_rate,
        volume_share,
        order_type: PositionOrderType::Market,
        timestamp: ic_cdk::api::time(), //change to time()
    };
    _insert_account_position(account, position);

    let new_current_tick = match get_best_offer(true, initial_tick, None) {
        Some(tick) => tick,
        None => initial_tick,
    };

    return Some((position, new_current_tick, crossed_ticks));
}

/// Close Long PositionDetails
///
///closes a user's  long position if position can be fully closed and  repays debt
///
/// Params
/// - User :The user (position owner)
/// - PositionDetails :The PositionDetails
/// - Current Tick :The current tick of market's state
/// - Stopping Tick : The max tick,corresponds to max price
/// - Vault : Vault canister
///
/// Returns
///  - Current Collateral :The amount to send to position owner after paying debt ,this amount is zero if debt is not fully paid
///  - Resulting Tick :The resulting tick from swapping
///  - Crosssed Ticks :An array of ticks that have been crossed during swapping
///   
/// Note
///  - If position can not be closed fully ,the position is partially closed (updated) and debt is paid back either fully or partially
fn _close_market_long_position(
    account: Subaccount,
    position: &mut PositionDetails,
    current_tick: Tick,
    stopping_tick: Tick,
) -> (Amount, Tick, Vec<Tick>, ManageDebtParams) {
    //
    let entry_price = _tick_to_price(position.entry_tick);
    let equivalent_at_entry_price =
        |amount: Amount, buy: bool| -> Amount { _equivalent(amount, entry_price, buy) };
    //
    let position_realised_value = _calc_position_realised_value(position.volume_share, true);

    let realised_position_size = equivalent_at_entry_price(position_realised_value, true);

    let (amount_out_value, amount_remaining, _, crossed_ticks) =
        _swap(realised_position_size, false, current_tick, stopping_tick);

    let interest_value = _calc_interest(
        position.debt_value,
        position.interest_rate,
        position.timestamp,
    );

    let profit: u128;

    let manage_debt_params: ManageDebtParams;

    if amount_remaining > 0 {
        let amount_remaining_value = equivalent_at_entry_price(amount_remaining, false);
        //
        (profit, manage_debt_params) = _update_market_position_after_swap(
            position,
            amount_out_value,
            amount_remaining_value,
            interest_value,
        );

        _insert_account_position(account, position.clone());
    } else {
        let net_debt = position.debt_value + interest_value;
        (profit, manage_debt_params) = (
            amount_out_value - net_debt,
            ManageDebtParams::init(position.debt_value, net_debt, net_debt),
        );
        _remove_account_position(&account);
    }

    let new_current_tick = match get_best_offer(true, current_tick, None) {
        Some(tick) => tick,
        None => current_tick,
    };

    return (profit, new_current_tick, crossed_ticks, manage_debt_params);
}

/// Close Short Position
///
/// similar to Close Long Function,but for short positions
fn _close_market_short_position(
    account: Subaccount,
    position: &mut PositionDetails,
    init_tick: Tick,
    stopping_tick: Tick,
) -> (Amount, Tick, Vec<Tick>, ManageDebtParams) {
    let position_realised_value = _calc_position_realised_value(position.volume_share, false);

    let realised_position_size = position_realised_value;

    let best_sell_offer_tick = match get_best_offer(true, init_tick, Some(stopping_tick)) {
        Some(tick) => tick,
        None => {
            return (
                0,
                init_tick,
                Vec::new(),
                ManageDebtParams::init(position.debt_value, position.debt_value, 0),
            )
        }
    };

    let (amount_out, amount_remaining_value, resulting_tick, crossed_ticks) = _swap(
        realised_position_size,
        true,
        best_sell_offer_tick,
        stopping_tick,
    );

    let best_price = _tick_to_price(best_sell_offer_tick);

    let amount_out_value = _equivalent(amount_out, best_price, false);

    let interest_value = _calc_interest(
        position.debt_value,
        position.interest_rate,
        position.timestamp,
    );

    let profit: u128;
    let manage_debt_params: ManageDebtParams;

    if amount_remaining_value > 0 {
        (profit, manage_debt_params) = _update_market_position_after_swap(
            position,
            amount_out_value,
            amount_remaining_value,
            interest_value,
        );

        _insert_account_position(account, position.clone());
    } else {
        let net_debt = position.debt_value + interest_value;
        (profit, manage_debt_params) = (
            amount_out_value - net_debt,
            ManageDebtParams::init(position.debt_value, net_debt, net_debt),
        );
        // deletes user position
        _remove_account_position(&account);
    }

    let new_current_tick = match get_best_offer(true, resulting_tick, None) {
        Some(tick) => tick,
        None => resulting_tick,
    };

    return (profit, new_current_tick, crossed_ticks, manage_debt_params);
}

/// Close Limit Position
///
///
/// Closes a limit position at a particular tick by closing removing the limit order if the order is not filled
///
/// Params
///  - User : The owner of the position
///  - Position : The particular position to close
///  - Vault :The vault type representing the vault canister  
fn _close_limit_long_position(
    account: Subaccount,
    position: &mut PositionDetails,
) -> (Amount, ManageDebtParams) {
    match position.order_type {
        //
        PositionOrderType::Limit(order) => {
            let (amount_received, amount_remaining_value) = _close_order(&order);

            let (removed_collateral, manage_debt_params);

            if amount_received == 0 {
                (removed_collateral, manage_debt_params) = (
                    position.collateral_value,
                    ManageDebtParams::init(
                        position.debt_value,
                        position.debt_value,
                        position.debt_value,
                    ),
                );

                _remove_account_position(&account);
            } else {
                (removed_collateral, manage_debt_params) =
                    _convert_limit_position(position, amount_remaining_value);

                _insert_account_position(account, position.clone());
            };

            return (removed_collateral, manage_debt_params);
        }
        PositionOrderType::Market => (0, ManageDebtParams::default()),
    }
}

/// Close Limit Short Function
///
/// Similar to close limit long position function but for long position
fn _close_limit_short_position(
    account: Subaccount,
    position: &mut PositionDetails,
) -> (Amount, ManageDebtParams) {
    match position.order_type {
        PositionOrderType::Limit(order) => {
            let (amount_received, amount_remaining) = _close_order(&order);

            let (removed_collateral, manage_debt_params);

            if amount_received == 0 {
                (removed_collateral, manage_debt_params) = (
                    position.collateral_value,
                    ManageDebtParams::init(
                        position.debt_value,
                        position.debt_value,
                        position.debt_value,
                    ),
                );
                _remove_account_position(&account);
            } else {
                let entry_price = _tick_to_price(position.entry_tick);

                let amount_remaining_value = _equivalent(amount_remaining, entry_price, false);
                (removed_collateral, manage_debt_params) =
                    _convert_limit_position(position, amount_remaining_value);
                // updates users positiion
                _insert_account_position(account, position.clone());
            };

            return (removed_collateral, manage_debt_params);
        }
        // unreachable code
        PositionOrderType::Market => return (0, ManageDebtParams::default()),
    }
}

/// Update Market Position After Swap Function
///
/// This function updates a  market position if it can not be closed i.e amount remaining after swapping to close position is greater than 0
///
/// It
///   - Updates the position debt ,the position collateral value , the position volume share
///   - Derives the update asset params that pays the debt either fully or partially
///
/// Params
///  - Position :A mutable reference to the particular position
///  - Resulting Tick : The resulting tick after swapping to closing the position
///  - Amount Out Value :The value of the amount gotten from swapping
///  - Amount Remaining Value :The value of the amount remaining after swapping
///  - Interest Value : The value of the interest accrued on current position debt
///
/// Returns
///  - Profit : The amount of profit for position owner or the amount of removable collateral from position
///  - Manage Debt Params : for repaying debt ,specifying the current debt and the previous debt and interest paid
fn _update_market_position_after_swap(
    position: &mut PositionDetails,
    amount_out_value: Amount,
    amount_remaining_value: Amount,
    interest_value: Amount,
) -> (Amount, ManageDebtParams) {
    let init_debt_value = position.debt_value;

    let net_debt_value = init_debt_value + interest_value;

    let profit;
    let manage_debt_params;
    //
    if amount_out_value < net_debt_value {
        //update new position details
        position.debt_value = net_debt_value - amount_out_value;

        profit = 0;

        manage_debt_params =
            ManageDebtParams::init(init_debt_value, net_debt_value, amount_out_value);
    } else {
        position.debt_value = 0;
        position.collateral_value = amount_remaining_value;

        profit = amount_out_value - net_debt_value;

        manage_debt_params =
            ManageDebtParams::init(init_debt_value, net_debt_value, net_debt_value);
    }

    let new_volume_share = _calc_position_volume_share(amount_remaining_value, position.long);

    position.volume_share = new_volume_share;

    // if position last time updated is greater than one hour ago ,position time is updated to current timestamp
    if position.timestamp + ONE_HOUR > ic_cdk::api::time() {
        position.timestamp = ic_cdk::api::time()
    }

    return (profit, manage_debt_params);
}

/// Convert Account Limit Position
///
/// Params:
///  - Account :The owner of the position
///
/// Returns
///   - is Fully Filled :Returns true  the limit order has been fully filled or returns false otherwise
///   - is Partially Filled :true if the position partially filled
fn _convert_account_limit_position(account: Subaccount) -> (bool, bool) {
    let mut position = _get_account_position(&account);

    if let PositionOrderType::Limit(order) = position.order_type {
        let (amount_out, amount_remaining) = _close_order(&order);

        let is_fully_filled = amount_remaining == 0;
        let is_partially_filled = amount_out > 0;
        _convert_limit_position(&mut position, 0);
        _insert_account_position(account, position);

        return (is_fully_filled, is_partially_filled);
    }
    return (false, false);
}

/// Convert Limit Position function
///
/// Converts a limit position into a market position after the reference limit order of that position has been filled fully or partially
/// any unfilled amount is refunded first as debt and if still remaining it is refunded back to the position owner and the position is updated to a market position
///
/// Params
///  - Position : A mutable reference to the cuurent position
///  - Amount Remaining Value : The value of the amount of  unfilled liquidity of the particular order
///
/// Returns
///  - Removed Collateral : The amount of collateral removed from that position
///  - Update Asset Details Params :The update asset details params for updating asset detailsin params   
///
fn _convert_limit_position(
    position: &mut PositionDetails,
    amount_remaining_value: Amount,
) -> (Amount, ManageDebtParams) {
    let initial_collateral_value: u128 = position.collateral_value;

    let initial_debt_value = position.debt_value;

    //
    let removed_collateral;
    if amount_remaining_value > initial_debt_value {
        removed_collateral = amount_remaining_value - initial_debt_value;

        position.debt_value = 0;
        position.collateral_value -= removed_collateral;
    } else {
        removed_collateral = 0;

        position.debt_value -= amount_remaining_value;
    }

    let remaining_order_value =
        initial_collateral_value + initial_debt_value - amount_remaining_value;

    let volume_share = _calc_position_volume_share(remaining_order_value, position.long);

    position.volume_share = volume_share;
    position.order_type = PositionOrderType::Market;
    position.timestamp = ic_cdk::api::time();

    let manage_debt_params = ManageDebtParams::init(
        initial_debt_value,
        initial_debt_value,
        initial_debt_value - position.debt_value,
    );

    return (removed_collateral, manage_debt_params);
}

/// Liquidation Status Function
///
/// Checks if a position is to be liquidated and the corrseponding collateral for liquidating that position
///
/// Params ;
///  - Position :The Position to check
///  - Max Leverage :The current maximum leverage for opening a position
///
/// Returns
///  - To Liquidate :true if position should be liquidated
///  - Collateral_Remaining : Returns the current value of collateral within the position
///  - Net Debt Value :returns the net debt value
///
/// Note :This collateral value can be less than zero in such case, a bad debt has occured
fn _liquidation_status(position: PositionDetails, max_leveragex10: u8) -> (bool, i128, Amount) {
    if let PositionOrderType::Market = position.order_type {
        let initial_collateral = position.collateral_value;

        let (pnl_in_percentage, net_debt_value) =
            _calculate_position_pnl_and_net_debt_value(position);

        let profit_or_loss = _percentage128(pnl_in_percentage.abs() as u64, initial_collateral);

        let current_collateral_value = if pnl_in_percentage > 0 {
            (position.collateral_value + profit_or_loss) as i128
        } else {
            (position.collateral_value as i128) - (profit_or_loss as i128)
        };

        let current_leverage_x10 = ((position.debt_value + position.collateral_value) as i128 * 10)
            / current_collateral_value;

        if current_leverage_x10.abs() as u8 >= max_leveragex10 {
            return (true, current_collateral_value, net_debt_value);
        }
    }

    return (false, 0, 0);
}

/// Opens Order Functions
///
/// opens an order at a particular tick
///
/// Params
/// - Order :: a generic type that implements the trait Order for the type of order to close
/// - Reference Tick :: The  tick to place order

fn _open_order(order: &mut LimitOrder) {
    TICKS_DETAILS.with_borrow_mut(|ticks_details| {
        INTEGRAL_BITMAPS.with_borrow_mut(|integrals_bitmaps| {
            let mut open_order_params = OpenOrderParams {
                order,
                integrals_bitmaps,
                ticks_details,
            };
            open_order_params.open_order();
        })
    });
}

/// Close Order Function
///
/// closes an order at a particular tick
///
/// Params :
///  - Order :: a generic that implements the trait Order for the type of order to close
///  - Order Size :: Tha amount of asset in order
///  - Order Direction :: Either a buy or a sell
///  - Order Reference Tick :: The tick where order was placed  
///
/// Returns
///  - Amont Out :: This corresponds to the asset to be bought i.e perp(base) asset for a buy order or quote asset for a sell order
///  - Amount Remaining :: This amount remaining corrseponds to the amount of asset at that tick that is still unfilled
///
fn _close_order(order: &LimitOrder) -> (Amount, Amount) {
    TICKS_DETAILS.with_borrow_mut(|ticks_details| {
        INTEGRAL_BITMAPS.with_borrow_mut(|multipliers_bitmaps| {
            let mut close_order_params = CloseOrderParams {
                order,
                multipliers_bitmaps,
                ticks_details,
            };
            close_order_params.close_order()
        })
    })
}

/// Swap Function
///
/// Params
///  - Order Size :: Tha amount of asset in order
///  - Buy :: the order direction ,true for buy and false otherwise
///  - Init Tick :: The current state tick
///  - Stopping Tick :: The maximum tick ,corresponds to maximum price
///
/// Returns
///  - Amount Out :: The amount out froom swapping
///  - Amount Remaining :: The amount remaining from swapping
///  - resulting Tick :The last tick at which swap occured
///  - Crossed Ticks :: An vector of all ticks crossed during swap
fn _swap(
    order_size: Amount,
    buy: bool,
    init_tick: Tick,
    stopping_tick: Tick,
) -> (Amount, Amount, Tick, Vec<Tick>) {
    TICKS_DETAILS.with_borrow_mut(|ticks_details| {
        INTEGRAL_BITMAPS.with_borrow_mut(|integrals_bitmaps| {
            let mut swap_params = SwapParams {
                buy,
                init_tick,
                stopping_tick,
                order_size,
                integrals_bitmaps,
                ticks_details,
            };
            swap_params._swap()
        })
    })
}

fn get_best_offer(buy: bool, current_tick: Tick, stopping_tick: Option<Tick>) -> Option<Tick> {
    let max_tick = match stopping_tick {
        Some(val) => val,
        None => max_or_default_max(None, current_tick, buy),
    };
    TICKS_DETAILS.with_borrow_mut(|ticks_details| {
        INTEGRAL_BITMAPS.with_borrow_mut(|integrals_bitmaps| {
            _get_best_offer(
                buy,
                current_tick,
                max_tick,
                integrals_bitmaps,
                ticks_details,
            )
        })
    })
}

/// Max or Default Max Tick
///
/// retrieves the max tick if valid else returns the default max tick

fn max_or_default_max(max_tick: Option<Tick>, current_tick: Tick, buy: bool) -> Tick {
    match max_tick {
        Some(tick) => {
            if buy && tick <= _def_max_tick(current_tick, true) {
                return tick;
            };
            if !buy && tick >= _def_max_tick(current_tick, false) {
                return tick;
            }
        }
        None => {
            if buy {
                return current_tick + _percentage64(DEFAULT_SWAP_SLIPPAGE, current_tick);
            } else {
                return current_tick - _percentage64(DEFAULT_SWAP_SLIPPAGE, current_tick);
            }
        }
    }
    return _def_max_tick(current_tick, buy);
}
/// Calculate Position PNL
///
/// Calculates the current pnl in percentage  for a particular position
///
/// Returns
///  PNL :The pnl(in percentage) on that position
///  Net Debt Value :The net debt on that position
fn _calculate_position_pnl_and_net_debt_value(position: PositionDetails) -> (i64, Amount) {
    let equivalent = |amount: Amount, tick: Tick, buy: bool| {
        let tick_price = _tick_to_price(tick);
        _equivalent(amount, tick_price, buy)
    };

    let state_details = _get_state_details();

    let position_realised_value =
        _calc_position_realised_value(position.volume_share, position.long);

    let interest_on_debt_value = _calc_interest(
        position.debt_value,
        position.interest_rate,
        position.timestamp,
    );

    let net_debt_value = position.debt_value + interest_on_debt_value;

    let pnl;

    if position.long {
        let init_position_value = (position.debt_value + position.collateral_value) as i128;

        let position_realised_size = equivalent(position_realised_value, position.entry_tick, true);

        let position_current_value =
            equivalent(position_realised_size, state_details.current_tick, false) as i128;

        pnl = ((position_current_value - (interest_on_debt_value as i128) - init_position_value)
            * (100 * _ONE_PERCENT as i128))
            / init_position_value;
    } else {
        let init_position_size = equivalent(
            position.debt_value + position.collateral_value,
            position.entry_tick,
            true,
        ) as i128;

        let position_current_size =
            equivalent(position_realised_value, state_details.current_tick, true) as i128;

        let interest_on_debt = equivalent(interest_on_debt_value, position.entry_tick, true);

        pnl = ((position_current_size - (interest_on_debt as i128) - init_position_size)
            * (100 * _ONE_PERCENT as i128))
            / init_position_size;
    }
    return (pnl as i64, net_debt_value);
}

///////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////
///  Funding Rate Functions
///////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////
/// Settle Funcding Rate
///
/// Settles Funding Rate by calling the XRC cansiter .fetching the Price ,calculating the premium and distributing the  fund to the right market direction,Long or Short
async fn settle_funding_rate() {
    let market_details = _get_market_details();

    let xrc = XRC::init(market_details.xrc_id);

    let request = GetExchangeRateRequest {
        base_asset: market_details.base_asset,
        quote_asset: market_details.quote_asset,
        timestamp: None,
    };

    match xrc._get_exchange_rate(request).await {
        Ok(rate_result) => {
            let state_details = _get_state_details();

            let current_price = _tick_to_price(state_details.current_tick);

            let perp_price =
                (current_price * 10u128.pow(rate_result.metadata.decimals)) / _BASE_PRICE;

            let spot_price = rate_result.rate as u128;

            _settle_funding_rate(perp_price, spot_price);
        }
        Err(_) => {
            return;
        }
    }
}

fn _settle_funding_rate(perp_price: u128, spot_price: u128) {
    let funding_rate = _calculate_funding_rate_premium(perp_price, spot_price);
    FUNDING_RATE_TRACKER.with_borrow_mut(|reference| {
        let mut funding_rate_tracker = reference.get().clone();

        funding_rate_tracker.settle_funding_rate(funding_rate.abs() as u64, funding_rate > 0);

        reference.set(funding_rate_tracker).unwrap();
    })
}

fn _calculate_funding_rate_premium(perp_price: u128, spot_price: u128) -> i64 {
    let funding_rate = ((perp_price as i128 - spot_price as i128) * 100 * _ONE_PERCENT as i128)
        / spot_price as i128;
    return funding_rate as i64;
}
///Calculate Position Realised value
///
///Calculates the Realised value for a position's volume share in a particular market direction,Long or Short   
///
/// Note:This function also adjust's the volume share
fn _calc_position_realised_value(volume_share: Amount, long: bool) -> Amount {
    FUNDING_RATE_TRACKER.with_borrow_mut(|tr| {
        let mut funding_rate_tracker = tr.get().clone();

        let value = funding_rate_tracker.remove_volume(volume_share, long);

        tr.set(funding_rate_tracker).unwrap();
        value
    })
}
/// Calculate Position Volume Share
///
/// Calculates the volume share for a particular poistion volume in a market direction ,Long or Short
fn _calc_position_volume_share(position_value: Amount, long: bool) -> Amount {
    FUNDING_RATE_TRACKER.with_borrow_mut(|tr| {
        let mut funding_rate_tracker = tr.get().clone();

        let value = funding_rate_tracker.add_volume(position_value, long);

        tr.set(funding_rate_tracker).unwrap();
        value
    })
}
////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////

//////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////
///  Limit Order Functions
/////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////
/// Store Tick Order
///
/// Stores an order under a particular tick
///
/// Utilised when positions are opened as limit orders
///
///Params
/// - Tick    :The tickat which order is placed
/// - Account : The account opening the position
pub fn store_tick_order(tick: Tick, account: Subaccount) {
    LIMIT_ORDERS_RECORD.with_borrow_mut(|reference| {
        let accounts = reference.entry(tick).or_insert(Vec::new());

        accounts.push(account);
    })
}

/// Remove Tick Order
///
/// Removes an order under a particular tick
///
/// Utilised when account owner closes a limit position before reference tick is fully crossed
///
/// Params
/// - Tick    :The tickat which order was placed
/// - Account : The account closing the position
pub fn remove_tick_order(tick: Tick, account: Subaccount) {
    LIMIT_ORDERS_RECORD.with_borrow_mut(|reference| {
        let accounts = reference.get_mut(&tick).unwrap();

        let index = accounts.iter().position(|x| x == &account).unwrap();

        accounts.remove(index);
    })
}

fn _schedule_execution_for_ticks_orders(crossed_ticks: Vec<Tick>) {
    if crossed_ticks.len() == 0 {
        return;
    }
    ic_cdk_timers::set_timer(Duration::from_nanos(2 * ONE_SECOND), || {
        _execute_ticks_orders(crossed_ticks)
    });
}

/// Execute Ticks Orders
///
/// Notifies Watcher to execute all orders placed at those tick respectively
///
/// Ticks:  An array of ticks crossed during the swap (meaning all orders at those tick has been filled)
pub fn _execute_ticks_orders(ticks: Vec<Tick>) {
    let mut count = 1u64;

    for tick in ticks {
        // set the exceution incrementally
        ic_cdk_timers::set_timer(Duration::from_nanos(count * ONE_SECOND), move || {
            serialize_limit_orders_accounts(tick)
        });

        count += 1
    }

    ic_cdk_timers::set_timer(Duration::from_nanos((count + 1) * ONE_SECOND), || {
        set_timer_for_limit_orders_execution();
    });
}

/// Serialize Limit Order Accounts
///
/// serializes all accounts with limit orders at a particular tick
fn serialize_limit_orders_accounts(tick: Tick) {
    LIMIT_ORDERS_RECORD.with_borrow_mut(|reference| {
        let accounts = reference.get(&tick).unwrap();

        for account in accounts.iter() {
            EXECUTABLE_LIMIT_ORDERS_ACCOUNTS.with_borrow_mut(|ref2| {
                ref2.push(account).unwrap();
            })
        }
    })
}

/// Sets Timer for executing each limit order that has been filled, by creating a timer_interval for executing each limit order
///
/// Note:This function checks if TimerID is default(meaning it is not set or no timer interval is already running) and if true sets a timer_interval else
/// starts a timer interval
fn set_timer_for_limit_orders_execution() {
    let pending_timer = _get_pending_timer();

    if pending_timer == TimerId::default() {
        let timer_id =
            ic_cdk_timers::set_timer_interval(Duration::from_nanos(2 * ONE_SECOND), || {
                _execute_each_limit_order();
            });

        _set_pending_timer(timer_id);
    }
}

#[test]

fn printing() {
    print!("{:?}", TimerId::default());
}

/// Execute Each Limit Order
///
/// Checks if user has any limit order ans if so  
fn _execute_each_limit_order() {
    EXECUTABLE_LIMIT_ORDERS_ACCOUNTS.with_borrow_mut(|reference| {
        if reference.len() == 0 {
            let timer_id = _get_pending_timer();

            _set_pending_timer(TimerId::default());

            ic_cdk_timers::clear_timer(timer_id);
        }

        let account = reference.pop().unwrap_or_default();

        _convert_account_limit_position(account);
    })
}
//////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////

//////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////
/// System Functions
//////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////
#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let multiplier_bitmaps =
        INTEGRAL_BITMAPS.with_borrow(|ref_mul_bitmaps| ref_mul_bitmaps.clone());

    let ticks_details = TICKS_DETAILS.with_borrow(|ref_ticks_details| ref_ticks_details.clone());

    storage::stable_save((multiplier_bitmaps, ticks_details)).expect("error storing data");
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let multiplier_bitmaps: HashMap<u64, u128>;

    let ticks_details: HashMap<Tick, TickDetails>;
    (multiplier_bitmaps, ticks_details) = storage::stable_restore().unwrap();
    INTEGRAL_BITMAPS.with(|ref_mul_bitmaps| *ref_mul_bitmaps.borrow_mut() = multiplier_bitmaps);

    TICKS_DETAILS.with(|ref_ticks_details| {
        *ref_ticks_details.borrow_mut() = ticks_details;
    })
}
//////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////
/// Admin Functions
//////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////
fn admin_guard() -> Result<(), String> {
    ADMIN.with_borrow(|admin_ref| {
        let admin = admin_ref.get().clone();
        if ic_cdk::caller() == admin {
            return Ok(());
        } else {
            return Err("Invalid".to_string());
        };
    })
}

#[ic_cdk::update(guard = "admin_guard", name = "updateStateDetails")]
async fn update_state_details(new_state_details: StateDetails) {
    _set_state_details(new_state_details);
}

#[ic_cdk::update(guard = "admin_guard", name = "startTimer")]
async fn start_timer() {
    ic_cdk_timers::set_timer_interval(Duration::from_nanos(ONE_HOUR), || {
        ic_cdk::spawn(async { settle_funding_rate().await });
    });
}

//////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////
///  Error Handling Functions
//////////////////////////////////////////////////////////////////////////////////////////////////
/// ///////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////
fn trusted_canister_guard() -> Result<(), String> {
    let market_details = _get_market_details();

    let caller = ic_cdk::caller();

    if caller == market_details.vault_id {
        return Ok(());
    } else {
        return Err("Untrusted Caller".to_string());
    }
}

#[ic_cdk::update(name = "retryAccountError")]
async fn retry_account_error(user: Principal) {
    let account = user._to_subaccount();

    let account_error_log = _get_account_error_log(&account);

    let details = _get_market_details();
    account_error_log.retry(details);
}

#[ic_cdk::update(name = "successNotification", guard = "trusted_canister_guard")]
async fn success_notif(account: Subaccount, _error_index: usize) {
    let market_details = _get_market_details();

    let caller = ic_cdk::caller();

    if caller == market_details.vault_id {
        _remove_account_error_log(&account);
        return;
    }
}
/////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////
/// Getter Functions
///////////////////////////////////////////////////////////////////////////////////////////////////////

fn _get_market_details() -> MarketDetails {
    MARKET_DETAILS.with(|ref_market_details| ref_market_details.borrow().get().clone())
}

fn _get_state_details() -> StateDetails {
    STATE_DETAILS.with(|ref_state_detaills| *ref_state_detaills.borrow().get())
}

fn _get_account_position(account: &Subaccount) -> PositionDetails {
    ACCOUNTS_POSITION
        .with(|ref_position_details| ref_position_details.borrow().get(&account).unwrap())
}

fn _get_account_error_log(account: &Subaccount) -> PositionUpdateErrorLog {
    ACCOUNTS_ERROR_LOGS.with_borrow(|reference| reference.get(account).unwrap())
}

fn _get_pending_timer() -> TimerId {
    PENDING_TIMER.with_borrow_mut(|reference| reference.clone())
}

////////////////////////////////////////////////////////////////////////////////////////////////////
/// ////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////
/// /////////////////////////////////////////////////////////////////////////////////////////////////
///   Setter Function
//////////////////////////////////////////////////////////////////////////////////////////////////////
fn _set_state_details(new_state: StateDetails) {
    STATE_DETAILS.with(|ref_state_details| ref_state_details.borrow_mut().set(new_state).unwrap());
}

fn _insert_account_position(account: Subaccount, position: PositionDetails) {
    ACCOUNTS_POSITION
        .with(|ref_users_position| ref_users_position.borrow_mut().insert(account, position));
}

fn _remove_account_position(account: &Subaccount) {
    ACCOUNTS_POSITION.with(|ref_user_position| ref_user_position.borrow_mut().remove(account));
}

fn _insert_account_error_log(account: Subaccount, error_log: PositionUpdateErrorLog) {
    ACCOUNTS_ERROR_LOGS.with_borrow_mut(|reference| reference.insert(account, error_log));
}

fn _remove_account_error_log(account: &Subaccount) {
    ACCOUNTS_ERROR_LOGS.with_borrow_mut(|reference| reference.remove(account));
}

fn _has_position_or_pending_error_log(account: &Subaccount) -> bool {
    let has_position = ACCOUNTS_POSITION.with_borrow(|reference| reference.contains_key(account));
    let has_pending_error =
        ACCOUNTS_ERROR_LOGS.with_borrow(|reference| reference.contains_key(account));

    return has_pending_error || has_position;
}

fn _set_pending_timer(timer_id: TimerId) {
    PENDING_TIMER.with_borrow_mut(|reference| {
        *reference = timer_id;
    })
}
////////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(CandidType, Deserialize, Debug, Serialize, Clone, Copy)]
enum OrderType {
    Market,
    Limit,
}

#[derive(CandidType, Deserialize, Debug, Serialize, Clone, Copy)]
enum PositionOrderType {
    Market,
    Limit(LimitOrder),
}

#[derive(CandidType, Deserialize, Debug, Clone, Copy)]
struct PositionDetails {
    /// Entry Tick
    ///
    /// The tick at which position is opened
    entry_tick: Tick,
    /// true if long
    long: bool,
    /// Collatreal Value
    ///
    /// collatreal within position
    collateral_value: Amount,
    /// Debt
    ///
    /// the amount borrowed as leveragex10
    ///
    /// Note:debt is in perp Asset when shorting and in collateral_value asset when longing
    debt_value: Amount,
    // /// PositionDetails Size
    // ///
    // /// The amount of asset in position
    // ///
    // /// This can either be
    // ///
    // ///  - The amount resulting from the _swap when opening a position or
    // ///  - The amount used to gotten from opening placing order at a tick in the case of an order type
    // position_size: Amount,
    /// Volume Share
    ///
    ///Measure of liqudiity share in position with respect to the net amount in all open position of same direction i.e
    /// LONG or SHORT
    volume_share: Amount,
    /// Intrerest Rate
    ///
    /// Current interest rate for opening a position with margin
    ///
    interest_rate: u32,
    ///Order Type
    ///
    ///Position Order  type can either be a
    ///
    /// Market
    ///  - This is when position is opened instantly at the current price
    ///
    /// Order
    ///   - This comprises of an order set at a particular tick and position is only opened when
    ///   that  order has been executed
    order_type: PositionOrderType,

    /// TimeStamp
    ///
    /// timestamp when psotion was executed opened
    /// Tnis corresponds to the start time for  calculating interest rate on a leveraged position
    ///
    /// Note: For order type, position this  is time  order was excuted
    timestamp: Time,
}

impl Storable for PositionDetails {
    const BOUND: Bound = Bound::Bounded {
        max_size: 200,
        is_fixed_size: false,
    };
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

/// ManageDebtParams is utilised to handle debt handling and  repayment
#[derive(Copy, Clone, Default, Deserialize, CandidType)]
struct ManageDebtParams {
    initial_debt: Amount,
    net_debt: Amount,
    amount_repaid: Amount,
}

impl ManageDebtParams {
    fn init(initial_debt: Amount, net_debt: Amount, amount_repaid: Amount) -> Self {
        ManageDebtParams {
            initial_debt,
            net_debt,
            amount_repaid,
        }
    }
}

/////////////////////////////
///   Possible error during inter canister calls and retry api
////////////////////////////

/// Retrying Trait
///
/// Trait for all Errors related to inter canister calls
trait Retrying {
    /// Retry  Function
    ///
    /// This is used to retry the  failed inter canister call
    fn retry(&self, details: MarketDetails);
}

/// ManageDebtError
///
/// This error occurs for failed intercanister calls
#[derive(Clone, Copy, Deserialize, CandidType)]
struct PositionUpdateErrorLog {
    user: Principal,
    profit: Amount,
    debt_params: ManageDebtParams,
}
impl Retrying for PositionUpdateErrorLog {
    fn retry(&self, details: MarketDetails) {
        let _ = ic_cdk::notify(
            details.vault_id,
            "managePositionUpdate",
            (self.user, self.profit, self.debt_params),
        );
    }
}

impl Storable for PositionUpdateErrorLog {
    const BOUND: Bound = Bound::Bounded {
        max_size: 130,
        is_fixed_size: false,
    };
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
}

/// Exchange Rate Canister
///
/// Utilised for fetching the price of current exchnage rate (spot price) of the market pair
struct XRC {
    canister_id: Principal,
}

impl XRC {
    fn init(canister_id: Principal) -> Self {
        XRC { canister_id }
    }

    /// tries to fetch the current exchange rate of the pair and returns the result
    async fn _get_exchange_rate(&self, request: GetExchangeRateRequest) -> GetExchangeRateResult {
        if let Ok((rate_result,)) = ic_cdk::api::call::call_with_payment128(
            self.canister_id,
            "get_exchange_rate",
            (request,),
            1_000_000_000,
        )
        .await
        {
            return rate_result;
        } else {
            panic!()
        }
    }
}

/// The Vault type representing vault canister that stores asset for the entire collateral's denominated market
/// it facilitates all movement of assets including collection and repayment of debt utilised for leverage
#[derive(Clone, Copy)]
struct Vault {
    canister_id: Principal,
}

impl Vault {
    // initialises the vault canister
    pub fn init(canister_id: Principal) -> Self {
        Vault { canister_id }
    }

    /// Manage Position Update
    ///
    /// Utilised when position is updated or closed
    /// Utilised when for updating user_balance,repayment of debt
    pub fn manage_position_update(
        &self,
        user: Principal,
        profit: Amount,
        manage_debt_params: ManageDebtParams,
    ) {
        if let Ok(()) = ic_cdk::notify(
            self.canister_id,
            "managePositionUpdate",
            (user, profit, manage_debt_params),
        ) {
        } else {
            let error_log = PositionUpdateErrorLog {
                user,
                profit,
                debt_params: manage_debt_params,
            };
            _insert_account_error_log(user._to_subaccount(), error_log);
        }
    }

    /// Create Position Validity Check
    ///
    /// Checks if position can be opened by checking that uswer has sufficient balance and amount to use as debt is available as free liquidity
    ///
    /// User:The Owner of Account that opened position
    /// Collateral Delta:The Amount of asset used as collateral for opening position
    /// Debt : The Amount of asset taken as debt
    ///
    /// Note :After checking that the condition holds ,the user balance is reduced by collateral amount and the free liquidity available is reduced by debt amount

    pub async fn create_position_validity_check(
        &self,
        user: Principal,
        collateral: Amount,
        debt: Amount,
    ) -> (bool, u32) {
        if let Ok((valid, interest_rate)) = ic_cdk::call(
            self.canister_id,
            "createPositionValidityCheck",
            (user, collateral, debt),
        )
        .await
        {
            return (valid, interest_rate);
        } else {
            return (false, 0);
        }
    }
}

trait UniqueSubAccount {
    const NONCE: u8;
    fn _to_subaccount(&self) -> Subaccount;
}

impl UniqueSubAccount for Principal {
    const NONCE: u8 = 1;
    fn _to_subaccount(&self) -> Subaccount {
        let mut hasher = Sha256::new();
        hasher.update(self.as_slice());
        hasher.update(&Principal::NONCE.to_be_bytes());
        let hash = hasher.finalize();
        let mut subaccount = [0u8; 32];
        subaccount.copy_from_slice(&hash[..32]);
        subaccount
    }
}

export_candid!();

pub mod corelib;
pub mod types;

#[cfg(test)]
pub mod integration_tests;
