use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap, Vector};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near_bindgen, AccountId, Balance, Gas,
    PanicOnDefault, Promise, PromiseOrValue, PromiseResult, Timestamp,
};
near_sdk::setup_alloc!();
use contract_const::{ft_contract, ref_contract, self_contract, Share, ERR_NO_POOL};
use lending_pool::{LenderInfo, LendingPool, Loan};
mod contract_const;
mod lending_pool;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct LendingContract {
    pub owner: AccountId,
    pub metadata: LazyOption<Metadata>,
    pub pool_id_by_lending_token: UnorderedMap<AccountId, u64>,
    pub pools: Vector<LendingPool>,
    pub pool_count: u64,
}

#[near_bindgen]
impl LendingContract {
    #[init]
    pub fn new(owner: ValidAccountId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            owner: owner.into(),
            metadata: LazyOption::new(
                b"metadata".to_vec(),
                Some(&Metadata {
                    title: Some("Bao Tran Lending Contract".to_string()),
                    organization: None,
                    description: Some(
                        "Lending, borrowing decentralize on Near protocol".to_string(),
                    ),
                }),
            ),
            pool_id_by_lending_token: UnorderedMap::new(b"pool_id_by_lending_token".to_vec()),
            pools: Vector::new(b"pools".to_vec()),
            pool_count: 0,
        }
    }

    pub fn create_new_lending_pool(&mut self, lending_token: ValidAccountId, interest_rate: u64) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner,
            "Caller is not owner"
        );
        log!(
            "{}",
            format!(
                "Create lending pool for token: {}, pool id: {}",
                lending_token, self.pool_count
            )
        );
        let pool = LendingPool {
            pool_id: self.pool_count,
            lending_token: lending_token.clone().into(),
            interest_rate: interest_rate,
            pool_supply: 0,
            amount_borrowed: 0,
            borrowers: UnorderedMap::new(b"borrowers".to_vec()),
            lenders: UnorderedMap::new(b"lenders".to_vec()),
            total_share: 0,
            reward_per_share: 0,
            lastest_reward_time: env::block_timestamp(),
        };
        self.pools.push(&pool);
        self.pool_id_by_lending_token
            .insert(&lending_token.into(), &self.pool_count);
        self.pool_count += 1;
    }

    #[payable]
    pub fn borrow(&mut self, pool_id: u64, amount: U128) -> Promise {
        assert_one_yocto();
        let pool = &self.pools.get(pool_id).expect(ERR_NO_POOL);
        assert!(
            Balance::from(amount) <= pool.pool_supply,
            "Dont enough token to borrow from pool"
        );
        assert_one_yocto();
        ft_contract::ft_transfer(
            ValidAccountId::try_from(env::predecessor_account_id()).unwrap(),
            amount,
            None,
            &pool.lending_token,
            1,
            10_000_000_000_000,
        )
        .then(self_contract::update_borrower(
            pool_id,
            env::predecessor_account_id(),
            amount,
            &env::current_account_id(),
            0,
            10_000_000_000_000,
        ))
    }

    #[private]
    pub fn update_borrower(&mut self, pool_id: u64, borrower: AccountId, amount: U128) -> bool {
        let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);

        if let PromiseResult::Successful(_) = env::promise_result(0) {
            log!(
                "{}",
                format!(
                    "{} borrowed {} token from pool {}",
                    borrower,Balance::from(amount), pool_id
                )
            );
            pool.borrow(borrower, Balance::from(amount));
            self.pools.replace(pool_id, &pool);
            return true;
        }
        return false;
    }

    #[payable]
    pub fn claim(&mut self, pool_id: u64) -> Promise {
        assert_one_yocto();
        log!(
            "{} claim {} token",
            env::predecessor_account_id(),
            self.get_amount_claimable(pool_id, env::predecessor_account_id())
        );
        let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
        let result = pool.claim_reward(env::predecessor_account_id());
        self.pools.replace(pool_id, &pool);
        result
    }

    #[payable]
    pub fn withdraw(&mut self, pool_id: u64, amount: U128) -> Promise {
        assert_one_yocto();
        log!(
            "{} withdraw {} token with interest {}",
            env::predecessor_account_id(),
            Balance::from(amount),
            self.get_amount_claimable(pool_id, env::predecessor_account_id())
        );
        let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
        let result = pool.withdraw(env::predecessor_account_id(), amount.into());
        self.pools.replace(pool_id, &pool);
        result
    }

    #[private]
    pub fn check_claim_success(&mut self, pool_id: u64, lender: AccountId) {
        if let PromiseResult::Successful(_) = env::promise_result(0) {
            let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
            pool.update_lender_claim(lender);
            self.pools.replace(pool_id, &pool);
        }
    }

    #[private]
    pub fn check_withdraw_success(&mut self, pool_id: u64, lender: AccountId, amount: U128) {
        if let PromiseResult::Successful(_) = env::promise_result(0) {
            let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
            pool.update_lender_withdraw(lender, Balance::from(amount));
            self.pools.replace(pool_id, &pool);
        }
    }

    pub fn metadata(&self) -> Metadata {
        self.metadata.get().unwrap()
    }

    pub fn get_amount_claimable(&self, pool_id: u64, lender_id: AccountId) -> Balance {
        self.pools
            .get(pool_id)
            .expect(ERR_NO_POOL)
            .amount_claimable(lender_id)
    }

    pub fn get_pools(&self, from_index: usize, limit: usize) -> Vec<PoolMetadata> {
        self.pools
            .iter()
            .skip(from_index)
            .take(limit)
            .map(|pool| PoolMetadata {
                pool_id: pool.pool_id,
                lending_token: pool.lending_token,
                interest_rate: pool.interest_rate,
                pool_supply: pool.pool_supply,
                amount_borrowed: pool.amount_borrowed,
                total_share: pool.total_share,
                reward_per_share: pool.reward_per_share,
            })
            .collect()
    }

    pub fn get_pool(&self, pool_id: u64) -> PoolMetadata {
        let pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
        PoolMetadata {
            pool_id: pool.pool_id,
            lending_token: pool.lending_token,
            interest_rate: pool.interest_rate,
            pool_supply: pool.pool_supply,
            amount_borrowed: pool.amount_borrowed,
            total_share: pool.total_share,
            reward_per_share: pool.reward_per_share,
        }
    }

    pub fn get_loan(&self, pool_id: u64, borrower_id: AccountId) -> Loan {
        self.pools
            .get(pool_id)
            .expect(ERR_NO_POOL)
            .borrowers
            .get(&borrower_id)
            .expect("ERR_NO_BORROWER")
    }

    pub fn get_lender(&self, pool_id: u64, lender_id: AccountId) -> LenderInfo {
        self.pools
            .get(pool_id)
            .expect(ERR_NO_POOL)
            .lenders
            .get(&lender_id)
            .expect("ERR_NO_LENDER")
    }
}

