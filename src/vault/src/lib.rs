use std::cell::RefCell;

use candid::{CandidType, Deserialize, Principal};

use sha2::{Digest, Sha256};

use icrc_ledger_types::icrc1::account::{Account, Subaccount};

use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, StableCell};

use core_lib::staking::{StakeDetails, StakeSpan};
use types::VaultDetails;

type Memory = VirtualMemory<DefaultMemoryImpl>;
type Amount = u128;
type Time = u64;

const _VAULT_DETAILS_MEMORY_ID: MemoryId = MemoryId::new(1);
const _USERS_STAKES_DETAILS_MEMORY_ID: MemoryId = MemoryId::new(2);
const _USERS_MARGIN_BALANCE_MEMORY_ID: MemoryId = MemoryId::new(3);
const _APPROVED_MARKETS_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {

    static MEMORY_MANAGER:RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default())) ;


    static VAULT_DETAILS :RefCell<StableCell<VaultDetails,Memory>> = RefCell::new(StableCell::init(MEMORY_MANAGER.with_borrow(|reference|{
        reference.get(_VAULT_DETAILS_MEMORY_ID)
    }),VaultDetails::default()).unwrap());

    static APPROVED_MARKETS :RefCell<StableBTreeMap<Principal,Amount,Memory>> = RefCell::new(StableBTreeMap::init(
        MEMORY_MANAGER.with_borrow(|reference|{
        reference.get(_APPROVED_MARKETS_MEMORY_ID)
    })));


    static USERS_MARGIN_BALANCE :RefCell<StableBTreeMap<Principal,Amount,Memory>> = RefCell::new(StableBTreeMap::init(
        MEMORY_MANAGER.with_borrow(|reference|{
        reference.get(_USERS_MARGIN_BALANCE_MEMORY_ID)
    })));


    static USERS_STAKES :RefCell<StableBTreeMap<(Principal,Time),StakeDetails,Memory>> = RefCell::new(StableBTreeMap::init(
        MEMORY_MANAGER.with_borrow(|reference|{
        reference.get(_USERS_STAKES_DETAILS_MEMORY_ID)
    })));

}

#[ic_cdk::init]
fn init(vault_details: VaultDetails) {
    VAULT_DETAILS.with_borrow_mut(|reference| reference.set(vault_details).unwrap());
}

#[ic_cdk::query]
fn get_user_account(user: Principal) -> Account {
    return Account {
        owner: ic_cdk::id(),
        subaccount: Some(user._to_subaccount()),
    };
}

/// Create  Position Validity Check
///
///
/// This function ensures that all asset movement for opening a position is valid i.e
///
/// It checks that user has sufficient margin balance to use as  collateral
/// It checks that vault has enough staked liquidity to provide leverage
///
///  If both condtions are  true,the asset changes i.e user balance and free liquidity for leverage is updated (reduced by the specified amounts)
///
/// Returns
///  - Valid: True if the required changes are valid and false otherwise
///  - Interest_Rate:The Interest Rate for borrowing that amount at that time    

#[ic_cdk::update(name = "createPositionValidityCheck", guard = "approved_market_guard")]
async fn create_position_validity_check(
    user: Principal,
    collateral: Amount,
    debt: Amount,
) -> (bool, u32) {
    let account_balance = _get_user_balance(user);

    let mut vault_details = _get_vault_details();

    let valid = account_balance >= collateral && vault_details.free_liquidity >= debt;

    if valid {
        vault_details.free_liquidity -= debt;
        vault_details.debt += debt;
        _update_user_margin_balance(user, collateral, false);
    }

    _update_vault_details(vault_details);

    return (valid, 0);
}

/// Manage Position Update
///
/// This function is called when position is being updated(either fully closed or partially closed) and debt is being repaid either with or without any interest
///
/// Params;
///  - User :The user whose position was updated or fully closed
///  - Margin Delta : The amount to add back into user's margin balance  
///  - Manage Debt Params :The debt management paramters
///
/// Note : This function also updates the vault staking details distributing the fees gotten into the respective stake spans

