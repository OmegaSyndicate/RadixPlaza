// use scrypto::prelude::*;
// use scrypto_unit::*;
// use transaction::builder::ManifestBuilder;
// use transaction::ecdsa_secp256k1::EcdsaSecp256k1PrivateKey;
// use radix_engine::transaction::TransactionReceipt;
// use std::collections::BTreeMap;
// use std::fs::{create_dir_all, File};
// use std::io::prelude::*;

// pub struct User {
//     pub public_key: EcdsaSecp256k1PublicKey,
//     pub private_key: EcdsaSecp256k1PrivateKey,
//     pub account: ComponentAddress,
// }

// #[allow(unused)]
// pub fn make_user(test_runner: &mut TestRunner) -> User {
//     let (public_key, private_key, account) = test_runner.new_allocated_account();
//     User {
//         public_key,
//         private_key,
//         account,
//     }
// }

// /// Saves the given transaction receipt to a text file.
// ///
// /// # Arguments
// ///
// /// * filename - The name of the file where the receipt will be saved.
// /// * receipt - The transaction receipt to be saved.
// fn save_receipt_to_file(filename: &str, receipt: &TransactionReceipt) {
//     create_dir_all("debug").expect("Unable to create 'debug' directory");
//     let filepath = format!("debug/{}", filename);
//     let mut file = File::create(&filepath).expect("Unable to create file");
//     let receipt_string = format!("{:?}\n", receipt);
//     file.write_all(receipt_string.as_bytes())
//         .expect("Unable to write to file");
// }

// fn create_tokens(runner: &mut TestRunner, user: &User) -> TransactionReceipt {
//     // Spawn some tokens
//     let manifest = ManifestBuilder::new()
//         .new_token_fixed(BTreeMap::from([("symbol".to_string(), "DFP2".to_string())]), dec!(10000))
//         .new_token_fixed(BTreeMap::from([("symbol".to_string(), "BASE1".to_string())]), dec!(10000))
//         .new_token_fixed(BTreeMap::from([("symbol".to_string(), "BASE2".to_string())]), dec!(10000))
//         .call_method(
//             user.account,
//             "deposit_batch",
//             manifest_args!(ManifestExpression::EntireWorktop),
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("create_tokens.txt", &receipt);

//     receipt
// }

// #[allow(unused)]
// fn instantiate_dex(runner: &mut TestRunner, package: PackageAddress, dfp2: ResourceAddress, user: &User) -> TransactionReceipt {
//     // Test the pair instantiation
//     let manifest = ManifestBuilder::new()
//         .call_function(
//             package,
//             "PlazaDex",
//             "instantiate_dex",
//             manifest_args!(dfp2)
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("instantiate_dex.txt", &receipt);

//     receipt
// }

// #[allow(unused)]
// fn create_pair(runner: &mut TestRunner, dex: ComponentAddress, base: ResourceAddress, dfp2: ResourceAddress, p0: Decimal, user: &User) -> TransactionReceipt {
//     // Test the pair instantiation
//     let manifest = ManifestBuilder::new()
//         .call_method(
//             user.account,
//             "withdraw",
//             manifest_args!(base, dec!(1000))
//         )
//         .call_method(
//             user.account,
//             "withdraw",
//             manifest_args!(dfp2, dec!(1000))
//         )
//         .take_from_worktop(base, |builder, base_bucket| {
//             builder.take_from_worktop(dfp2, |builder, dfp2_bucket| {
//                 builder.call_method(
//                     dex,
//                     "create_pair",
//                     manifest_args!(base_bucket, dfp2_bucket, p0)
//                 )
//             })
//         })
//         .call_method(
//             user.account,
//             "deposit_batch",
//             manifest_args!(ManifestExpression::EntireWorktop),
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("create_pair.txt", &receipt);

//     receipt
// }

// #[allow(unused)]
// pub fn swap(runner: &mut TestRunner, dex: ComponentAddress, input: ResourceAddress, amount: Decimal, output: ResourceAddress, user: &User) -> TransactionReceipt {
//     // Call the swap method
//     let manifest = ManifestBuilder::new()
//         .call_method(
//             user.account,
//             "withdraw",
//             manifest_args!(input, amount)
//         )
//         .take_from_worktop(input, |builder, input_bucket| {
//             builder.call_method(
//                 dex,
//                 "swap",
//                 manifest_args!(input_bucket, output)
//             )
//         })
//         .call_method(
//             user.account,
//             "deposit_batch",
//             manifest_args!(ManifestExpression::EntireWorktop),
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("last_swap.txt", &receipt);

