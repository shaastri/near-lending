pub const REF_FINANCE: &str = "ref-finance.testnet";
pub const REF_FEE_DIVISOR: u32 = 10_000;
pub const INTEREST_DIVISOR: u128 = 10_000;
pub const SHARE_DIVISOR: Balance = 1_000_000_000_000;
pub const ONE_DAY: Timestamp = 60_000_000_000;
pub const MAX_BORROW_RATE: Balance= 50;
pub const LIQUIDATE_THRESHOLD: Balance = 65;
pub const ERR_NO_POOL:&str = "ERR_NO_POOL";
pub const ERR_NO_BORROWER:&str = "ERR_NO_BORROWER";
pub const WNEAR: &str = "wrap.testnet";
use uint::construct_uint;

pub type Share = u128;
use crate::*;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
#[ext_contract(ft_contract)]
trait TFT {
    fn ft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        memo: Option<String>
    );
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        memo: Option<String>,
        msg: String
    ) -> PromiseOrValue<U128>;
}

#[ext_contract(ref_contract)]
trait TRefFinance{
    #[payable]
    fn swap(&mut self, actions: Vec<SwapAction>, referral_id: Option<ValidAccountId>) -> U128;
    fn get_pool(&self, pool_id: u64) -> PoolInfo;
    fn get_return(
        &self,
        pool_id: u64,
        token_in: ValidAccountId,
        amount_in: U128,
        token_out: ValidAccountId,
    ) -> U128;
}

#[ext_contract(self_contract)]
pub trait TSelf{
    fn get_ref_pool_callback(&mut self, pool_id: u64, borrower_id: AccountId, amount: Balance) -> Promise;
    fn check_claim_success(&mut self, pool_id: u64, lender: AccountId);
    fn check_withdraw_success(&mut self, pool_id: u64, lender: AccountId, amount: U128);
    fn update_borrower(&mut self, pool_id: u64, borrower: AccountId, amount: U128);
}

/// Single swap action.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct SwapAction {
    /// Pool which should be used for swapping.
    pub pool_id: u64,
    /// Token to swap from.
    pub token_in: AccountId,
    /// Amount to exchange.
    /// If amount_in is None, it will take amount_out from previous step.
    /// Will fail if amount_in is None on the first step.
    pub amount_in: Option<U128>,
    /// Token to swap into.
    pub token_out: AccountId,
    /// Required minimum amount of token_out.
    pub min_amount_out: U128,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct PoolInfo {
    /// Pool kind.
    pub pool_kind: String,
    /// List of tokens in the pool.
    pub token_account_ids: Vec<AccountId>,
    /// How much NEAR this contract has.
    pub amounts: Vec<U128>,
    /// Fee charged for swap.
    pub total_fee: u32,
    /// Total number of shares.
    pub shares_total_supply: U128,
    //pub amp: u64,
}

impl PoolInfo {
    /// Returns token index for given pool.
    fn token_index(&self, token_id: &AccountId) -> usize {
        self.token_account_ids
            .iter()
            .position(|id| id == token_id)
            .expect("ERR_MISSING_TOKEN")
    }

    /// Returns number of tokens in outcome, given amount.
    /// Tokens are provided as indexes into token list for given pool.
    fn internal_get_return(
        &self,
        token_in: usize,
        amount_in: Balance,
        token_out: usize,
    ) -> Balance {
        let in_balance = U256::from(Balance::from(self.amounts[token_in]));
        let out_balance = U256::from(Balance::from(self.amounts[token_out]));
        assert!(
            in_balance > U256::zero()
                && out_balance > U256::zero()
                && token_in != token_out
                && amount_in > 0,
            "ERR_INVALID"
        );
        let amount_with_fee = U256::from(amount_in) * U256::from(REF_FEE_DIVISOR - self.total_fee);
        (amount_with_fee * out_balance / (U256::from(REF_FEE_DIVISOR) * in_balance + amount_with_fee))
            .as_u128()
    }

    /// Returns how much token you will receive if swap `token_amount_in` of `token_in` for `token_out`.
    pub fn get_return(
        &self,
        token_in: &AccountId,
        amount_in: Balance,
        token_out: &AccountId,
    ) -> Balance {
        self.internal_get_return(
            self.token_index(token_in),
            amount_in,
            self.token_index(token_out),
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct TransferPayload{
    pub transfer_type: TransferType,//"deposit", "repay", "borrow"
    pub token: AccountId,
    pub pool_id: u64
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum TransferType{
    Deposit,
    Repay,
    Mortgate
}