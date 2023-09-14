use defiplaza::dex::test_bindings::*;
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
    let bucket3 = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(1000, &mut env)?;

    let a_token_address = bucket1.resource_address(&mut env)?; 
    let b_token_address = bucket2.resource_address(&mut env)?; 
    let dfp2_address = bucket3.resource_address(&mut env)?; 

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
        a_token_address,
        bucket3.take(dec!(1), &mut env)?,
        config,
        dec!(1),
        &mut env,
    )?;
    dex.create_pair( 
        b_token_address,
        bucket3.take(dec!(1), &mut env)?,
        config,
        dec!(1),
        &mut env,
    )?;

    // Act
    let _ = dex.add_liquidity(bucket1, None, &mut env)?;
    let _ = dex.add_liquidity(bucket2, None, &mut env)?;
    let _ = dex.add_liquidity(bucket3, Some(a_token_address), &mut env)?;

    // Assert
    Ok(())
}