#[ic_cdk::update(name = "managePositionUpdate", guard = "approved_market_guard")]
async fn manage_position_update(
    user: Principal,
    margin_delta: Amount,
    manage_debt_params: ManageDebtParams,
) {
    if margin_delta != 0 {
        _update_user_margin_balance(user, margin_delta, true);
    }

    let mut vault_details = _get_vault_details();

    let ManageDebtParams {
        initial_debt,
        net_debt,
        amount_repaid,
    } = &manage_debt_params;

    vault_details.debt = vault_details.debt + net_debt - (initial_debt + amount_repaid);
    vault_details.free_liquidity += amount_repaid;

    let fees_gotten = if amount_repaid > initial_debt {
        amount_repaid - initial_debt
    } else {
        0
    };
    if fees_gotten == 0 {
        return;
    }
    vault_details.lifetime_fees += fees_gotten;

    {
        vault_details.staking_details._create_stake(
            0,
            vault_details.lifetime_fees,
            StakeSpan::Instant,
        )
    };
    {
        vault_details.staking_details._create_stake(
            0,
            vault_details.lifetime_fees,
            StakeSpan::Month2,
        )
    };
    {
        vault_details.staking_details._create_stake(
            0,
            vault_details.lifetime_fees,
            StakeSpan::Month6,
        )
    };
    {
        vault_details
            .staking_details
            ._create_stake(0, vault_details.lifetime_fees, StakeSpan::Year)
    };
    _update_vault_details(vault_details);
}

/// Funds a Traders margin account to make a thread
///
///
/// Funds a user's margin accout with amount
///
/// Params
///  - Amount :The amount to deposit;
///  - For Principal :The principal whose margin account is being funded
#[ic_cdk::update]
async fn fund_margin_account(amount: Amount, for_principal: Principal) {
    let vault_details = _get_vault_details();
    assert!(amount >= vault_details.min_amount);

    let user = ic_cdk::caller();

    let token = vault_details.asset;
    if token
        .move_asset(amount, ic_cdk::id(), Some(user._to_subaccount()), None)
        .await
    {
        _update_user_margin_balance(for_principal, amount, true);
    }
}

/// Withdraw From Margin Account
///
/// defunds user's margin account  
///
/// Params
///  - Amount :The amount to withdraw from user's margin account

#[ic_cdk::update]
async fn withdraw_from_margin_account(amount: Amount) {
    let user = ic_cdk::caller();

    let vault_details = _get_vault_details();

    let amount_to_withdraw = if amount < vault_details.min_amount {
        _get_user_balance(user)
    } else {
        amount
    };

    let tx_fee = vault_details.tx_fee;
    if amount_to_withdraw - tx_fee == 0 {
        return;
    }

    let token = vault_details.asset;
    if token
        .move_asset(
            amount_to_withdraw - tx_fee,
            ic_cdk::id(),
            None,
            Some(user._to_subaccount()),
        )
        .await
    {
        _update_user_margin_balance(user, amount_to_withdraw, false);
    }
}

///////////////////////////
///  Stakers Functions
//////////////////////////

///
///
/// Provide Leverage Function
///
/// Primary function for depositing asset into vault to be borrowed by traders as leverage
///
///
/// Note:Function alsp creates a stake of the Instant stake span type
#[ic_cdk::update]
async fn provide_leverage(amount: Amount) {
    let user = ic_cdk::caller();
    //
    let mut vault_details = _get_vault_details();

    assert!(amount >= vault_details.min_amount);

    let token = vault_details.asset;

    if !(token
        .move_asset(amount, ic_cdk::id(), Some(user._to_subaccount()), None)
        .await)
    {
        return;
    }

    let vtoken = vault_details.virtaul_asset;
    // minting asset to user
    if !(vtoken
        .move_asset(amount, ic_cdk::id(), None, Some(user._to_subaccount()))
        .await)
    {
        token
            .move_asset(amount, ic_cdk::id(), None, Some(user._to_subaccount()))
            .await;
        return;
    }
    vault_details.free_liquidity += amount;

    let stake: StakeDetails = vault_details.staking_details._create_stake(
        amount,
        vault_details.lifetime_fees,
        StakeSpan::Instant,
    );
    _insert_user_stake(user, stake);
    _update_vault_details(vault_details);
}

///
///Remove Leverage Function
///
/// removes leverage and sends back that amount back into user's funding account
#[ic_cdk::update]
async fn remove_leverage(amount: Amount) {
    let user = ic_cdk::caller()._to_subaccount();
    let mut vault_details = _get_vault_details();

    assert!(amount >= vault_details.min_amount);
    // if tokens are not much
    if vault_details.free_liquidity < amount {
        return;
    }

    let vtoken = vault_details.virtaul_asset;
    // minting asset to user
    if !vtoken
        .move_asset(amount, ic_cdk::id(), Some(user), None)
        .await
    {
        return;
    }

    let token = vault_details.asset;

    let tx_fee = vault_details.tx_fee;

    if !(token
        .move_asset(amount - tx_fee, ic_cdk::id(), None, Some(user))
        .await)
    {
        // if asset can't be sent back
        // mint back
        vtoken
            .move_asset(amount, ic_cdk::id(), None, Some(user))
            .await;
        return;
    }

    vault_details.free_liquidity -= amount;
    _update_vault_details(vault_details);
}

