use crate::*;

#[near_bindgen]
impl LendingContract {
    pub fn metadata(&self) -> Metadata {
        self.metadata.get().unwrap()
    }

    pub fn get_amount_claimable(&self, pool_id: u64, lender_id: AccountId) -> Balance {
        self.pools
            .get(pool_id)
            .expect(ERR_NO_POOL)
            .amount_claimable(&lender_id)
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
