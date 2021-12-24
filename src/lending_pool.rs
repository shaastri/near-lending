use crate::contract_const::{Share, INTEREST_DIVISOR, ONE_DAY, SHARE_DIVISOR};
use crate::*;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LendingPool {
    pub pool_id: u64,
    pub lending_token: AccountId,
    pub interest_rate: u64,
    pub pool_supply: Balance,
    pub amount_borrowed: Balance,
    pub borrowers: UnorderedMap<AccountId, Loan>,
    pub lenders: UnorderedMap<AccountId, LenderInfo>,
    pub total_share: Share,
    pub reward_per_share: Balance,
    pub lastest_reward_time: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Loan {
    loan_start_time: Timestamp,
    amount: Balance,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LenderInfo {
    share: Share,
    reward_debt: Balance,
    acc_reward: Balance,
}

impl LendingPool {
    pub fn update_pool(&mut self) {
        if self.total_share == 0 {
            self.lastest_reward_time = env::block_timestamp();
            return;
        }
        let pendding_reward = self.borrowers.values().fold(0, |acc, borrower| {
            acc + ((env::block_timestamp() - self.lastest_reward_time) / ONE_DAY) as Balance
                * borrower.amount
                * self.interest_rate as Balance
                / 365
                / INTEREST_DIVISOR as Balance
        });
        self.reward_per_share += pendding_reward / (self.total_share / SHARE_DIVISOR);
        self.lastest_reward_time = env::block_timestamp();
    }

    pub fn deposit(&mut self, lender_id: AccountId, amount: Balance) {
        self.update_pool();
        let mut lender = self.lenders.get(&lender_id).unwrap_or(LenderInfo {
            share: 0,
            reward_debt: 0,
            acc_reward: 0,
        });
        if amount > 0 {
            let pending = self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt;
            lender.acc_reward += pending;
        }
        lender.reward_debt = self.reward_per_share * lender.share / SHARE_DIVISOR;
        lender.share += amount;
        self.lenders.insert(&lender_id, &lender);
        self.pool_supply += amount;
        self.total_share += amount;
    }

    pub fn borrow(&mut self, borrower_id: AccountId, amount: Balance) {
        assert!(
            amount <= self.pool_supply - self.amount_borrowed,
            "Dont enough token to borrow from pool"
        );
        self.update_pool();
        let mut borrower = self.borrowers.get(&borrower_id).unwrap_or(Loan {
            loan_start_time: env::block_timestamp(),
            amount: 0u128,
        });
        let interest = (SHARE_DIVISOR as u64
            * self.interest_rate
            * (env::block_timestamp() - borrower.loan_start_time)
            / ONE_DAY
            / 365
            / INTEREST_DIVISOR) as Balance;
        borrower.amount += amount + amount * interest / SHARE_DIVISOR;
        self.pool_supply -= amount;
        self.borrowers.insert(&borrower_id, &borrower);
    }

    pub fn repay(&mut self, borrower_id: AccountId, amount: Balance) -> Balance {
        self.update_pool();
        let mut borrower = self
            .borrowers
            .get(&borrower_id)
            .expect("You have not borrowed anything yet");
        let interest = (SHARE_DIVISOR as u64
            * self.interest_rate
            * (env::block_timestamp() - borrower.loan_start_time)
            / ONE_DAY
            / 365
            / INTEREST_DIVISOR) as Balance;
        assert!(
            amount >= amount * interest / SHARE_DIVISOR,
            "Amount repay must be greater than interest"
        );
        if amount >= (borrower.amount + amount * interest / SHARE_DIVISOR) {
            self.pool_supply += borrower.amount + amount * interest / SHARE_DIVISOR;
            self.borrowers.remove(&borrower_id);
            amount - (borrower.amount + amount * interest / SHARE_DIVISOR)
        } else {
            borrower.amount -= (amount - amount * interest / SHARE_DIVISOR);
            borrower.loan_start_time = env::block_timestamp();
            self.pool_supply += amount;
            self.borrowers.insert(&borrower_id, &borrower);
            0
        }
    }

    pub fn claim_reward(&mut self, lender_id: AccountId) -> Promise {
        self.update_pool();
        let mut lender = self.lenders.get(&lender_id).expect("Nothing to claim");
        self.lenders.insert(&lender_id, &lender);
        ft_contract::ft_transfer(
            ValidAccountId::try_from(lender_id.clone()).unwrap(),
            U128::from(
                self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt
                    + lender.acc_reward,
            ),
            None,
            &self.lending_token,
            1,
            10_000_000_000_000,
        )
        .then(self_contract::check_claim_success(
            self.pool_id,
            lender_id,
            &env::current_account_id(),
            0,
            5_000_000_000_000,
        ))
    }

    pub fn withdraw(&mut self, lender_id: AccountId, amount: Balance) -> Promise {
        self.update_pool();
        let lender = self.lenders.get(&lender_id).expect("Nothing to claim");
        assert!(
            amount <= lender.share,
            "Amount withdraw is greater than your deposit"
        );
        ft_contract::ft_transfer(
            ValidAccountId::try_from(lender_id.clone()).unwrap(),
            U128::from(
                amount + self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt + lender.acc_reward,
            ),
            None,
            &self.lending_token,
            1,
            10_000_000_000_000,
        )
        .then(self_contract::check_withdraw_success(
            self.pool_id,
            lender_id,
            U128::from(amount),
            &env::current_account_id(),
            0,
            10_000_000_000_000,
        ))
    }

    pub fn update_lender_withdraw(&mut self, lender_id: AccountId, amount: Balance) {
        let mut lender = self.lenders.get(&lender_id).unwrap();
        self.pool_supply -= amount + self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt + lender.acc_reward;
        lender.acc_reward = 0;
        lender.share -= amount;
        lender.reward_debt = self.reward_per_share * lender.share / SHARE_DIVISOR;
        self.total_share -= amount;
        self.lenders.insert(&lender_id, &lender);
    }

    pub fn update_lender_claim(&mut self, lender_id: AccountId) {
        let mut lender = self.lenders.get(&lender_id).unwrap();
        self.pool_supply -= self.reward_per_share * lender.share / SHARE_DIVISOR
            - lender.reward_debt
            + lender.acc_reward;
        lender.reward_debt = self.reward_per_share * lender.share / SHARE_DIVISOR;
        lender.acc_reward = 0;
        self.lenders.insert(&lender_id, &lender);
    }

    pub fn amount_claimable(&self, lender_id: AccountId) -> Balance {
        if let Some(lender) = self.lenders.get(&lender_id) {
            let pendding_reward = self.borrowers.values().fold(0, |acc, borrower| {
                acc + ((env::block_timestamp() - self.lastest_reward_time) / ONE_DAY) as Balance
                    * borrower.amount
                    * self.interest_rate as Balance
                    / 365
                    / INTEREST_DIVISOR as Balance
            });
            let reward_per_share =
                self.reward_per_share + pendding_reward / (self.total_share / SHARE_DIVISOR);
            reward_per_share * lender.share / SHARE_DIVISOR + lender.acc_reward
        } else {
            0
        }
    }
}