/// Stake Function
///
/// Staking function for locking up vtokens for a any of the existing stake span
///
/// Params
///  - Amount :The Amount of vtoken to stake
///  - Stake Span :The specific stake duration
///
#[ic_cdk::update]
async fn stake(amount: Amount, stake_span: StakeSpan) {
    if let StakeSpan::Instant = stake_span {
        return;
    };
    let user = ic_cdk::caller();
    let mut vault_details = _get_vault_details();

    assert!(amount >= vault_details.min_amount);

    let vtoken = vault_details.virtaul_asset;
    // send in asset from user to account
    assert!(
        vtoken
            .move_asset(
                amount,
                ic_cdk::id(),
                Some(user._to_subaccount()),
                Some(_vault_subaccount())
            )
            .await
    );

    let stake = vault_details.staking_details._create_stake(
        amount,
        vault_details.lifetime_fees,
        stake_span,
    );

    _insert_user_stake(user, stake);
    _update_vault_details(vault_details);
}

/// Unstake Function
///
/// removes a  particular user's stake

#[ic_cdk::update]
async fn unstake(stake_timestamp: Time) -> Result<Amount, String> {
    let user = ic_cdk::caller();
    let ref_stake = _get_user_stake(user, stake_timestamp);

    if ic_cdk::api::time() < ref_stake.expiry_time {
        return Err("Expiry time in the future".to_string());
    };

    let mut vault_details = _get_vault_details();

    let amount_out = vault_details
        .staking_details
        ._close_stake(ref_stake, vault_details.lifetime_fees);

    let vtoken = vault_details.virtaul_asset;

    if !(vtoken
        .move_asset(
            amount_out,
            ic_cdk::id(),
            Some(_vault_subaccount()),
            Some(user._to_subaccount()),
        )
        .await)
    {
        return Err("failed".to_string());
    }
    _remove_user_stake(user, stake_timestamp);
    _update_vault_details(vault_details);

    return Ok(amount_out);
}

/// Update user balance

fn _update_user_margin_balance(user: Principal, delta: Amount, deposit: bool) {
    USERS_MARGIN_BALANCE.with_borrow_mut(|reference| {
        let initial_balance = { reference.get(&user).or(Some(0)).unwrap() };
        let new_balance = if deposit {
            initial_balance + delta
        } else {
            initial_balance - delta
        };
        if new_balance == 0 {
            reference.remove(&user)
        } else {
            reference.insert(user, new_balance)
        }
    });
}

fn _get_vault_details() -> VaultDetails {
    VAULT_DETAILS.with(|reference| reference.borrow().get().clone())
}

fn _update_vault_details(new_details: VaultDetails) {
    VAULT_DETAILS.with_borrow_mut(|reference| [reference.set(new_details).unwrap()]);
}

fn _get_user_balance(user: Principal) -> Amount {
    USERS_MARGIN_BALANCE.with_borrow_mut(|reference| {
        return reference.get(&user).or(Some(0)).unwrap();
    })
}

fn _get_user_stake(user: Principal, timestamp: Time) -> StakeDetails {
    USERS_STAKES.with_borrow(|reference| reference.get(&(user, timestamp)).unwrap())
}

fn _insert_user_stake(user: Principal, stake: StakeDetails) {
    let timestamp = ic_cdk::api::time();
    USERS_STAKES.with_borrow_mut(|reference| reference.insert((user, timestamp), stake));
}

fn _remove_user_stake(user: Principal, timestamp: Time) {
    USERS_STAKES.with_borrow_mut(|reference| reference.remove(&(user, timestamp)));
}

/// Approved Markets Guard
///
/// Ensures that only approved markets can call the specified functions
fn approved_market_guard() -> Result<(), String> {
    let caller = ic_cdk::caller();
    APPROVED_MARKETS.with_borrow(|reference| {
        if reference.contains_key(&caller) {
            return Ok(());
        } else {
            return Err("Caller not an approved market".to_string());
        }
    })
}

#[derive(Copy, Clone, Default, Deserialize, CandidType)]
struct ManageDebtParams {
    initial_debt: Amount,
    net_debt: Amount,
    amount_repaid: Amount,
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

fn _vault_subaccount() -> Subaccount {
    let canister_id = ic_cdk::caller();
    return canister_id._to_subaccount();
}

pub mod core_lib;
pub mod types;
