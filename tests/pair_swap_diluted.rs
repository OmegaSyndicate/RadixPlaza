use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaPair, Bucket, Bucket) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let base_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(20000, &mut env)?;

    let config = PairConfig {
        k_in: dec!(14) / dec!(9),
        k_out: dec!("4"),
        fee: dec!(0),
        decay_factor: dec!("0.998"),
    };

    let mut pair = PlazaPair::instantiate_pair(
        OwnerRole::None,
        base_bucket.take(dec!("0.0001"), &mut env)?,
        quote_bucket.take(dec!("0.0001"), &mut env)?,
        config,
        dec!(1),
        package,
        &mut env,
    )?;

    let _lp_tokens = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
    let _lp_tokens = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn swaps_base_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap, _) = pair.swap(base_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let expected = dec!(500);
        assert!(swap.amount(&mut env)? == expected,
            "Incorrect return amount. Got {}, expected {}",
            swap.amount(&mut env)?, expected
        );

        let (swap, _) = pair.swap(quote_bucket.take(dec!(750), &mut env)?, &mut env)?;
        let expected = dec!(2500);
        assert!(swap.amount(&mut env)? == expected,
            "Incorrect return amount. Got {}, expected {}",
            swap.amount(&mut env)?, expected
        );
        Ok(())
    })
}

#[test]
fn swaps_quote_to_correct_amount() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {
        let (swap, _) = pair.swap(quote_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let expected = dec!(500);
        assert!(swap.amount(&mut env)? == expected,
            "Incorrect return amount. Got {}, expected {}",
            swap.amount(&mut env)?, expected
        );

        let (swap, _) = pair.swap(base_bucket.take(dec!(750), &mut env)?, &mut env)?;
        let expected = dec!(2500);
        assert!(swap.amount(&mut env)? == expected,
            "Incorrect return amount. Got {}, expected {}",
            swap.amount(&mut env)?, expected
        );
        Ok(())
    })
}