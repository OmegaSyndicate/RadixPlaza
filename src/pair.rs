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
            let (pool, in_shortage) = match is_quote {
                true => {
                    let in_shortage = self.state.shortage == Shortage::QuoteShortage;
                    (&mut self.quote_pool, in_shortage)
                }
                false => {
                    let in_shortage = self.state.shortage == Shortage::BaseShortage;
                    (&mut self.base_pool, in_shortage)
                }
            };

            let input_amount = input_bucket.amount();
            let reserve = *pool.get_vault_amounts().get_index(0).map(|(_addr, amount)| amount).unwrap();
            let min_liq = self.min_liquidity.get_mut(&pool.address()).unwrap();
            let min_liq_addr = min_liq.resource_address();
            let mut tiny_bucket = min_liq.take(MIN_LIQUIDITY);

            let lp_bucket = match (reserve == ZERO, in_shortage) {
                // No liquidity present at the moment
                (true, _) => {
                    // Reset to equilbrium if this token was in shortage
                    if in_shortage {
                        self.state.shortage = Shortage::Equilibrium;
                        self.state.target_ratio = ONE;
                        self.state.last_out_spot = self.state.p0;
                    }

                    // Simply add the tokens plus the min_liq bucket first
                    let input_address = input_bucket.resource_address();
                    let (mut lp_tokens, _) = pool.contribute((input_bucket, tiny_bucket));

                    // Beef up amount of LP tokens by a factor of 1000 for non-technical reasons
                    let scale_bucket_input = pool.protected_withdraw(
                        input_address,
                        input_amount * (ONE_THOUSAND - ONE) / ONE_THOUSAND,
                        WithdrawStrategy::Exact
                    );
                    let scale_bucket_minliq = pool.protected_withdraw(
                        min_liq_addr,
                        MIN_LIQUIDITY * (ONE_THOUSAND - ONE) / ONE_THOUSAND,
                        WithdrawStrategy::Exact
                    );
                    let (scale_lp, remainder) = pool.contribute((scale_bucket_input, scale_bucket_minliq));
                    if let Some(bucket) = remainder {
                        pool.protected_deposit(bucket);
                    }

                    // Take back the min_liq and put it back into the vault
                    min_liq.put(
                        pool.protected_withdraw(min_liq_addr, MIN_LIQUIDITY, WithdrawStrategy::Exact)
                    );

                    // Return the collected LP tokens to the user
                    lp_tokens.put(scale_lp);
                    lp_tokens
                }
                // Not in shortage, can just add in ratio
                (false, false) => {
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
                // The most difficult case. Pool is in shortage, need to calculate precisely
                (false, true) => {
                    // We don't need our tiny bucket in this scenario
                    min_liq.put(tiny_bucket);

                    // Retrieve surplus and target ratio values
                    let (&surplus_address, &surplus) = pool.get_vault_amounts().get_index(1).unwrap();
                    let target_ratio = self.state.target_ratio;
                    let shortfall = target_ratio * reserve - reserve;

                    // Compute time since previous trade and resulting decay factor for the filter
                    let t = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
                    let delta_t = (t - self.state.last_outgoing).max(0);
                    let factor = Decimal::checked_powi(&self.config.decay_factor, delta_t / 60).unwrap();

                    // Caculate the filtered reference price
                    let old_pref = match is_quote {
                        true => self.state.p0,
                        false => ONE / self.state.p0,
                    };
                    let p_ref_ss = calc_p0_from_curve(shortfall, surplus, target_ratio, self.config.k_in);
                    let p_ref = factor * old_pref + (ONE - factor) * p_ref_ss;

                    // Calculate the target ratios before and after the add
                    let new_actual = reserve + input_amount;
                    let target = calc_target_ratio(p_ref, reserve, surplus, self.config.k_in) * reserve;
                    let new_target = calc_target_ratio(p_ref, new_actual, surplus, self.config.k_in) * new_actual;

                    // Withdraw the correct fraction of surplus to add back
                    let new_lp_fraction = (new_target - target) / new_target;
                    let other_bucket = pool.protected_withdraw(
                        surplus_address,
                        new_lp_fraction * surplus,
                        WithdrawStrategy::Exact
                    );

                    // Also withdraw a complement of the main reserve
                    let complement = new_lp_fraction * new_actual - input_amount;
                    let mut complement_bucket = pool.protected_withdraw(
                        min_liq_addr,
                        complement,
                        WithdrawStrategy::Exact
                    );
                    complement_bucket.put(input_bucket);

                    // Finally add the liquidity and add back the remainder
                    let (lp_tokens, remainder) = pool.contribute((complement_bucket, other_bucket));
                    if let Some(bucket) = remainder {
                        pool.protected_deposit(bucket);
                    }

                    // Update the target ratio and return the lp_tokens
                    self.state.target_ratio = new_target / new_actual;
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
            let pool = match is_quote {
                true => &mut self.quote_pool,
                false => &mut self.base_pool,
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
            let (output_amount, _remainder, fee, allocation, new_state) = self.quote(input_amount, input_is_quote);

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
                    deposit_to_pool(base_pool, &mut input_bucket, allocation.base_quote);
                    deposit_to_pool(quote_pool, &mut input_bucket, allocation.quote_quote);
                    
                    let mut bucket = Bucket::new(self.base_address);
                    withdraw_from_pool(base_pool, &mut bucket, allocation.base_base);
                    withdraw_from_pool(quote_pool, &mut bucket, allocation.quote_base);
                    bucket
                },
                false => {
                    deposit_to_pool(base_pool, &mut input_bucket, allocation.base_base);
                    deposit_to_pool(quote_pool, &mut input_bucket, allocation.quote_base);
                    
                    let mut bucket = Bucket::new(self.quote_address);
                    withdraw_from_pool(base_pool, &mut bucket, allocation.base_quote);
                    withdraw_from_pool(quote_pool, &mut bucket, allocation.quote_quote);
                    bucket
                }
            };

            // Adjust pair state and donate the fee
            self.state = new_state;
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
        fn donate_to_pool(
            &mut self,
            donation_bucket: Bucket,
            donation_is_quote: bool
        ) {
            let (address, in_shortage) = match donation_is_quote {
                true => (self.quote_address, self.state.shortage == Shortage::QuoteShortage),
                false => (self.base_address, self.state.shortage == Shortage::BaseShortage),
            };
            assert!(donation_bucket.resource_address() == address, "Wrong token");

            // Update target ratio if the donated token is in shortage
            if in_shortage {
                let target_ratio = self.state.target_ratio;
                let (actual, _surplus, _shortfall) = match donation_is_quote {
                    true => self.assess_pool(&self.quote_pool, target_ratio),
                    false => self.assess_pool(&self.base_pool, target_ratio),
                }; 
                let donation_amount = donation_bucket.amount();
                let new_actual = actual + donation_amount;
                let new_target = target_ratio * actual + donation_amount;
                self.state.target_ratio = new_target / new_actual;
            }

            // Transfer the donation to the pool
            match donation_is_quote {
                true => &self.quote_pool.protected_deposit(donation_bucket),
                false => &self.base_pool.protected_deposit(donation_bucket),
            };
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(
            &self,
            input_amount: Decimal,
            input_is_quote: bool
        ) -> (Decimal, Decimal, Decimal, TradeAllocation, PairState) {
            assert!(input_amount > ZERO, "Invalid input amount");
            let mut new_state = self.state;

            // Check which pool we're workings with and extract relevant values
            let (mut pool, old_pref, incoming) = self.select_pool(&new_state, input_is_quote);
            let (mut actual, surplus, shortfall) = self.assess_pool(&pool, new_state.target_ratio);

            // Compute time since previous trade and resulting decay factor for the filter
            let t = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            let delta_t = (t - new_state.last_outgoing).max(0);
            let factor = Decimal::checked_powi(&self.config.decay_factor, delta_t / 60).unwrap();

            // Caculate the filtered reference price
            let mut p_ref_ss = match shortfall > ZERO {
                true => calc_p0_from_curve(shortfall, surplus, new_state.target_ratio, self.config.k_in),
                false => old_pref,
            };
            let p_ref = factor * old_pref + (ONE - factor) * p_ref_ss;

            // Define running counters
            let mut amount_traded = ZERO;
            let mut output_amount = ZERO;
            let (mut base_base, mut base_quote, mut quote_base, mut quote_quote) = (ZERO, ZERO, ZERO, ZERO);

            // Handle the incoming case (trading towards equilibrium). We project the current reserves on the
            // incoming curve by calculating a adjusted target value to reach equilibrium and spend all surplus
            // counter tokens. If we go past equilibrium, we update state accordingly. Note that we ignore the
            // stored target value to elegantly deal with the excess tokens from earlier from the sparser
            // liquidity on the curve trading away from equilibrium.
            if incoming {
                let adjusted_target = match actual > ZERO {
                    true => calc_target_ratio(p_ref, actual, surplus, self.config.k_in) * actual,
                    false => ZERO,
                };
                let adjusted_shortfall = adjusted_target - actual;

                // If we add more than required to reach equilibrium, we reset to equilibrium and continue the
                // trade on the outgoing curve below.
                if input_amount < adjusted_shortfall {
                    // If we stay in the same shortage situation, we calculate according to the incoming curve.
                    output_amount = calc_incoming(
                        input_amount,
                        adjusted_target,
                        actual,
                        p_ref,
                        self.config.k_in,
                    );
                    amount_traded = input_amount;

                    // Update target ratio (mop up excess surplus tokens into target)
                    let new_actual = actual + input_amount;
                    let new_surplus = surplus - output_amount;
                    new_state.target_ratio = calc_target_ratio(p_ref_ss, new_actual, new_surplus, self.config.k_in);
                } else {
                    // Update running parameters for possible outgoing leg
                    output_amount = surplus;
                    amount_traded = adjusted_shortfall;
                    p_ref_ss = ONE / p_ref;    

                    // Set to equilibrium and switch pools for outgoing leg
                    new_state.shortage = Shortage::Equilibrium;
                    new_state.target_ratio = ONE;
                    pool = match pool == &self.base_pool {
                        true => {
                            new_state.last_out_spot = p_ref;
                            new_state.p0 = p_ref;
                            &self.quote_pool
                        },
                        false => {
                            new_state.last_out_spot = ONE / p_ref;
                            new_state.p0 = ONE / p_ref;
                            &self.base_pool
                        },
                    };
                    let reserves = pool.get_vault_amounts();
                    actual = *reserves.get_index(0).map(|(_addr, amount)| amount).unwrap();                    
                }
            }

            // Allocate pool changes
            match input_is_quote {
                true => {
                    quote_base = output_amount;
                    quote_quote = amount_traded;
                },
                false => {
                    base_base = amount_traded;
                    base_quote = output_amount;
                }
            };

            // Handle the trading away from equilbrium case
            if amount_traded < input_amount && actual > ZERO {
                let last_outgoing_spot = match pool == &self.base_pool {
                    true => new_state.last_out_spot,
                    false => ONE / new_state.last_out_spot,
                };

                // Calibrate outgoing price curve to filtered spot price.
                let incoming_spot = calc_spot(p_ref_ss, new_state.target_ratio, self.config.k_in);
                let outgoing_spot = factor * last_outgoing_spot + (ONE - factor) * incoming_spot;
                let virtual_p_ref = calc_p0_from_spot(outgoing_spot, new_state.target_ratio, self.config.k_out);

                // Calculate output amount based on outgoing curve
                let target = actual * new_state.target_ratio;
                let outgoing_amount = calc_outgoing(
                    input_amount - amount_traded,
                    target,
                    actual,
                    virtual_p_ref,
                    self.config.k_out,
                );
                output_amount += outgoing_amount;

                // Calculate new values and allocate pool changes
                let new_actual = actual - outgoing_amount;
                match input_is_quote {
                    true => {
                        base_base = outgoing_amount;
                        base_quote = input_amount - amount_traded;
                    },
                    false => {
                        quote_base = input_amount - amount_traded;
                        quote_quote = outgoing_amount;
                    }
                };
                amount_traded = input_amount;

                // Update pair state variables. Target stays the same, but ratio needs to be updated.
                new_state.last_outgoing = t;
                new_state.target_ratio = target / new_actual;
                (new_state.shortage, new_state.last_out_spot, new_state.p0) = match pool == &self.base_pool {
                    true => (
                        Shortage::BaseShortage,
                        calc_spot(virtual_p_ref, new_state.target_ratio, self.config.k_out),
                        p_ref,
                    ),
                    false => (
                        Shortage::QuoteShortage,
                        ONE / calc_spot(virtual_p_ref, new_state.target_ratio, self.config.k_out),
                        ONE / p_ref,
                    ),
                };
            }

            // No liquidity in outgoing direction, reset to equilibrium
            if amount_traded < input_amount {
                new_state.shortage = Shortage::Equilibrium;
                new_state.target_ratio = ONE;
                new_state.last_out_spot = new_state.p0;
                new_state.last_outgoing = t;
            }

            // Calculate output variables
            let fee = self.config.fee * output_amount;
            let remainder = input_amount - amount_traded;
            let allocation = TradeAllocation{base_base, base_quote, quote_base, quote_quote};

            (output_amount - fee, remainder, fee, allocation, new_state)
        }

        // Select which of the liquidity pools and corresponding target ratio we're working with
        fn select_pool(&self, state: &PairState, input_is_quote: bool) -> (&Global<TwoResourcePool>, Decimal, bool) {
            let p_ref = state.p0;
            let p_ref_inv = ONE / p_ref;
            match (state.shortage, input_is_quote) {
                (Shortage::BaseShortage, true) => (&self.base_pool, p_ref, false),
                (Shortage::BaseShortage, false) => (&self.base_pool, p_ref, true),
                (Shortage::Equilibrium, true) => (&self.base_pool, p_ref, false),
                (Shortage::Equilibrium, false) => (&self.quote_pool, p_ref_inv, false),
                (Shortage::QuoteShortage, true) => (&self.quote_pool, p_ref_inv, true),
                (Shortage::QuoteShortage, false) => (&self.quote_pool, p_ref_inv, false),
            }
        }

        fn  assess_pool(&self, pool: &Global<TwoResourcePool>, target_ratio: Decimal) -> (Decimal, Decimal, Decimal) {
            let reserves = pool.get_vault_amounts();
            let actual = *reserves.get_index(0).map(|(_addr, amount)| amount).unwrap();
            let surplus = *reserves.get_index(1).map(|(_addr, amount)| amount).unwrap();
            let shortfall = target_ratio * actual - actual;
            (actual, surplus, shortfall)
        }
    }
}
