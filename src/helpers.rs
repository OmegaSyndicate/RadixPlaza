use scrypto::prelude::*;
use crate::constants::*;

// Helper function to deposit to a pool
pub fn deposit_to_pool(pool: &mut Global<TwoResourcePool>, bucket: &mut Bucket, amount: Decimal) {
    if amount > ZERO {
        pool.protected_deposit(bucket.take(amount));
    }   
}

// Helper function to withdraw from a pool
pub fn withdraw_from_pool(pool: &mut Global<TwoResourcePool>, bucket: &mut Bucket, amount: Decimal) {
    let address = bucket.resource_address();
    if amount > ZERO {
        bucket.put(pool.protected_withdraw(address, amount, WithdrawStrategy::Exact));
    }
}
