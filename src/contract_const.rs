pub const REF_FINANCE: &str = "ref-finance.testnet";
pub const INTEREST_DIVISOR: u64 = 10_000;
pub const SHARE_DIVISOR: Balance = 1_000_000_000_000;
pub const ONE_DAY: Timestamp = 60_000_000_000;
pub const ERR_NO_POOL:&str = "ERR_NO_POOL";
pub type Share = u128;
use crate::*;

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
    fn check_claim_success(&mut self, pool_id: u64, lender: AccountId);
    fn check_withdraw_success(&mut self, pool_id: u64, lender: AccountId, amount: U128);
    fn update_borrower(&mut self, pool_id: u64, borrower: AccountId, amount: U128);
}

/// Single swap action.
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
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