//     receipt   
// }

// #[allow(unused)]
// pub fn add_liquidity(runner: &mut TestRunner, dex: ComponentAddress, input: ResourceAddress, amount: Decimal, base: Option<ResourceAddress>, user: &User) -> TransactionReceipt {
//     // Call the add liquidity method
//     let manifest = ManifestBuilder::new()
//         .call_method(
//             user.account,
//             "withdraw",
//             manifest_args!(input, amount)
//         )
//         .take_from_worktop(input, |builder, bucket| {
//             builder.call_method(
//                 dex,
//                 "add_liquidity",
//                 manifest_args!(bucket, base)
//             )
//         })
//         .call_method(
//             user.account,
//             "deposit_batch",
//             manifest_args!(ManifestExpression::EntireWorktop),
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("liquidity_add.txt", &receipt);
    
//     receipt    
// }

// pub fn get_lp_tokens(runner: &mut TestRunner, dex: ComponentAddress, base_token: ResourceAddress, user: &User) -> (ResourceAddress, ResourceAddress) {
//     let manifest = ManifestBuilder::new()
//         .call_method(
//             dex,
//             "get_lp_tokens",
//             manifest_args!(base_token)
//         )
//         .build();
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     //save_receipt_to_file("get_lp_tokens.txt", &receipt);
//     let result = receipt.expect_commit_success();

//     result.output(1)
// }


// #[allow(unused)]
// pub fn remove_liquidity(runner: &mut TestRunner, dex: ComponentAddress, lp_address: ResourceAddress, amount: Decimal, user: &User) -> TransactionReceipt {
//     // Call the add liquidity method
//     let manifest = ManifestBuilder::new()
//         .call_method(
//             user.account,
//             "withdraw",
//             manifest_args!(lp_address, amount)
//         )
//         .take_from_worktop(lp_address, |builder, bucket| {
//             builder.call_method(
//                 dex,
//                 "remove_liquidity",
//                 manifest_args!(bucket)
//             )
//         })
//         .call_method(
//             user.account,
//             "deposit_batch",
//             manifest_args!(ManifestExpression::EntireWorktop),
//         )
//         .build();        
//     let receipt = runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     save_receipt_to_file("liquidity_remove.txt", &receipt);

//     receipt    
// }

// // #[allow(unused)]
// // pub fn get_data(
// //     test_runner: &mut TestRunner,
// //     user: &User,
// //     component: ComponentAddress,
// // ) -> TransactionReceipt {
// //     let manifest = ManifestBuilder::new()
// //         .call_method(component, "get_data", manifest_args!())
// //         .build();
// //     let receipt = test_runner.execute_manifest_ignoring_fee(
// //         manifest,
// //         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
// //     );
// //     println!("get_data receipt:{:?}\n", receipt);
// //     save_receipt_to_file("get_data.txt", &receipt);

// //     receipt
// // }

// pub fn fixtures() -> (TestRunner, User, ComponentAddress, Vec<ResourceAddress>) {
//     // Setup the test environment
//     let mut test_runner = TestRunner::builder().build();

//     // Publish the package
//     let package = test_runner.compile_and_publish(this_package!());

//     // Create a new account
//     let user = make_user(&mut test_runner);

//     // Create some tokens
//     let receipt = create_tokens(&mut test_runner, &user);
//     let tokens = receipt.expect_commit(true).new_resource_addresses().to_vec();

//     // Instantiate dex
//     let receipt = instantiate_dex(&mut test_runner, package, tokens[0], &user);
//     let dex = receipt.expect_commit(true).new_component_addresses()[0];

//     // Create pairs
//     let _receipt = create_pair(&mut test_runner, dex, tokens[1], tokens[0], dec!(1), &user);
//     let _receipt = create_pair(&mut test_runner, dex, tokens[2], tokens[0], dec!(1), &user);

//     (test_runner, user, dex, tokens)
// }