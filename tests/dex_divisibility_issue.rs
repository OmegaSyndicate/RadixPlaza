use defiplaza::dex::test_bindings::*;
use defiplaza::types::PairConfig;
use scrypto::*;
use scrypto_test::prelude::*;
//use scrypto::prelude::ToRoleEntry;
//use crate::node_modules::auth::RoleDefinition;
//use scrypto::prelude::Url;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaDex, Bucket, Bucket) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let a_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(6)
        .mint_initial_supply(10_000_000, &mut env)?;
    let dfp2_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10_000_000, &mut env)?;

    let admin_badge = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1, &mut env)?;
    let admin_address = admin_badge.resource_address(&mut env)?;
    let dfp2_address = dfp2_bucket.resource_address(&mut env)?; 

    let mut dex = PlazaDex::instantiate_dex(
        dfp2_address,
        admin_address,
        package,
        &mut env
    )?;

    let config = PairConfig {
        k_in: dec!("0.4"),
        k_out: dec!("1"),
        fee: dec!("0.0015"),
        decay_factor: dec!("0.9512"),
    };
    dex.create_pair( 
        a_bucket.take(dec!(12.5), &mut env)?,
        dfp2_bucket.take(dec!(500), &mut env)?,
        config,
        dec!(40),
        &mut env,
    )?;

    Ok(func(env, &mut dex, a_bucket, dfp2_bucket)?)
}

// Individual tests
#[test]
fn gives_back_proper_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, _dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(a_bucket.take(dec!(1000), &mut env)?, None, Some(base_address), &mut env)?;
        let expected = dec!(1000);
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == expected, "Expected output amount: {}, actual: {}", expected, output_amount);

        let (output, _) = dex.add_liquidity(a_bucket.take(dec!(1000), &mut env)?, None, Some(base_address), &mut env)?;
        let expected = dec!(999.999999999999999999);
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == expected, "Expected output amount: {}, actual: {}", expected, output_amount);

        // This was observed to give zero in StokeNet testing (earlier Scrypto versions)
        let (output, _) = dex.add_liquidity(a_bucket.take(dec!(1), &mut env)?, None, Some(base_address), &mut env)?;
        let expected = dec!(0.999999999999999999);
        let output_amount = output.amount(&mut env)?;
        assert!(output_amount == expected, "Minimum output amount: {}, actual: {}", expected, output_amount);

        Ok(())
    })
}

#[test]
fn accepts_base_add_over_100x_existing() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, _dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(a_bucket.take(dec!(10_000), &mut env)?, None, Some(base_address), &mut env)?;
        assert!(output.amount(&mut env)? == dec!(10_000), "Unexpected output amount: {}", output.amount(&mut env)?);

        Ok(())
    })
}

#[test]
fn accepts_base_add_under_one_hundredth_existing() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, _dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(a_bucket.take(dec!(0.01), &mut env)?, None, Some(base_address), &mut env)?;
        assert!(output.amount(&mut env)? == dec!(0.01), "Unexpected output amount: {}", output.amount(&mut env)?);

        Ok(())
    })
}

#[test]
fn accepts_quote_add_almost_100x_existing() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(dfp2_bucket.take(dec!(49_000), &mut env)?, None, Some(base_address), &mut env)?;
        assert!(output.amount(&mut env)? == dec!(49_000), "Unexpected output amount: {}", output.amount(&mut env)?);

        Ok(())
    })
}

#[test]
fn accepts_quote_add_more_than_100x_existing() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(dfp2_bucket.take(dec!(100_000), &mut env)?, None, Some(base_address), &mut env)?;
        assert!(output.amount(&mut env)? == dec!(100_000), "Unexpected output amount: {}", output.amount(&mut env)?);
        Ok(())
    })
}

#[test]
fn accepts_quote_add_under_one_hundredth_existing() -> Result<(), RuntimeError> {
    publish_and_setup(|mut env, &mut mut dex, a_bucket, dfp2_bucket| -> Result<(), RuntimeError> {
        let base_address = a_bucket.resource_address(&mut env)?;

        let (output, _) = dex.add_liquidity(dfp2_bucket.take(dec!(1), &mut env)?, None, Some(base_address), &mut env)?;
        assert!(output.amount(&mut env)? == dec!(1), "Unexpected output amount: {}", output.amount(&mut env)?);

        Ok(())
    })
}