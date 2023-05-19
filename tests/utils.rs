use scrypto::prelude::*;
use scrypto_unit::*;
use transaction::builder::ManifestBuilder;
use transaction::ecdsa_secp256k1::EcdsaSecp256k1PrivateKey;
use radix_engine::transaction::TransactionReceipt;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::prelude::*;

pub struct User {
    pub public_key: EcdsaSecp256k1PublicKey,
    pub private_key: EcdsaSecp256k1PrivateKey,
    pub account: ComponentAddress,
}

#[allow(unused)]
pub fn make_user(test_runner: &mut TestRunner) -> User {
    let (public_key, private_key, account) = test_runner.new_allocated_account();
    User {
        public_key,
        private_key,
        account,
    }
}

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

fn create_tokens(runner: &mut TestRunner, user: &User) -> TransactionReceipt {
    // Spawn some tokens
    let manifest = ManifestBuilder::new()
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "BASE".to_string())]), dec!(10000))
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "QUOTE".to_string())]), dec!(10000))
        .call_method(
            user.account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
    );
    save_receipt_to_file("create_tokens.txt", &receipt);

    receipt
}

fn instantiate_pair(runner: &mut TestRunner, package: PackageAddress, base: ResourceAddress, quote: ResourceAddress, user: &User) -> TransactionReceipt {
    // Test the pair instantiation
    let manifest = ManifestBuilder::new()
        .call_method(
            user.account,
            "withdraw",
            manifest_args!(base, dec!(1000))
        )
        .call_method(
            user.account,
            "withdraw",
            manifest_args!(quote, dec!(2000))
        )
        .take_from_worktop(base, |builder, base_bucket| {
            builder.take_from_worktop(quote, |builder, quote_bucket| {
                builder.call_function(
                    package,
                    "PlazaPair",
                    "instantiate_pair",
                    manifest_args!(base_bucket, quote_bucket, dec!(10)),
                )
            })
        })
        .call_method(
            user.account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
    );
    save_receipt_to_file("instantiate_pair.txt", &receipt);

    receipt
}

pub fn swap(runner: &mut TestRunner, input: ResourceAddress, amount: Decimal, pair: ComponentAddress, user: &User) -> TransactionReceipt {
    // Call the swap method
    let manifest = ManifestBuilder::new()
        .call_method(
            user.account,
            "withdraw",
            manifest_args!(input, amount)
        )
        .take_from_worktop(input, |builder, input_bucket| {
            builder.call_method(
                pair,
                "swap",
                manifest_args!(input_bucket)
            )
        })
        .call_method(
            user.account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
    );
    save_receipt_to_file("last_swap.txt", &receipt);

    receipt   
}

pub fn add_liquidity(runner: &mut TestRunner, input: ResourceAddress, amount: Decimal, pair: ComponentAddress, user: &User) -> TransactionReceipt {
    // Call the add liquidity method
    let manifest = ManifestBuilder::new()
        .call_method(
            user.account,
            "withdraw",
            manifest_args!(input, amount)
        )
        .take_from_worktop(input, |builder, bucket| {
            builder.call_method(
                pair,
                "add_liquidity",
                manifest_args!(bucket)
            )
        })
        .call_method(
            user.account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        let receipt = runner.execute_manifest_ignoring_fee(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
        );
        save_receipt_to_file("liquidity_add.txt", &receipt);
    
        receipt    
}

pub fn remove_liquidity(runner: &mut TestRunner, lp_address: ResourceAddress, amount: Decimal, pair: ComponentAddress, user: &User) -> TransactionReceipt {
    // Call the add liquidity method
    let manifest = ManifestBuilder::new()
        .call_method(
            user.account,
            "withdraw",
            manifest_args!(lp_address, amount)
        )
        .take_from_worktop(lp_address, |builder, bucket| {
            builder.call_method(
                pair,
                "remove_liquidity",
                manifest_args!(bucket)
            )
        })
        .call_method(
            user.account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        let receipt = runner.execute_manifest_ignoring_fee(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
        );
        save_receipt_to_file("liquidity_remove.txt", &receipt);
    
        receipt    
}

// #[allow(unused)]
// pub fn get_data(
//     test_runner: &mut TestRunner,
//     user: &User,
//     component: ComponentAddress,
// ) -> TransactionReceipt {
//     let manifest = ManifestBuilder::new()
//         .call_method(component, "get_data", manifest_args!())
//         .build();
//     let receipt = test_runner.execute_manifest_ignoring_fee(
//         manifest,
//         vec![NonFungibleGlobalId::from_public_key(&user.public_key)],
//     );
//     println!("get_data receipt:{:?}\n", receipt);
//     save_receipt_to_file("get_data.txt", &receipt);

//     receipt
// }

pub fn fixtures() -> (TestRunner, User, ComponentAddress, ResourceAddress, ResourceAddress) {
    // Setup the test environment
    let mut test_runner = TestRunner::builder().build();

    // Publish the package
    let package = test_runner.compile_and_publish(this_package!());

    // Create a new account
    let user = make_user(&mut test_runner);
    //let (public_key, _private_key, account) = test_runner.new_allocated_account();

    // Create some tokens
    let receipt = create_tokens(&mut test_runner, &user);
    let base_address = receipt.expect_commit(true).new_resource_addresses()[0];
    let quote_address = receipt.expect_commit(true).new_resource_addresses()[1];

    // Instantiate pair
    let receipt = instantiate_pair(&mut test_runner, package, base_address, quote_address, &user);
    let _base_lp = receipt.expect_commit(true).new_resource_addresses()[0];
    let _quote_lp = receipt.expect_commit(true).new_resource_addresses()[1];
    let pair = receipt.expect_commit(true).new_component_addresses()[0];

    (test_runner, user, pair, base_address, quote_address)
}