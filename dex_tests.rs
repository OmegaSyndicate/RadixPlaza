use defiplaza::dex::test_bindings::*;
use defiplaza::types::PairConfig;
use scrypto::*;
use scrypto_test::prelude::*;
use scrypto::prelude::ScryptoBucket;

#[test]
fn deploys() -> Result<(), RuntimeError> {
    // Arrange
    let mut env = TestEnvironment::new();
    let package_address = Package::compile_and_publish(this_package!(), &mut env)?;

    let a_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;
    let b_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;
    let dfp2_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;

    let a_token_address = a_bucket.resource_address(&mut env)?; 
    let b_token_address = b_bucket.resource_address(&mut env)?; 
    let dfp2_address = dfp2_bucket.resource_address(&mut env)?; 

    let admin_badge = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1, &mut env)?;
    let admin_address = admin_badge.resource_address(&mut env)?;

    let mut dex = PlazaDex::instantiate_dex(
        dfp2_address,
        admin_address,
        package_address,
        &mut env
    )?;

    let config = PairConfig {
        k_in: dec!("0.4"),
        k_out: dec!("1"),
        fee: dec!("0"),
    };
    dex.create_pair( 
        a_bucket.take(dec!(1000), &mut env)?.as_fungible()?,
        dfp2_bucket.take(dec!(1), &mut env)?.as_fungible()?,
        config,
        dec!(1),
        &mut env,
    )?;
    dex.create_pair( 
        b_bucket.take(dec!(1000), &mut env)?.as_fungible()?,
        dfp2_bucket.take(dec!(1), &mut env).as_fungible()?,
        config,
        dec!(1),
        &mut env,
    )?;

    // Act
    let _ = dex.add_liquidity(a_bucket, None, &mut env)?;
    let _ = dex.add_liquidity(b_bucket, None, &mut env)?;
    let _ = dex.add_liquidity(dfp2_bucket, Some(a_token_address), &mut env)?;

    // Assert
    Ok(())
}