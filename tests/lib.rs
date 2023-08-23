mod plazapair_tests {
    use std::io::Write;
    use std::fs::{File, create_dir_all};
    use scrypto::prelude::*;
    use radix_engine::types::dec;
    use radix_engine::transaction::TransactionReceipt;
    use radix_engine_interface::blueprints::resource::OwnerRole;
    use test_engine::env_args;
    use test_engine::environment::Environment;
    use test_engine::test_engine::TestEngine;
    use test_engine::receipt_traits::{GetReturn, Outcome};

    /// Saves the given transaction receipt to a text file.
    ///
    /// # Arguments
    ///
    /// * filename - The name of the file where the receipt will be saved.
    /// * receipt - The transaction receipt to be saved.
    fn save_receipt_to_file(filename: &str, receipt: &TransactionReceipt) {
        create_dir_all("debug").expect("Unable to create 'debug' directory");
        let filepath = format!("debug/{}", filename);
        let mut file = File::create(&filepath).expect("Unable to create file");
        let receipt_string = format!("{:?}\n", receipt);
        file.write_all(receipt_string.as_bytes())
            .expect("Unable to write to file");
    }

    fn initialize() -> TestEngine {
        let mut test_engine = TestEngine::new();
        test_engine.new_token("base", dec!(1_000_000));
        test_engine.new_token("quote", dec!(1_000_000));
        test_engine.new_package("defiplaza package", "./");
        test_engine.new_component(
            "plazapair",
            "PlazaPair",
            "instantiate_pair",
            env_args!(
                Environment::Resource("base"),
                Environment::Resource("quote"),
                dec!(1)
            ),
        );
        test_engine
    }

    fn init_funded() -> TestEngine {
        let mut test_engine = initialize();
        test_engine.call_method(
            "add_liquidity",
            env_args!(
                Environment::FungibleBucket("base", dec!(1000))
            ),
        );
        test_engine.call_method(
            "add_liquidity",
            env_args!(
                Environment::FungibleBucket("quote", dec!(1000))
            ),
        );
        test_engine
    }

    #[test]
    fn test_add_first_base_liquidity() {
        let mut test_engine = initialize();
        test_engine.call_method(
            "add_liquidity",
            env_args!(
                Environment::FungibleBucket("base", dec!(1000))
            ),
        ).assert_is_success();
        let lp_amount = test_engine.current_balance("BASELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(1000));
        assert_eq!(base_amount, dec!(999_000));
        assert_eq!(quote_amount, dec!(1_000_000));
    }

    #[test]
    fn test_add_second_base_liquidity() {
        let mut test_engine = initialize();
        for _ in 0..2 {
            test_engine.call_method(
                "add_liquidity",
                env_args!(
                    Environment::FungibleBucket("base", dec!(1000))
                ),
            ).assert_is_success();
        }
        let lp_amount = test_engine.current_balance("BASELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(2000));
        assert_eq!(base_amount, dec!(998_000));
        assert_eq!(quote_amount, dec!(1_000_000));
    }

    #[test]
    fn test_add_first_quote_liquidity() {
        let mut test_engine = initialize();
        test_engine.call_method(
            "add_liquidity",
            env_args!(
                Environment::FungibleBucket("quote", dec!(1000))
            ),
        ).assert_is_success();
        let lp_amount = test_engine.current_balance("QUOTELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(1000));
        assert_eq!(base_amount, dec!(1_000_000));
        assert_eq!(quote_amount, dec!(999_000));
    }

    #[test]
    fn test_add_second_quote_liquidity() {
        let mut test_engine = initialize();
        for _ in 0..2 {
            test_engine.call_method(
                "add_liquidity",
                env_args!(
                    Environment::FungibleBucket("quote", dec!(1000))
                ),
            ).assert_is_success();
        }
        let lp_amount = test_engine.current_balance("QUOTELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(2000));
        assert_eq!(base_amount, dec!(1_000_000));
        assert_eq!(quote_amount, dec!(998_000));
    }

    #[test]
    fn test_remove_base_liquidity() {
        let mut test_engine = init_funded();
        test_engine.call_method(
            "remove_liquidity",
            env_args!(
                Environment::FungibleBucket("BASELP", dec!(500))
            ),
        ).assert_is_success();
        let lp_amount = test_engine.current_balance("BASELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(500));
        assert_eq!(base_amount, dec!(999_500));
        assert_eq!(quote_amount, dec!(999_000));
    }

    #[test]
    fn test_remove_quote_liquidity() {
        let mut test_engine = init_funded();
        test_engine.call_method(
            "remove_liquidity",
            env_args!(
                Environment::FungibleBucket("QUOTELP", dec!(500))
            ),
        ).assert_is_success();
        let lp_amount = test_engine.current_balance("QUOTELP");
        let base_amount = test_engine.current_balance("base");
        let quote_amount = test_engine.current_balance("quote");
        assert_eq!(lp_amount, dec!(500));
        assert_eq!(base_amount, dec!(999_000));
        assert_eq!(quote_amount, dec!(999_500));
    }
}


// use scrypto::prelude::*;
// mod utils;

// #[test]
// fn instantiates() {
//     let (_test_runner, _user, _dex, _tokens) = utils::fixtures();
// }

// #[test]
// fn swap_a_to_quote() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[0], &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();    
// }

// #[test]
// fn swap_quote_to_a() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::swap(&mut test_runner, dex, tokens[0], dec!(1), tokens[1], &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();    
// }

// #[test]
// fn swap_a_to_b() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[2], &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();    
// }

// #[test]
// fn swap_refuses_a_to_a() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::swap(&mut test_runner, dex, tokens[1], dec!(1), tokens[1], &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_failure();    
// }

// #[test]
// fn add_quote_in_equilibrium() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::add_liquidity(&mut test_runner, dex, tokens[0], dec!(1), Some(tokens[1]), &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();
// }

// #[test]
// fn add_a_in_equilibrium() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let receipt = utils::add_liquidity(&mut test_runner, dex, tokens[1], dec!(1), None, &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();
// }

// #[test]
// fn remove() {
//     let (mut test_runner, user, dex, tokens) = utils::fixtures();
//     let (base_lp, _quote_lp) = utils::get_lp_tokens(&mut test_runner, dex, tokens[1], &user);
//     let receipt = utils::remove_liquidity(&mut test_runner, dex, base_lp, dec!(1), &user);
//     println!("{:?}\n", receipt);
//     receipt.expect_commit_success();
// }



// // #[test]
// // fn remove_accepts_base_removal() {
// //     let (mut test_runner, account, key, pair, base_address, _quote_address) = utils::fixtures();
// //     let receipt = utils::remove_liquidity(&mut test_runner, base_address, dec!(1), pair, account, key);
// //     println!("{:?}\n", receipt);
// //     receipt.expect_commit_success();
// // }