use scrypto::prelude::*;
mod utils;

#[test]
fn instantiates() {
    let (_test_runner, _user, _pair, _base_address, _quote_address) = utils::fixtures();
}

#[test]
fn swaps() {
    let (mut test_runner, user, pair, base_address, _quote_address) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, base_address, dec!(1), pair, &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}

#[test]
fn add_accepts_new_liquidity() {
    let (mut test_runner, user, pair, base_address, _quote_address) = utils::fixtures();
    let receipt = utils::add_liquidity(&mut test_runner, base_address, dec!(1), pair, &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}

// #[test]
// fn get_data() {
//     let (mut test_runner, account, key, pair, base_address, _quote_address) = utils::fixtures();
//     let user = utils::make_user(&mut test_runner);
//     let receipt = utils::get_data(&mut test_runner, &user, pair);
//     receipt.expect_commit_success();
//     println!("get_data receipt success");
//     let data: (u16, Decimal, Decimal) = receipt.expect_commit(true).output(1);
//     println!("data:{:?}\n", data);   
// }



// #[test]
// fn remove_accepts_base_removal() {
//     let (mut test_runner, account, key, pair, base_address, _quote_address) = utils::fixtures();
//     let receipt = utils::remove_liquidity(&mut test_runner, base_address, dec!(1), pair, account, key);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();
// }