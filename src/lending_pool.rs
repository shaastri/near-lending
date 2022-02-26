use crate::utils::{
    Share, ERR_NO_BORROWER, INTEREST_DIVISOR, LIQUIDATE_THRESHOLD, LIQUIDATOR_INCENTIVE,
    MAX_LIQUADATE_RATE, ONE_DAY, SHARE_DIVISOR,
};
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
    pub lending_token: AccountId,
    pub borrower: AccountId,
    pub loan_start_time: Timestamp,
    pub amount: Balance,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LenderInfo {
    pub lending_token: AccountId,
    pub share: Share,
    pub reward_debt: Balance,
    pub acc_reward: Balance,
}

impl LendingPool {
    pub fn update_pool(&mut self) {
        if self.total_share == 0 {
            self.lastest_reward_time = env::block_timestamp();
            return;
        }
        let pending_reward = self.borrowers.values().fold(0, |acc, borrower| {
            acc + self.get_pending_interest(&borrower)
        });
        self.reward_per_share += (U256::from(pending_reward) * U256::from(SHARE_DIVISOR)
            / U256::from(self.total_share))
        .as_u128();
        self.lastest_reward_time = env::block_timestamp();
    }

    pub fn deposit(&mut self, lender_id: AccountId, amount: Balance) {
        self.update_pool();
        let lending_token = self.lending_token.clone();
        let mut lender = self.lenders.get(&lender_id).unwrap_or(LenderInfo {
            lending_token,
            share: 0,
            reward_debt: 0,
            acc_reward: 0,
        });
        if lender.share > 0 {
            let pending = self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt;
            lender.acc_reward += pending;
        }
        lender.share += amount;
        lender.reward_debt = self.reward_per_share * lender.share / SHARE_DIVISOR;
        self.lenders.insert(&lender_id, &lender);
        self.pool_supply += amount;
        self.total_share += amount;
    }

    pub fn borrow(&mut self, borrower_id: &AccountId, amount: Balance) {
        assert!(
            amount <= self.pool_supply - self.amount_borrowed,
            "Dont enough token to borrow from pool"
        );
        self.update_pool();
        let lending_token = self.lending_token.clone();
        let mut borrower = self.borrowers.get(&borrower_id).unwrap_or(Loan {
            lending_token,
            amount: 0,
            borrower: borrower_id.clone(),
            loan_start_time: env::block_timestamp(),
        });
        let mut interest = 0;
        if borrower.amount > 0 {
            interest = self.get_interest(&borrower);
            borrower.loan_start_time = env::block_timestamp();
        }
        borrower.amount += amount + interest;
        self.amount_borrowed += amount;
        self.pool_supply -= amount;
        self.borrowers.insert(&borrower_id, &borrower);
    }

    pub fn repay(&mut self, borrower_id: AccountId, amount: Balance) -> Balance {
        self.update_pool();
        let mut borrower = self
            .borrowers
            .get(&borrower_id)
            .expect("You have not borrowed anything yet");
        let interest = self.get_interest(&borrower);
        assert!(
            amount >= interest,
            "Amount repay must be greater than interest"
        );
        if amount >= (borrower.amount + interest) {
            self.pool_supply += borrower.amount + interest;
            self.borrowers.remove(&borrower_id);
            self.amount_borrowed -= amount - (borrower.amount + interest);
            amount - (borrower.amount + interest)
        } else {
            borrower.amount -= amount - interest;
            borrower.loan_start_time = env::block_timestamp();
            self.pool_supply += amount;
            self.amount_borrowed -= amount;
            self.borrowers.insert(&borrower_id, &borrower);
            0
        }
    }

    pub fn withdraw(&mut self, lender_id: AccountId, amount: Balance, interest: Balance) {
        self.update_pool();
        let mut lender = self.lenders.get(&lender_id).unwrap();
        self.pool_supply -= amount + interest;
        lender.acc_reward = 0;
        lender.share -= amount;
        lender.reward_debt = self.reward_per_share * lender.share / SHARE_DIVISOR;
        self.total_share -= amount;
        self.lenders.insert(&lender_id, &lender);
    }

