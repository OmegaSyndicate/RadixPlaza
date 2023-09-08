use defiplaza::pair::test_bindings::*;
use defiplaza::types::PairConfig;
use scrypto::*;
use scrypto_test::prelude::*;

#[test]
fn deploys() -> Result<(), RuntimeError> {
    // Arrange
    let mut env = TestEnvironment::new();
    let package_address = Package::compile_and_publish(this_package!(), &mut env)?;

    let bucket1 = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(1000, &mut env)?;
    let bucket2 = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(1000, &mut env)?;

    let resource_address1 = bucket1.resource_address(&mut env)?; 
    let resource_address2 = bucket2.resource_address(&mut env)?; 

    let config = PairConfig {
        k_in: dec!("0.4"),
        k_out: dec!("1"),
        fee: dec!("0.003"),
    };
    let mut pair = PlazaPair::instantiate_pair( 
        resource_address1,
        resource_address2,
        config,
        dec!(1),
        package_address,
        &mut env,
    )?;

    // Act
    let pool_units = pair.add_liquidity(bucket1, &mut env)?; 

    // Assert
    assert_eq!(pool_units.amount(&mut env)?, dec!("1000")); 
    Ok(())
}

// mod plazapair_tests {
//     use std::io::Write;
//     use std::fs::{File, create_dir_all};
//     use scrypto_unit::prelude::*;
//     use radix_engine_interface::radix_engine_common::dec;
//     use radix_engine::transaction::TransactionReceipt;
//     use radix_engine_interface::blueprints::resource::OwnerRole;
//     use test_engine::env_args;
//     use test_engine::environment::{Environment};
//     use test_engine::test_engine::TestEngine;
//     use test_engine::receipt_traits::{GetReturn, Outcome};

//     fn save_receipt_to_file(filename: &str, receipt: &TransactionReceipt) {
//         create_dir_all("debug").expect("Unable to create 'debug' directory");
//         let filepath = format!("debug/{}", filename);
//         let mut file = File::create(&filepath).expect("Unable to create file");
//         let receipt_string = format!("{:?}\n", receipt);
//         file.write_all(receipt_string.as_bytes())
//             .expect("Unable to write to file");
//     }

//     fn initialize() -> TestEngine {
//         let mut test_engine = TestEngine::new();
//         // let config = PairConfig {
//         //     k_in: dec!("0.4"),
//         //     k_out: dec!("1"),
//         //     fee: dec!("0.003"),
//         // };
//         test_engine.new_token("base", dec!(1_000_000));
//         test_engine.new_token("quote", dec!(1_000_000));
//         test_engine.new_package("defiplaza package", "./");
//         test_engine.new_component(
//             "plazapair",
//             "PlazaPair",
//             "instantiate_pair",
//             env_args!(
//                 Environment::Resource("base"),
//                 Environment::Resource("quote"),
//                 dec!(2)
//             ),
//         );
//         test_engine
//     }

//     fn init_funded() -> TestEngine {
//         let mut test_engine = initialize();
//         test_engine.call_method(
//             "add_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(1000))
//             ),
//         );
//         test_engine.call_method(
//             "add_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(1000))
//             ),
//         );
//         test_engine
//     }

//     #[test]
//     fn test_add_first_base_liquidity() {
//         let mut test_engine = initialize();
//         test_engine.call_method(
//             "add_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("BASELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(1000));
//         assert_eq!(base_amount, dec!(999_000));
//         assert_eq!(quote_amount, dec!(1_000_000));
//     }

//     #[test]
//     fn test_add_second_base_liquidity() {
//         let mut test_engine = initialize();
//         for _ in 0..2 {
//             test_engine.call_method(
//                 "add_liquidity",
//                 env_args!(
//                     Environment::FungibleBucket("base", dec!(1000))
//                 ),
//             ).assert_is_success();
//         }
//         let lp_amount = test_engine.current_balance("BASELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(2000));
//         assert_eq!(base_amount, dec!(998_000));
//         assert_eq!(quote_amount, dec!(1_000_000));
//     }

//     #[test]
//     fn test_add_first_quote_liquidity() {
//         let mut test_engine = initialize();
//         test_engine.call_method(
//             "add_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("QUOTELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(1000));
//         assert_eq!(base_amount, dec!(1_000_000));
//         assert_eq!(quote_amount, dec!(999_000));
//     }

//     #[test]
//     fn test_add_second_quote_liquidity() {
//         let mut test_engine = initialize();
//         for _ in 0..2 {
//             test_engine.call_method(
//                 "add_liquidity",
//                 env_args!(
//                     Environment::FungibleBucket("quote", dec!(1000))
//                 ),
//             ).assert_is_success();
//         }
//         let lp_amount = test_engine.current_balance("QUOTELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(2000));
//         assert_eq!(base_amount, dec!(1_000_000));
//         assert_eq!(quote_amount, dec!(998_000));
//     }

//     #[test]
//     fn test_remove_base_liquidity() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("BASELP", dec!(500))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("BASELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(500));
//         assert_eq!(base_amount, dec!(999_500));
//         assert_eq!(quote_amount, dec!(999_000));
//     }

//     #[test]
//     fn test_remove_quote_liquidity() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("QUOTELP", dec!(500))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("QUOTELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(500));
//         assert_eq!(base_amount, dec!(999_000));
//         assert_eq!(quote_amount, dec!(999_500));
//     }

