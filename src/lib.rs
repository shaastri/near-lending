use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap, Vector};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, ext_contract, log, near_bindgen, serde_json, AccountId, Balance, Gas,
    PanicOnDefault, Promise, PromiseOrValue, PromiseResult, Timestamp,
};
near_sdk::setup_alloc!();
use lending_pool::{LenderInfo, LendingPool, Loan};
use utils::{
    ft_contract, ref_contract, self_contract, PoolInfo, Share, TransferPayload, TransferType,
    ERR_NO_BORROWER, ERR_NO_POOL, MAX_BORROW_RATE, REF_FEE_DIVISOR, REF_FINANCE, U256, WNEAR,
};
mod lending_pool;
mod utils;
mod view;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct LendingContract {
    pub owner: AccountId,
    pub metadata: LazyOption<Metadata>,
    pub pool_ids_by_lending_token: UnorderedMap<AccountId, UnorderedMap<AccountId, u64>>,
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
            pool_ids_by_lending_token: UnorderedMap::new(b"pool_id_by_lending_token".to_vec()),
            pools: Vector::new(b"pools".to_vec()),
            pool_count: 0,
        }
    }

    pub fn create_new_lending_pool(
        &mut self,
        lending_token: ValidAccountId,
        collateral_token: ValidAccountId,
        ref_pool_ids: Vec<u64>,
        interest_rate: u64,
    ) {
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
        let mut pool_id_by_collateral_token = self
            .pool_ids_by_lending_token
            .get(&lending_token.clone().into())
            .unwrap_or(UnorderedMap::new(
                format!("lending_token{}", lending_token.to_string()).as_bytes(),
            ));
        let pool = LendingPool {
            pool_id: self.pool_count,
            lending_token: lending_token.clone().into(),
            collateral_token: collateral_token.clone().into(),
            ref_pool_ids,
            interest_rate: interest_rate,
            pool_supply: 0,
            amount_borrowed: 0,
            borrowers: UnorderedMap::new(format!("{}borrowers", lending_token).as_bytes()),
            lenders: UnorderedMap::new(format!("{}lenders", lending_token).as_bytes()),
            total_share: 0,
            reward_per_share: 0,
            lastest_reward_time: env::block_timestamp(),
        };
        self.pools.push(&pool);
        pool_id_by_collateral_token.insert(&collateral_token.clone().into(), &self.pool_count);
        self.pool_ids_by_lending_token
            .insert(&lending_token.into(), &pool_id_by_collateral_token);
        self.pool_count += 1;
    }

    #[payable]
    pub fn borrow(&mut self, pool_id: u64, amount: U128) -> Promise {
        let pool = &self.pools.get(pool_id).expect(ERR_NO_POOL);
        assert!(
            Balance::from(amount) <= pool.pool_supply,
            "Dont enough token to borrow from pool"
        );
        assert_one_yocto();
        if pool.collateral_token == WNEAR.to_string() {
            ref_contract::get_pool(pool.ref_pool_ids[0], &REF_FINANCE, 0, 10_000_000_000_000).then(
                self_contract::get_ref_pool_callback(
                    pool_id,
                    env::predecessor_account_id(),
                    Balance::from(amount),
                    &env::current_account_id(),
                    0,
                    50_000_000_000_000,
                ),
            )
        } else {
            ref_contract::get_pool(pool.ref_pool_ids[1], &REF_FINANCE, 0, 10_000_000_000_000)
                .then(ref_contract::get_pool(
                    pool.ref_pool_ids[0],
                    &REF_FINANCE,
                    0,
                    10_000_000_000_000,
                ))
                .then(self_contract::get_ref_pool_callback(
                    pool_id,
                    env::predecessor_account_id(),
                    Balance::from(amount),
                    &env::current_account_id(),
                    0,
                    50_000_000_000_000,
                ))
        }
    }

    pub fn get_ref_pool_callback(
        &mut self,
        pool_id: u64,
        borrower_id: AccountId,
        amount: Balance,
    ) -> PromiseOrValue<bool> {
        if let PromiseResult::Successful(result_lending_token_pool) =
            env::promise_result(env::promise_results_count() - 1)
        {
            let pool = &self.pools.get(pool_id).expect(ERR_NO_POOL);
            let borrower = pool.borrowers.get(&borrower_id).expect(ERR_NO_BORROWER);
            let mut amount_near = borrower.amount_collateral;
            let lending_ref_pool =
                serde_json::from_slice::<PoolInfo>(&result_lending_token_pool).unwrap();
            if pool.collateral_token != WNEAR.to_string() {
                if let PromiseResult::Successful(result_collateral_token_pool) =
                    env::promise_result(env::promise_results_count() - 2)
                {
                    let collateral_ref_pool =
                        serde_json::from_slice::<PoolInfo>(&result_collateral_token_pool).unwrap();
                    amount_near = collateral_ref_pool.get_return(
                        &pool.collateral_token,
                        borrower.amount_collateral,
                        &WNEAR.to_string(),
                    );
                } else {
                    return PromiseOrValue::Value(false);
                }
            }
            let max_amount =
                lending_ref_pool.get_return(&WNEAR.to_string(), amount_near, &pool.lending_token)
                    * MAX_BORROW_RATE
                    / 100;
            if max_amount > amount + borrower.amount {
                //transfer token to borrower
                PromiseOrValue::from(
                    ft_contract::ft_transfer(
                        ValidAccountId::try_from(borrower_id.clone()).unwrap(),
                        U128::from(amount),
                        None,
                        &pool.lending_token,
                        1,
                        15_000_000_000_000,
                    )
                    .then(self_contract::update_borrower(
                        pool_id,
                        borrower_id,
                        U128::from(amount),
                        &env::current_account_id(),
                        0,
                        10_000_000_000_000,
                    )),
                )
            } else {
                PromiseOrValue::Value(false)
            }
        } else {
            PromiseOrValue::Value(false)
        }
    }

    #[private]
    pub fn update_borrower(&mut self, pool_id: u64, borrower: AccountId, amount: U128) -> bool {
        let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);

        log!(
            "{}",
            format!(
                "{} borrowed {} token from pool {}",
                borrower,
                Balance::from(amount),
                pool_id
            )
        );
        pool.borrow(borrower, Balance::from(amount));
        self.pools.replace(pool_id, &pool);
        return true;
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
        let transfer_payload = serde_json::from_str::<TransferPayload>(&msg).expect("Wrong format");
        match transfer_payload.transfer_type {
            TransferType::Deposit => {
                let pool_id = self
                    .pool_ids_by_lending_token
                    .get(&env::predecessor_account_id())
                    .expect(ERR_NO_POOL)
                    .get(&transfer_payload.token)
                    .expect(ERR_NO_POOL);
                assert_eq!(pool_id, transfer_payload.pool_id, "pool id: not good");
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
            TransferType::Repay => {
                let pool_id = self
                    .pool_ids_by_lending_token
                    .get(&env::predecessor_account_id())
                    .expect(ERR_NO_POOL)
                    .get(&transfer_payload.token)
                    .expect(ERR_NO_POOL);
                assert_eq!(pool_id, transfer_payload.pool_id, "pool id: not good");
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
            }
            TransferType::Mortgate => {
                let pool_id = self
                    .pool_ids_by_lending_token
                    .get(&transfer_payload.token)
                    .expect(ERR_NO_POOL)
                    .get(&env::predecessor_account_id())
                    .expect(ERR_NO_POOL);
                log!("{} mortgated {} token {} to pool {}", sender_id, Balance::from(amount), env::predecessor_account_id(), pool_id);
                assert_eq!(pool_id, transfer_payload.pool_id, "pool id: not good");
                let mut pool = self.pools.get(pool_id).expect(ERR_NO_POOL);
                pool.mortgate(sender_id.into(), Balance::from(amount));
                self.pools.replace(pool_id, &pool);
                PromiseOrValue::Value(U128::from(0))
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
    pub collateral_token: AccountId,
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