    pub fn claim(&mut self, lender_id: AccountId) {
        self.update_pool();
        let mut lender = self.lenders.get(&lender_id).unwrap();
        self.pool_supply -= (U256::from(self.reward_per_share) * U256::from(lender.share)
            / SHARE_DIVISOR)
            .as_u128()
            - lender.reward_debt
            + lender.acc_reward;
        lender.reward_debt = (U256::from(self.reward_per_share) * U256::from(lender.share)
            / SHARE_DIVISOR)
            .as_u128();
        lender.acc_reward = 0;
        self.lenders.insert(&lender_id, &lender);
    }

    pub fn liquidate(
        &mut self,
        borrower_id: AccountId,
        amount_deposit: Balance,
        amount_collateral_token_out: Balance,
    ) {
    }

    pub fn amount_claimable(&self, lender_id: &AccountId) -> Balance {
        if let Some(lender) = self.lenders.get(&lender_id) {
            let pending_reward = self.get_pending_reward();
            let reward_per_share = self.reward_per_share
                + (U256::from(pending_reward) * U256::from(SHARE_DIVISOR)
                    / U256::from(self.total_share))
                .as_u128();
            reward_per_share * lender.share / SHARE_DIVISOR + lender.acc_reward - lender.reward_debt
        } else {
            0
        }
    }

    pub fn get_pending_reward(&self) -> Balance {
        self.borrowers.values().fold(0, |acc, borrower| {
            acc + self.get_pending_interest(&borrower)
        })
    }

    pub fn get_pending_interest(&self, borrower: &Loan) -> Balance {
        (U256::from(self.interest_rate)
            * U256::from(env::block_timestamp() - self.lastest_reward_time)
            * U256::from(borrower.amount)
            / U256::from(ONE_DAY)
            / U256::from(365u128)
            / U256::from(INTEREST_DIVISOR))
        .as_u128()
    }

    pub fn get_interest(&self, borrower: &Loan) -> Balance {
        (U256::from(self.interest_rate)
            * U256::from(env::block_timestamp() - borrower.loan_start_time)
            * U256::from(borrower.amount)
            / U256::from(ONE_DAY)
            / U256::from(365u128)
            / U256::from(INTEREST_DIVISOR))
        .as_u128()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};
    use near_sdk::{AccountId, Balance};

