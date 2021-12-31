use crate::utils::{
    Share, ERR_NO_BORROWER, INTEREST_DIVISOR, LIQUIDATE_THRESHOLD, MAX_BORROW_RATE, ONE_DAY,
    SHARE_DIVISOR,
};
use crate::*;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LendingPool {
    pub pool_id: u64,
    pub lending_token: AccountId,
    pub collateral_token: AccountId,
    pub ref_pool_ids: Vec<u64>, //[lending token pool id, collateral token pool id] - pool wnear - token
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
    pub borrower: AccountId,
    pub loan_start_time: Timestamp,
    pub amount: Balance,
    pub amount_collateral: Balance,
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
        let pendding_reward = self
            .borrowers
            .values()
            .fold(0, |acc, borrower| acc + self.get_interest(&borrower));
        self.reward_per_share += (U256::from(pendding_reward) * U256::from(SHARE_DIVISOR)
            / U256::from(self.total_share))
        .as_u128();
        self.lastest_reward_time = env::block_timestamp();
    }

    pub fn deposit(&mut self, lender_id: AccountId, amount: Balance) {
        self.update_pool();
        let mut lender = self.lenders.get(&lender_id).unwrap_or(LenderInfo {
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

    pub fn mortgate(&mut self, borrower_id: AccountId, amount_collateral: Balance) -> Loan {
        let mut borrower = self.borrowers.get(&borrower_id).unwrap_or(Loan {
            borrower: borrower_id.clone(),
            loan_start_time: env::block_timestamp(),
            amount: 0u128,
            amount_collateral: 0u128,
        });
        borrower.amount_collateral += amount_collateral;
        self.borrowers.insert(&borrower_id, &borrower);
        borrower
    }

    pub fn withdraw_collateral(&mut self, borrower_id: AccountId, amount: Balance) -> Loan {
        let mut borrower = self.borrowers.get(&borrower_id).expect(ERR_NO_BORROWER);
        borrower.amount_collateral -= amount;
        self.borrowers.insert(&borrower_id, &borrower);
        borrower
    }

    pub fn borrow(&mut self, borrower_id: AccountId, amount: Balance) {
        assert!(
            amount <= self.pool_supply - self.amount_borrowed,
            "Dont enough token to borrow from pool"
        );
        self.update_pool();
        let mut borrower = self.borrowers.get(&borrower_id).expect(ERR_NO_BORROWER);
        let interest = self.get_interest(&borrower);
        borrower.amount += amount + interest / SHARE_DIVISOR;
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
            amount >= interest / SHARE_DIVISOR,
            "Amount repay must be greater than interest"
        );
        if amount >= (borrower.amount + interest / SHARE_DIVISOR) {
            self.pool_supply += borrower.amount + interest / SHARE_DIVISOR;
            self.borrowers.remove(&borrower_id);
            amount - (borrower.amount + interest / SHARE_DIVISOR)
        } else {
            borrower.amount -= (amount - interest / SHARE_DIVISOR);
            borrower.loan_start_time = env::block_timestamp();
            self.pool_supply += amount;
            self.borrowers.insert(&borrower_id, &borrower);
            0
        }
    }

    pub fn claim_reward(&mut self, lender_id: AccountId) -> Promise {
        self.update_pool();
        let lender = self.lenders.get(&lender_id).expect("Nothing to claim");
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
            10_000_000_000_000,
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
                amount + self.reward_per_share * lender.share / SHARE_DIVISOR - lender.reward_debt
                    + lender.acc_reward,
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
        self.pool_supply -= amount + self.reward_per_share * lender.share / SHARE_DIVISOR
            - lender.reward_debt
            + lender.acc_reward;
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

    pub fn get_list_liquidatable(&self, ref_pool: Vec<PoolInfo>) -> Vec<Loan> {
        //price: 10**24 collateral => return amount lending token
        self.borrowers
            .values()
            .filter_map(|borrower| {
                let mut amount_near = borrower.amount_collateral;
                if self.collateral_token != WNEAR.to_string() {
                    amount_near = ref_pool[1].get_return(
                        &self.collateral_token,
                        borrower.amount_collateral,
                        &WNEAR.to_string(),
                    )
                }
                let max_amount =
                    ref_pool[0].get_return(&WNEAR.to_string(), amount_near, &self.lending_token);
                if borrower.amount > (max_amount * LIQUIDATE_THRESHOLD / 100) {
                    Some(borrower)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn amount_claimable(&self, lender_id: AccountId) -> Balance {
        if let Some(lender) = self.lenders.get(&lender_id) {
            let pendding_reward = self
                .borrowers
                .values()
                .fold(0, |acc, borrower| acc + self.get_interest(&borrower));
            let reward_per_share = self.reward_per_share
                + (U256::from(pendding_reward) * U256::from(SHARE_DIVISOR)
                    / U256::from(self.total_share))
                .as_u128();
            reward_per_share * lender.share / SHARE_DIVISOR + lender.acc_reward
        } else {
            0
        }
    }

    fn get_interest(&self, borrower: &Loan) -> Balance {
        (U256::from(self.interest_rate)
            * U256::from(env::block_timestamp() - borrower.loan_start_time)
            / U256::from(ONE_DAY)
            / U256::from(365u128)
            / U256::from(INTEREST_DIVISOR))
        .as_u128()
            * borrower.amount
    }
}
