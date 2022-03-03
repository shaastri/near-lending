pub const ORACLE: &str = "oracle.tieubaoca.testnet";
pub const INTEREST_DIVISOR: u128 = 10_000;
pub const PRICE_DIVISOR: f64 = 10_000f64;
pub const SHARE_DIVISOR: Balance = 1_000_000_000_000;
pub const ONE_DAY: Timestamp = 86_400_000_000_000;
pub const ORACLE_DATA_EXPIRATION: Timestamp = 600_000_000_000;
pub const MAX_BORROW_RATE: u128 = 50;
pub const BORROW_RATE_DIVISOR: Balance = 100;
pub const LIQUIDATE_THRESHOLD: u128 = 65;
pub const LIQUIDATOR_INCENTIVE: u128 = 5;
pub const MAX_LIQUIDATE_RATE: u128 = 50;
pub const ERR_ORACLE_DATA_EXPIRED: &str = "ERR_ORACLE_DATA_EXPIRED";
pub const ERR_NO_POOL: &str = "ERR_NO_POOL";
pub const ERR_NO_BORROWER: &str = "ERR_NO_BORROWER";
pub const ERR_BORROW_VALUE_LIMITED: &str = "ERR_BORROW_VALUE_LIMITED";
pub const WRONG_FORMAT_PROMISE_RESULT: &'static [u8] = b"ERR_WRONG_VAL_RECEIVED";
pub const PROMISE_NOT_SUCCESSFUL: &'static [u8] = b"ERR_PROMISE_NOT_SUCCESSFUL";
use uint::construct_uint;

pub type Share = u128;
use crate::*;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
#[ext_contract(ft_contract)]
trait TFT {
    fn ft_transfer(&mut self, receiver_id: ValidAccountId, amount: U128, memo: Option<String>);
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128>;
}

#[ext_contract(oracle_contract)]
trait TOracle {
    fn get_data_response(&self, request_id: String) -> Option<Response>;
}

#[ext_contract(self_contract)]
pub trait TSelf {
    fn check_claim_success(&mut self, pool_id: u64, lender: AccountId);
    fn check_withdraw_success(
        &mut self,
        pool_id: u64,
        lender: AccountId,
        amount: U128,
        interest: U128,
    );
    fn update_borrower(&mut self, pool_id: u64, borrower: AccountId, amount: U128);
    fn check_borrowable(
        &mut self,
        borrower_id: AccountId,
        pool_id: u64,
        amount: U128,
        loans: Vec<Loan>,
        deposits: Vec<LenderInfo>,
    );
    fn liquidate(
        &mut self,
        liquidator: AccountId,
        pool_id: u64,
        amount: Balance,
        borrower_id: AccountId,
    ) -> PromiseOrValue<U128>;
    fn liquidate_callback(
        &mut self,
        pool_id: u64,
        amount_deposit: Balance,
        remain_amount: Balance,
        amount_collateral_out: Balance,
        borrower_id: AccountId,
    ) -> PromiseOrValue<U128>;
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault, Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Response {
    pub result: String,
    pub timestamp: Timestamp,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct TransferPayload {
    pub transfer_type: TransferType, //"Deposit", "Repay", "Mortgate", "Liquidate"
    pub borrower_id: Option<AccountId>, // Require once deposit to liquidate asset of borrower
    pub token: AccountId,
    pub pool_id: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum TransferType {
    Deposit,
    Repay,
    Liquidate,
}
