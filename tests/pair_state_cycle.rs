use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;

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
        k_in: dec!("0.5"),
        k_out: dec!("1"),
        fee: dec!(0),
        decay_factor: dec!("0.9512"),
    };

    let mut pair = PlazaPair::instantiate_pair(
        OwnerRole::None,
        base_bucket.resource_address(&mut env)?,
        quote_bucket.resource_address(&mut env)?,
        config,
        dec!(1),
        package,
        &mut env,
    )?;

    let _ = pair.add_liquidity(base_bucket.take(dec!(1000), &mut env)?, None, &mut env)?;
    let _ = pair.add_liquidity(quote_bucket.take(dec!(1000), &mut env)?, None, &mut env)?;

    Ok(func(env, &mut pair, base_bucket, quote_bucket)?)
}

#[test]
fn full_state_cycle_with_exact_math() -> Result<(), RuntimeError> {
    publish_and_setup(|
        mut env: TestEnvironment,
        pair: &mut PlazaPair,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(), RuntimeError> {

        // Complete cycle: Equilibrium -> QuoteShortage -> Equilibrium -> BaseShortage -> Equilibrium

        // Transition 1: Equilibrium -> QuoteShortage
        let (output1, _) = pair.swap(base_bucket.take(dec!(3000), &mut env)?, &mut env)?;
        assert!(output1.amount(&mut env)? == dec!(750), "Output 1 should be 750 quote");

        // Transition 2: QuoteShortage -> Equilibrium
        let (output2, _) = pair.swap(quote_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        assert!(output2.amount(&mut env)? == dec!(3000), "Output 2 should be 3000 base");

        // Transition 3: Equilibrium -> BaseShortage
        let (output3, _) = pair.swap(quote_bucket.take(dec!(3000), &mut env)?, &mut env)?;
        assert!(output3.amount(&mut env)? == dec!(750), "Output 3 should be 750 base");

        // Transition 4: BaseShortage -> Equilibrium
        let (output4, _) = pair.swap(base_bucket.take(dec!(1000), &mut env)?, &mut env)?;
        assert!(output4.amount(&mut env)? == dec!(3000), "Output 4 should be 3000 quote");

        // Verify final state: back to equilibrium with no accumulated errors
        let (_, state, _, _, _, _, _, _) =
            env.read_component_state::<(
                PairConfig, PairState, ResourceAddress, ResourceAddress,
                u8, u8, ComponentAddress, ComponentAddress
            ), _>(*pair).expect("Error reading state");

        assert!(state.p0 == dec!(1), "p0 should remain 1 after full cycle");
        assert!(state.shortage == Shortage::Equilibrium, "Should restore to Equilibrium");
        assert!(state.target_ratio == dec!(1), "target_ratio should restore to exactly 1");
        assert!(state.last_out_spot == dec!(1), "last_out_spot should restore to exactly 1");

        Ok(())
    })
}
