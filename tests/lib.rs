use scrypto::prelude::*;
mod utils;

#[test]
fn instantiates() {
    let (_test_runner, _account, _key, _pair, _base_address, _quote_address) = utils::fixtures();
}

#[test]
fn swaps() {
    let (mut test_runner, account, key, pair, base_address, _quote_address) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, base_address, dec!(1), pair, account, key);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}