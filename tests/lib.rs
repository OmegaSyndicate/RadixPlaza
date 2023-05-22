use scrypto::prelude::*;
mod utils;

#[test]
fn instantiates() {
    let (_test_runner, _user, _dex, _tokens) = utils::fixtures();
}

#[test]
fn swap_a_to_dfp2() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[0], &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();    
}

#[test]
fn swap_dfp2_to_a() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, dex, tokens[0], dec!(1), tokens[1], &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();    
}

#[test]
fn swap_a_to_b() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[2], &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();    
}

#[test]
fn swap_refuses_a_to_a() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[1], &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_failure();    
}

#[test]
fn add_dfp2_in_equilibrium() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::add_liquidity(&mut test_runner, dex, tokens[0], dec!(1), Some(tokens[1]), &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}

#[test]
fn add_a_in_equilibrium() {
    let (mut test_runner, user, dex, tokens) = utils::fixtures();
    let receipt = utils::add_liquidity(&mut test_runner, dex, tokens[1], dec!(1), None, &user);
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}



// #[test]
// fn remove_accepts_base_removal() {
//     let (mut test_runner, account, key, pair, base_address, _quote_address) = utils::fixtures();
//     let receipt = utils::remove_liquidity(&mut test_runner, base_address, dec!(1), pair, account, key);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();
// }