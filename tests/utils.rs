use scrypto::prelude::*;
use scrypto_unit::*;
use transaction::builder::ManifestBuilder;
use radix_engine::transaction::TransactionReceipt;
use std::collections::BTreeMap;
use std::fs::{create_dir_all, File};
use std::io::prelude::*;

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

fn create_tokens(runner: &mut TestRunner, account: ComponentAddress, public_key: EcdsaSecp256k1PublicKey) -> TransactionReceipt {
    // Spawn some tokens
    let manifest = ManifestBuilder::new()
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "BASE".to_string())]), dec!(10000))
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "QUOTE".to_string())]), dec!(10000))
        .call_method(
            account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    save_receipt_to_file("create_tokens.txt", &receipt);

    receipt
}

fn instantiate_pair(runner: &mut TestRunner, package: PackageAddress, base: ResourceAddress, quote: ResourceAddress, account: ComponentAddress, public_key: EcdsaSecp256k1PublicKey) -> TransactionReceipt {
    // Test the pair instantiation
    let manifest = ManifestBuilder::new()
        .call_method(
            account,
            "withdraw",
            manifest_args!(base, dec!(1000))
        )
        .call_method(
            account,
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
            account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    save_receipt_to_file("instantiate_pair.txt", &receipt);

    receipt
}

pub fn swap(runner: &mut TestRunner, input: ResourceAddress, amount: Decimal, pair: ComponentAddress, account: ComponentAddress, public_key: EcdsaSecp256k1PublicKey) -> TransactionReceipt {
    // Test the swap function
    let manifest = ManifestBuilder::new()
        .call_method(
            account,
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
            account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    save_receipt_to_file("last_swap.txt", &receipt);

    receipt   
}

pub fn fixtures() -> (TestRunner, ComponentAddress, EcdsaSecp256k1PublicKey, ComponentAddress, ResourceAddress, ResourceAddress) {
    // Setup the test environment
    let mut test_runner = TestRunner::builder().build();

    // Create a new account
    let (public_key, _private_key, account) = test_runner.new_allocated_account();

    // Publish the package
    let package = test_runner.compile_and_publish(this_package!());

    // Create some tokens
    let receipt = create_tokens(&mut test_runner, account, public_key);
    let base_address = receipt.expect_commit(true).new_resource_addresses()[0];
    let quote_address = receipt.expect_commit(true).new_resource_addresses()[1];

    // Instantiate pair
    let receipt = instantiate_pair(&mut test_runner, package, base_address, quote_address, account, public_key);
    let _base_lp = receipt.expect_commit(true).new_resource_addresses()[0];
    let _quote_lp = receipt.expect_commit(true).new_resource_addresses()[1];
    let pair = receipt.expect_commit(true).new_component_addresses()[0];

    (test_runner, account, public_key, pair, base_address, quote_address)
}