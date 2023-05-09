use scrypto::prelude::*;
use scrypto_unit::*;
use transaction::builder::ManifestBuilder;
use std::collections::BTreeMap;

#[test]
fn test_plazapair() {
    // Setup the environment
    let mut test_runner = TestRunner::builder().build();

    // Create an account
    let (public_key, _private_key, account_component) = test_runner.new_allocated_account();

    // Publish package
    let package_address = test_runner.compile_and_publish(this_package!());

    // Spawn some tokens
    let manifest = ManifestBuilder::new()
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "BASE".to_string())]), dec!(10000))
        .new_token_fixed(BTreeMap::from([("symbol".to_string(), "QUOTE".to_string())]), dec!(10000))
        .call_method(
            account_component,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = test_runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    println!("{:?}\n", receipt);
    let base_address = receipt.expect_commit(true).new_resource_addresses()[0];
    let quote_address = receipt.expect_commit(true).new_resource_addresses()[1];

    // Test the pair instantiation
    let manifest = ManifestBuilder::new()
        .call_method(
            account_component,
            "withdraw",
            manifest_args!(base_address, dec!(2000))
        )
        .call_method(
            account_component,
            "withdraw",
            manifest_args!(quote_address, dec!(1000))
        )
        .take_from_worktop(base_address, |builder, base_bucket| {
            builder.take_from_worktop(quote_address, |builder, quote_bucket| {
                builder.call_function(
                    package_address,
                    "PlazaPair",
                    "instantiate_pair",
                    manifest_args!(base_bucket, quote_bucket, dec!(10)),
                )
            })
        })
        .call_method(
            account_component,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = test_runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    println!("{:?}\n", receipt);
    let _base_lp = receipt.expect_commit(true).new_resource_addresses()[0];
    let _quote_lp = receipt.expect_commit(true).new_resource_addresses()[1];
    let pair = receipt.expect_commit(true).new_component_addresses()[0];

    // Test the swap function
    let manifest = ManifestBuilder::new()
        .call_method(
            account_component,
            "withdraw",
            manifest_args!(base_address, dec!(1))
        )
        .take_from_worktop(base_address, |builder, base_bucket| {
            builder.call_method(
                pair,
                "swap",
                manifest_args!(base_bucket)
            )
        })
        .call_method(
            account_component,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = test_runner.execute_manifest_ignoring_fee(
        manifest,
        vec![NonFungibleGlobalId::from_public_key(&public_key)],
    );
    println!("{:?}\n", receipt);

    // // Test the `free_token` method.
    // let manifest = ManifestBuilder::new()
    //     .call_method(component, "free_token", manifest_args!())
    //     .call_method(
    //         account_component,
    //         "deposit_batch",
    //         manifest_args!(ManifestExpression::EntireWorktop),
    //     )
    //     .build();
    // let receipt = test_runner.execute_manifest_ignoring_fee(
    //     manifest,
    //     vec![NonFungibleGlobalId::from_public_key(&public_key)],
    // );
    // println!("{:?}\n", receipt);
    // receipt.expect_commit_success();
}