#[near_bindgen]
impl FungibleTokenReceiver for LendingContract {
    //deposit token by ft_transfer_call
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        if self
            .pool_id_by_lending_token
            .get(&env::predecessor_account_id())
            .is_none()
        {
            PromiseOrValue::Value(U128::from(amount))
        } else {
            //deposit
            if msg == "lend".to_string() {
                let pool_id = self
                    .pool_id_by_lending_token
                    .get(&env::predecessor_account_id())
                    .unwrap();
                log!(
                    "{} deposited {} Yocto {} to pool {}",
                    sender_id,
                    Balance::from(amount),
                    env::predecessor_account_id(),
                    pool_id
                );
                let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
                pool.deposit(sender_id.into(), amount.into());
                self.pools.replace(pool_id, &pool);
                PromiseOrValue::Value(U128::from(0))
            }
            //repay
            else if msg == "repay".to_string() {
                let pool_id = self
                    .pool_id_by_lending_token
                    .get(&env::predecessor_account_id())
                    .unwrap();
                log!(
                    "{} repayed {} Yocto {} to pool {}",
                    sender_id,
                    Balance::from(amount),
                    env::predecessor_account_id(),
                    pool_id
                );
                let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
                let refund = pool.repay(sender_id.into(), amount.into());
                self.pools.replace(pool_id, &pool);
                PromiseOrValue::Value(U128::from(refund))
            } else {
                PromiseOrValue::Value(U128::from(amount))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[derive(BorshDeserialize, BorshSerialize)]

pub struct Metadata {
    pub title: Option<String>,
    pub organization: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct PoolMetadata {
    pub pool_id: u64,
    pub lending_token: AccountId,
    pub interest_rate: u64,
    pub pool_supply: Balance,
    pub amount_borrowed: Balance,
    pub total_share: Share,
    pub reward_per_share: Balance,
}

// #[cfg(all(test, not(target_arch = "wasm32")))]
// mod tests {
//     use near_sdk::test_utils::{accounts, VMContextBuilder};
//     use near_sdk::testing_env;

//     use super::*;

//     fn get_context(predecessor_account_id: ValidAccountId, amount: Balance) -> VMContextBuilder {
//         let mut builder = VMContextBuilder::new();
//         builder
//             .current_account_id(accounts(0))
//             .signer_account_id(predecessor_account_id.clone())
//             .predecessor_account_id(predecessor_account_id)
//             .attached_deposit(amount);
//         builder
//     }

//     #[test]
//     fn test_new() {
//         let mut context = get_context(accounts(1), 0);
//         testing_env!(context.build());
//         let contract = LendingContract::new(accounts(1).into());
//         contract.create_new_lending_pool(ValidAccountId::try_from("tieubaoca.testnet").unwrap(), 200);
//     }
// }
