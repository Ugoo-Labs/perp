use candid::{CandidType, Deserialize, Nat, Principal};
use ic_cdk;

use icrc_ledger_types::icrc1::{
    account::{Account, Subaccount},
    transfer::{TransferArg as ICRCTransferrgs, TransferError},
};

use ic_ledger_types::{
    transfer, AccountIdentifier, Memo, Subaccount as ICSubaccount, Tokens, TransferArgs,
    DEFAULT_FEE, DEFAULT_SUBACCOUNT,
};
use serde::Serialize;

type Amount = u128;

#[derive(CandidType, Deserialize, Clone, Copy)]
pub enum AssetType {
    ICP,
    ICRC,
}

#[derive(CandidType, Deserialize, Clone, Copy)]
pub struct Asset {
    ledger_id: Principal,
    asset_type: AssetType,
}

impl Asset {
    pub async fn move_asset(
        &self,
        amount: Amount,
        principal: Principal,
        from_subaccount: Option<Subaccount>,
        to_subaccount: Option<Subaccount>,
    ) -> bool {
        match self.asset_type {
            AssetType::ICP => {
                return move_asset_icp(
                    amount,
                    self.ledger_id,
                    principal,
                    from_subaccount,
                    to_subaccount,
                )
                .await;
            }
            AssetType::ICRC => {
                return move_asset_icrc(
                    amount,
                    self.ledger_id,
                    principal,
                    from_subaccount,
                    to_subaccount,
                )
                .await;
            }
        }
        // moving asset
    }
}

impl Asset {}

impl Default for Asset {
    fn default() -> Self {
        return Asset {
            ledger_id: Principal::anonymous(),
            asset_type: AssetType::ICRC,
        };
    }
}

async fn move_asset_icp(
    amount: Amount,
    ledger_id: Principal,
    owner: Principal,
    from: Option<Subaccount>,
    to_sub: Option<Subaccount>,
) -> bool {
    if amount == 0 {
        return true;
    }

    let args = TransferArgs {
        amount: Tokens::from_e8s(amount as u64),
        memo: Memo(0),
        fee: DEFAULT_FEE,
        from_subaccount: Some(_to_ic_subaccount(from)),
        to: AccountIdentifier::new(&owner, &_to_ic_subaccount(to_sub)),
        created_at_time: None,
    };

    match transfer(ledger_id, args).await {
        Ok(res) => {
            if let Ok(_) = res {
                return true;
            } else {
                return false;
            }
        }
        Err(_) => return false,
    };
}

async fn move_asset_icrc(
    amount: Amount,
    ledger_id: Principal,
    owner: Principal,
    from: Option<Subaccount>,
    to_sub: Option<Subaccount>,
) -> bool {
    let args = ICRCTransferrgs {
        amount: Nat::from(amount),
        from_subaccount: from,
        to: Account {
            owner,
            subaccount: to_sub,
        },
        fee: None,
        created_at_time: None,
        memo: None,
    };

    let tx_result: Result<TransferResult, TransferError>;

    if let Ok((result,)) = ic_cdk::call(ledger_id, "icrc1_transfer", (args,)).await {
        tx_result = result;
        if let Ok(_) = tx_result {
            return true;
        } else {
            return false;
        }
    } else {
        return false;
    }
}

#[derive(CandidType, Deserialize, Serialize)]
struct TransferResult {
    block_index: Nat,
}

fn _to_ic_subaccount(sub: Option<Subaccount>) -> ICSubaccount {
    match sub {
        Some(res) => return ICSubaccount(res),
        None => return DEFAULT_SUBACCOUNT,
    }
}
