use scrypto::prelude::*;
use crate::constants::*;
use crate::events::*;
use crate::helpers::*;
use crate::types::*;

#[blueprint]
#[events(SwapEvent, AddLiquidityEvent, RemoveLiquidityEvent)]
mod plazapair {
    // PlazaPair struct represents a liquidity pair in the trading platform
    struct PlazaPair {
        config: PairConfig,                         // Pool configuration
        state: PairState,                           // Pool parameters
        base_address: ResourceAddress,              // Base token address
        quote_address: ResourceAddress,             // Quote token address
        base_pool: Global<TwoResourcePool>,         // Holds base tokens plus some quote tokens
        quote_pool: Global<TwoResourcePool>,        // Holds quote tokens plus some base tokens
        min_liquidity: HashMap<                          
            ComponentAddress,                       // For both pools counter token:
            Vault                                   //  hold a tiny amount to temp add when empty
        >,  
    }

    impl PlazaPair {
        pub fn instantiate_pair(
            owner_role: OwnerRole,
            base_bucket: Bucket,
            quote_bucket: Bucket,
            config: PairConfig,
            initial_price: Decimal,
        ) -> Global<PlazaPair> {
            assert!(base_bucket.amount() == MIN_LIQUIDITY, "Invalid base amount");
            assert!(quote_bucket.amount() == MIN_LIQUIDITY, "Invalid quote amount");
            assert!(config.k_in >= MIN_K_IN, "Invalid k_in value");
            assert!(config.k_out > config.k_in, "k_out should be larger than k_in");
            assert!(config.k_out == ONE || config.k_out < CLIP_K_OUT, "Invalid k_out value");
            assert!(config.fee >= ZERO && config.fee < ONE, "Invalid fee level");
            assert!(config.decay_factor >= ZERO && config.decay_factor < ONE, "Invalid decay factor");

            // Reserve address for Actor Virtual Badge
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(PlazaPair::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);
            let access_rule = rule!(require(global_component_caller_badge));

            // Gather token properties
            let base_address = base_bucket.resource_address();
            let quote_address = quote_bucket.resource_address();
            let base_manager = ResourceManager::from(base_address);
            let quote_manager = ResourceManager::from(quote_address);
            assert!(base_manager.resource_type().divisibility().unwrap() == 18, "Bad base divisibility");
            assert!(quote_manager.resource_type().divisibility().unwrap() == 18, "Bad quote divisibility");

            // Create pool for base liquidity providers
            let base_pool = Blueprint::<TwoResourcePool>::instantiate(
                owner_role.clone(),
                access_rule.clone(),
                (base_address, quote_address),
                None,
            );

            // Create pool for quote liquidity providers
            let quote_pool = Blueprint::<TwoResourcePool>::instantiate(
                owner_role.clone(),
                access_rule,
                (quote_address, base_address),
                None,
            );

            // Create vaults for the minimum liquidity
            let base_vault = Vault::with_bucket(base_bucket);
            let quote_vault = Vault::with_bucket(quote_bucket);
            let pool_map: HashMap<_, _> = [
                (base_pool.address(), quote_vault),
                (quote_pool.address(), base_vault),
            ].into_iter().collect();

            // Instantiate a PlazaPair component
            let now = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            Self {
                config: config,
                state: PairState {
                    p0: initial_price,
                    shortage: Shortage::Equilibrium,
                    target_ratio: ONE,
                    last_outgoing: now,
                    last_out_spot: initial_price,
                },
                base_address: base_address,
                quote_address: quote_address,
                base_pool: base_pool,
                quote_pool: quote_pool,
                min_liquidity: pool_map,
            }
            .instantiate()
            .prepare_to_globalize(owner_role)
            .with_address(address_reservation)
            .globalize()
        }

        // Add liquidity to the pool in return for LP tokens
        pub fn add_liquidity(
            &mut self,
            input_bucket: Bucket,
            is_quote: bool,
        ) -> Bucket {
            // Retrieve appropriate liquidity pool
            let mut pool = match is_quote {
                true => self.quote_pool,
                false => self.base_pool,
            };

            let input_amount = input_bucket.amount();
            let reserve = *pool.get_vault_amounts().get_index(0).map(|(_addr, amount)| amount).unwrap();
            let min_liq = self.min_liquidity.get_mut(&pool.address()).unwrap();
            let min_liq_addr = min_liq.resource_address();
            let mut tiny_bucket = min_liq.take(MIN_LIQUIDITY);

            let lp_bucket = match reserve == ZERO {
                // No liquidity present, this is the first to be added
                true => {
                    // Do initial contribution
                    let input_address = input_bucket.resource_address();
                    let (lp_tokens, _) = pool.contribute((input_bucket, tiny_bucket));

                    // Beef up LP tokens by a factor of 10000 for non-technical reasons
                    let scale_bucket_input = pool.protected_withdraw(
                        input_address,
                        input_amount * (TEN_THOUSAND - ONE) / TEN_THOUSAND,
                        WithdrawStrategy::Exact
                    );
                    let scale_bucket_minliq = pool.protected_withdraw(
                        min_liq_addr,
                        MIN_LIQUIDITY * (TEN_THOUSAND - ONE) / TEN_THOUSAND,
                        WithdrawStrategy::Exact
                    );
                    let (mut scale_lp, remainder) = pool.contribute((scale_bucket_input, scale_bucket_minliq));
                    if let Some(bucket) = remainder {
                        pool.protected_deposit(bucket);
                    }

                    // Stash the min liquidity and return the LP tokens
                    min_liq.put(
                        pool.protected_withdraw(min_liq_addr, MIN_LIQUIDITY, WithdrawStrategy::Exact)
                    );
                    scale_lp.put(lp_tokens);
                    scale_lp
                }
                // Add in ratio
                false => {
                    pool.protected_deposit(
                        tiny_bucket.take(reserve / (reserve + input_amount) / TWO * tiny_bucket.amount())
                    );
                    let (lp_tokens, remainder) = pool.contribute((input_bucket, tiny_bucket));
                    pool.protected_deposit(remainder.expect("Remainder not found??"));
                    min_liq.put(
                        pool.protected_withdraw(min_liq_addr, MIN_LIQUIDITY, WithdrawStrategy::Exact)
                    );
                    lp_tokens
                }
            };
        
            // Emit add liquidity event
            let lp_amount = lp_bucket.amount();
            Runtime::emit_event(AddLiquidityEvent{is_quote, token_amount: input_amount, lp_amount});

            lp_bucket
        }