    fn get_context(
        _account_id: String,
        block_timestamp: Timestamp,
        attached_deposit: Balance,
    ) -> VMContext {
        VMContext {
            current_account_id: _account_id.clone(),
            signer_account_id: _account_id.clone(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id: _account_id,
            input: vec![],
            block_index: 0,
            block_timestamp,
            account_balance: 1_00_000_000_000_000_000_000_000_000,
            account_locked_balance: 0,
            storage_usage: 1000_000_000,
            attached_deposit,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    #[test]
    fn test_lending_pool() {
        let context = get_context(String::from("bob.near"), 0, 0);
        let deposit_amount: Balance = 1000_000_000_000;
        let borrow_amount: Balance = 1_000_000_000;
        testing_env!(context.clone());
        let mut lending_pool = LendingPool {
            pool_id: 0,
            lending_token: String::from("test-token"),
            interest_rate: 2000,
            pool_supply: 0,
            amount_borrowed: 0,
            borrowers: UnorderedMap::new(b"borrowers".to_vec()),
            lenders: UnorderedMap::new(b"lenders".to_vec()),
            total_share: 0,
            reward_per_share: 0,
            lastest_reward_time: context.block_timestamp,
        };
        //lender deposit at day 0
        lending_pool.deposit(String::from("lender.near"), deposit_amount);
        assert_eq!(lending_pool.pool_supply, deposit_amount, "total supply err");
        let lender = lending_pool
            .lenders
            .get(&String::from("lender.near"))
            .unwrap();
        assert_eq!(lender.share, deposit_amount, "err lender share");
        //bob borrowed at day 0
        lending_pool.borrow(&String::from("bob.near"), borrow_amount);
        assert_eq!(
            lending_pool.pool_supply,
            deposit_amount - borrow_amount,
            "total supply after borrowed err"
        );
        assert_eq!(
            lending_pool.amount_borrowed, borrow_amount,
            "err amount borrowed"
        );
        let loan = lending_pool
            .borrowers
            .get(&String::from("bob.near"))
            .unwrap();
        assert_eq!(loan.amount, borrow_amount, "err loan");
        assert_eq!(loan.loan_start_time, 0, "err loan");
        assert_eq!(loan.borrower, String::from("bob.near"), "err loan");

        // bob's interest at day 10
        let context = get_context(String::from("bob.near"), ONE_DAY * 10, 0);
        testing_env!(context.clone());

        let interest = lending_pool.get_interest(&loan);
        assert_eq!(interest, 5_479_452, "err interest"); //1_000_000_000 * 0.2 / 365 * 10
        assert_eq!(
            interest,
            lending_pool.get_pending_reward(),
            "Err pending reward"
        );
        assert_eq!(
            interest,
            lending_pool.amount_claimable(&String::from("lender.near")),
            "err amount claimable"
        );

        //lender 2 deposited at day 10
        lending_pool.deposit(String::from("lender2.near"), deposit_amount);

        assert_eq!(
            lending_pool.reward_per_share,
            (U256::from(interest) * U256::from(SHARE_DIVISOR) / U256::from(deposit_amount))
                .as_u128()
        );

        // day 20
        let context = get_context(String::from("bob.near"), ONE_DAY * 20, 0);
        testing_env!(context.clone());
        assert_eq!(
            interest,
            lending_pool.get_pending_reward(),
            "Err pending reward"
        );
        assert_eq!(
            lending_pool.get_interest(&loan),
            5_479_452 * 2,
            "err interest"
        );

        assert_eq!(
            interest,
            lending_pool.get_pending_reward(),
            "Err pending reward"
        );

        // day 0 -> day 10, lender's shares = 100% pool
        // day 10 -> day 20, lender 50%, lender 2 50%
        assert_eq!(
            interest + interest / 2,
            lending_pool.amount_claimable(&String::from("lender.near")),
            "err amount claimable"
        );

        assert_eq!(
            interest / 2,
            lending_pool.amount_claimable(&String::from("lender2.near")),
            "err amount claimable"
        );

        lending_pool.claim(String::from("lender.near"));

        assert_eq!(
            lending_pool.amount_claimable(&String::from("lender.near")),
            0,
            "Err after claim"
        );

        assert_eq!(
            lending_pool.pool_supply as u128,
            deposit_amount * 2 - (interest + interest / 2) - borrow_amount,
            "Err pool supply after claim"
        );
        //alice borrowed at day 20
        lending_pool.borrow(&String::from("alice.near"), borrow_amount);

        //day 30
        let context = get_context(String::from("bob.near"), ONE_DAY * 30, 0);
        testing_env!(context.clone());

        assert_eq!(lending_pool.amount_borrowed, borrow_amount * 2);
        //from day 20 - day 30: interest is diveded equally to 2 lender and lender 2.
        assert_eq!(
            lending_pool.amount_claimable(&String::from("lender.near")),
            interest
        );

        let lender2_interest = lending_pool.amount_claimable(&String::from("lender2.near"));

        assert_eq!(lender2_interest, 3 * interest / 2);

        lending_pool.withdraw(
            String::from("lender2.near"),
            deposit_amount / 2,
            lender2_interest,
        );

        assert_eq!(
            lending_pool.pool_supply,
            deposit_amount * 2 - 3 * interest - deposit_amount / 2 - lending_pool.amount_borrowed
        );

        //day 40
        // day 30 -> 40, lender 2/3, lender 2 1/3
        // borrowed amount = 2 * borrow_amount -> interest * 2
        let context = get_context(String::from("bob.near"), ONE_DAY * 40, 0);
        testing_env!(context.clone());

        assert_eq!(
            lending_pool.amount_claimable(&String::from("lender2.near")),
            2 * interest / 3
        );
    }
}