//     #[test]
//     fn test_remove_all_base_liquidity() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("BASELP", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("BASELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(000));
//         assert_eq!(base_amount, dec!(1_000_000));
//         assert_eq!(quote_amount, dec!(999_000));
//     }

//     #[test]
//     fn test_remove_all_quote_liquidity() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("QUOTELP", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("QUOTELP");
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(lp_amount, dec!(000));
//         assert_eq!(base_amount, dec!(999_000));
//         assert_eq!(quote_amount, dec!(1_000_000));
//     }

//     #[test]
//     fn test_remove_all_liquidity() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("BASELP", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("BASELP");
//         assert_eq!(lp_amount, dec!(000));
//         test_engine.call_method(
//             "remove_liquidity",
//             env_args!(
//                 Environment::FungibleBucket("QUOTELP", dec!(1000))
//             ),
//         ).assert_is_success();
//         let lp_amount = test_engine.current_balance("QUOTELP");
//         assert_eq!(lp_amount, dec!(000));
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(base_amount, dec!(1_000_000));
//         assert_eq!(quote_amount, dec!(1_000_000));
//     }

//     #[test]
//     fn test_swap_quote_to_base() {
//         let mut test_engine = init_funded();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(2000))
//             ),
//         ).assert_is_success();
//         //save_receipt_to_file("swap_outgoing.txt", &_receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(base_amount, dec!("999498.5"));
//         assert_eq!(quote_amount, dec!(997_000));
//     }

//     #[test]
//     fn test_swap_base_to_quote() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(500))
//             ),
//         ).assert_is_success();
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         assert_eq!(base_amount, dec!(998_500));
//         assert_eq!(quote_amount, dec!("999498.5"));
//     }

//     #[test]
//     fn test_swap_quote_to_base_two_step() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(1000))
//             ),
//         ).assert_is_success();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(1000))
//             ),
//         ).assert_is_success();
//         //save_receipt_to_file("swap_outgoing.txt", &receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Slightly higher return from doing it in 2 steps as your own fees are now part of the liquidity   
//         assert_eq!(base_amount, dec!("999498.562242477213135301"));
//         assert_eq!(quote_amount, dec!(997_000));
//     }

//     #[test]
//     fn test_swap_base_to_quote_two_step() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(250))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(250))
//             ),
//         ).assert_is_success();
//         //save_receipt_to_file("swap_outgoing.txt", &receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Slightly higher return from doing it in 2 steps as your own fees are now part of the liquidity   
//         assert_eq!(base_amount, dec!(998_500));
//         assert_eq!(quote_amount, dec!("999498.56224247721313555"));
//     }

//     #[test]
//     fn test_swap_quote_to_base_three_step() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(1000))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(500))
//             ),
//         ).assert_is_success();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(500))
//             ),
//         ).assert_is_success();        
//         //save_receipt_to_file("swap_outgoing.txt", &receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Slightly higher return from doing it in 2 steps as your own fees are now part of the liquidity   
//         assert_eq!(base_amount, dec!("999498.566682382973174966"));
//         assert_eq!(quote_amount, dec!(997_000));
//     }

//     #[test]
//     fn test_swap_base_to_quote_three_step() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(250))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(125))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(125))
//             ),
//         ).assert_is_success();
//         // save_receipt_to_file("swap_outgoing.txt", &_receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Slightly higher return from doing it in 2 steps as your own fees are now part of the liquidity   
//         assert_eq!(base_amount, dec!(998_500));
//         assert_eq!(quote_amount, dec!("999498.566682382973175122"));
//     }

//     #[test]
//     fn test_swap_base_to_quote_and_back() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(250))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(150))
//             ),
//         ).assert_is_success();
//         // save_receipt_to_file("swap_incoming.txt", &_receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Definitely not a favorable trade to make  
//         assert_eq!(base_amount, dec!("998857.645552688276204298"));
//         assert_eq!(quote_amount, dec!("999182.333333333333333334"));
//     }

//     #[test]
//     fn test_swap_quote_to_base_and_back() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(500))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(75))
//             ),
//         ).assert_is_success();
//         // save_receipt_to_file("swap_incoming.txt", &_receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Definitely not a favorable trade to make  
//         assert_eq!(base_amount, dec!("999124.4"));
//         assert_eq!(quote_amount, dec!("998679.44769142102292177"));
//     }

//     #[test]
//     fn test_swap_base_to_quote_and_back_across_eq() {
//         let mut test_engine = init_funded();
//         test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("base", dec!(250))
//             ),
//         ).assert_is_success();
//         let _receipt = test_engine.call_method(
//             "swap",
//             env_args!(
//                 Environment::FungibleBucket("quote", dec!(500))
//             ),
//         ).assert_is_success();
//         save_receipt_to_file("swap_incoming.txt", &_receipt);
//         let base_amount = test_engine.current_balance("base");
//         let quote_amount = test_engine.current_balance("quote");
//         // Net result of trades more base tokens for LPs  
//         assert_eq!(base_amount, dec!("999045.459355092839813726"));
//         assert_eq!(quote_amount, dec!("998832.333333333333333334"));
//     }
// }