        // Exchange LP tokens for the underlying liquidity held in the pair
        pub fn remove_liquidity(&mut self, lp_bucket: Bucket, is_quote: bool) -> (Bucket, Bucket) {
            // Get corresponding pool component
            let mut pool = match is_quote {
                true => self.quote_pool,
                false => self.base_pool,
            };

            // Retrieve liquidity and return to the caller
            let lp_amount = lp_bucket.amount();
            let (main_bucket, other_bucket) = pool.redeem(lp_bucket);

            // Emit RemoveLiquidityEvent
            let (main_amount, other_amount) = (main_bucket.amount(), other_bucket.amount());
            Runtime::emit_event(RemoveLiquidityEvent{is_quote, main_amount, other_amount, lp_amount});

            (main_bucket, other_bucket)
        }

        /// Swap a bucket of tokens along the AMM curve.
        pub fn swap(&mut self, mut input_bucket: Bucket) -> (Bucket, Option<Bucket>) {
            // Ensure the input bucket isn't empty
            assert!(input_bucket.amount() > ZERO, "Empty input bucket");

            // Calculate the amount of output tokens and pair impact variables
            let input_amount = input_bucket.amount();
            let input_is_quote = input_bucket.resource_address() == self.quote_address;
            let (output_amount, remainder, fee) = self.quote(input_amount, input_is_quote);

            // Match values to log trade event
            let (base_amount, quote_amount) = match input_is_quote{
                true => (-output_amount, input_amount),
                false => (input_amount, -output_amount),
            };
            Runtime::emit_event(SwapEvent{base_amount, quote_amount});

            // Process the pool allocations
            let (base_pool, quote_pool) = (&mut self.base_pool, &mut self.quote_pool);
            let mut output_bucket = match input_is_quote {
                true => {
                    deposit_to_pool(quote_pool, &mut input_bucket, input_amount - remainder);
                    let mut bucket = Bucket::new(self.base_address);
                    withdraw_from_pool(base_pool, &mut bucket, output_amount);
                    bucket
                },
                false => {
                    deposit_to_pool(base_pool, &mut input_bucket, input_amount - remainder);
                    let mut bucket = Bucket::new(self.quote_address);
                    withdraw_from_pool(quote_pool, &mut bucket, output_amount);
                    bucket
                }
            };

            // Adjust pair state and donate the fee
            if fee > ZERO {
                self.donate_to_pool(output_bucket.take(fee), !input_is_quote);
            }
            assert!(output_bucket.amount() == output_amount, "Something doesn't add up");

            // Create remainder bucket option
            let remainder = match input_bucket.is_empty() {
                true => {
                    input_bucket.drop_empty();
                    None
                },
                false => Some(input_bucket),
            };

            (output_bucket, remainder)
        }

        // To donate some liquidity to the pair
        pub fn donate_to_pool(
            &mut self,
            donation_bucket: Bucket,
            donation_is_quote: bool
        ) {
            let (address, mut pool) = match donation_is_quote {
                true => (
                    self.quote_address,
                    self.quote_pool,
                ),
                false => (
                    self.base_address,
                    self.base_pool,
                ),
            };
            assert!(donation_bucket.resource_address() == address, "Wrong token");

            // Transfer the donation to the pool
            pool.protected_deposit(donation_bucket);
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(
            &self,
            input_amount: Decimal,
            input_is_quote: bool
        ) -> (Decimal, Decimal, Decimal) {
            assert!(input_amount > ZERO, "Invalid input amount");

            let (pool, _p_ref, _incoming) = self.select_pool(input_is_quote); 
            let reserves = pool.get_vault_amounts();
            let available = *reserves.get_index(0).unwrap().1;

            let amount = input_amount.min(available);
            let remainder = input_amount - amount;
            let fee = ZERO;

            (amount, remainder, fee)
        }

        // Select which of the liquidity pools and corresponding target ratio we're working with
        fn select_pool(&self, input_is_quote: bool) -> (&Global<TwoResourcePool>, Decimal, bool) {
            let p_ref = self.state.p0;
            let p_ref_inv = ONE / p_ref;
            match (self.state.shortage, input_is_quote) {
                (Shortage::BaseShortage, true) => (&self.base_pool, p_ref, false),
                (Shortage::BaseShortage, false) => (&self.base_pool, p_ref, true),
                (Shortage::Equilibrium, true) => (&self.base_pool, p_ref, false),
                (Shortage::Equilibrium, false) => (&self.quote_pool, p_ref_inv, false),
                (Shortage::QuoteShortage, true) => (&self.quote_pool, p_ref_inv, true),
                (Shortage::QuoteShortage, false) => (&self.quote_pool, p_ref_inv, false),
            }
        }
    }
}
