use defiplaza::pair::test_bindings::*;
use defiplaza::types::*;
use scrypto::*;
use scrypto_test::prelude::*;


// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(Decimal, Decimal), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut PlazaPair, Bucket, Bucket, Bucket, Bucket) -> Result<(Decimal, Decimal), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let base_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(50000, &mut env)?;
    let quote_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(50000, &mut env)?;

    let config = PairConfig {
        k_in: dec!("0.5"),
        k_out: dec!("1"),
        fee: dec!(0.0015),
        decay_factor: dec!(0),
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

    let base_lp = pair.add_liquidity(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
    let quote_lp = pair.add_liquidity(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;

    Ok(func(env, &mut pair, base_lp, quote_lp, base_bucket, quote_bucket)?)
}


// Individual tests
#[test]
fn add_remove_vs_swap_during_small_quote_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(base_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let lp_quote = pair.add_liquidity(quote_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (quote_bucket, base_bucket) = pair.remove_liquidity(lp_quote, true, &mut env)?;

        Ok((add_amount - quote_bucket.amount(&mut env)?, base_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(quote_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}

#[test]
fn add_remove_vs_swap_during_cherry_picked_quote_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(base_bucket.take(dec!(5000), &mut env)?, &mut env)?;
        let lp_quote = pair.add_liquidity(quote_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (quote_bucket, base_bucket) = pair.remove_liquidity(lp_quote, true, &mut env)?;

        Ok((add_amount - quote_bucket.amount(&mut env)?, base_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(5000), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(quote_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}

#[test]
fn add_remove_vs_swap_during_larger_quote_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let lp_quote = pair.add_liquidity(quote_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (quote_bucket, base_bucket) = pair.remove_liquidity(lp_quote, true, &mut env)?;

        Ok((add_amount - quote_bucket.amount(&mut env)?, base_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(base_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(quote_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}

#[test]
fn add_remove_vs_swap_during_small_base_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(quote_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let lp_base = pair.add_liquidity(base_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (base_bucket, quote_bucket) = pair.remove_liquidity(lp_base, false, &mut env)?;

        Ok((add_amount - base_bucket.amount(&mut env)?, quote_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(2500), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(base_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}

#[test]
fn add_remove_vs_swap_during_cherry_picked_base_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(quote_bucket.take(dec!(5000), &mut env)?, &mut env)?;
        let lp_base = pair.add_liquidity(base_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (base_bucket, quote_bucket) = pair.remove_liquidity(lp_base, false, &mut env)?;

        Ok((add_amount - base_bucket.amount(&mut env)?, quote_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(5000), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(base_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}

#[test]
fn add_remove_vs_swap_during_larger_base_shortage() -> Result<(), RuntimeError> {
    let (virt_in, virt_out) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let add_amount = dec!(1000);
        let _swap = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let lp_base = pair.add_liquidity(base_bucket.take(add_amount, &mut env)?, &mut env)?;
        let (base_bucket, quote_bucket) = pair.remove_liquidity(lp_base, false, &mut env)?;

        Ok((add_amount - base_bucket.amount(&mut env)?, quote_bucket.amount(&mut env)?))
    })?;

    let (swap_out, _remainder) = publish_and_setup(|
        mut env: TestEnvironment, 
        pair: &mut PlazaPair,
        _base_lp: Bucket,
        _quote_lp: Bucket,
        base_bucket: Bucket,
        quote_bucket: Bucket,
    | -> Result<(Decimal, Decimal), RuntimeError> {
        let _swap = pair.swap(quote_bucket.take(dec!(10000), &mut env)?, &mut env)?;
        let (output, _remainder) = pair.swap(base_bucket.take(virt_in, &mut env)?, &mut env)?;
        
        Ok((output.amount(&mut env)?, dec!(0)))
    })?;

    assert!(swap_out >= virt_out, "add/remove output: {}, swap output: {}", virt_out, swap_out);
    Ok(())
}
