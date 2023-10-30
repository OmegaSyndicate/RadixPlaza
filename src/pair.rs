use scrypto::prelude::*;
use crate::constants::*;
use crate::curves::*;
use crate::events::*;
use crate::helpers::*;
use crate::types::*;

#[blueprint]
#[events(SwapEvent, AddLiquidityEvent, RemoveLiquidityEvent)]
mod plazapair {
    enable_package_royalties! {
        instantiate_pair => Free;
        add_liquidity => Free;
        remove_liquidity => Free;
        quote => Free;
        swap => _SWAP_ROYALTY;
    }

    /// `PlazaPair` struct represents a liquidity pair with fixed configuration
    ///
    /// ## Description
    ///
    /// The pair is a generalization of the Dodo liquidity pool. It allows users to trade between a BASE token
    /// and a QUOTE token, as well as supply liquidity to it. It keeps track of BASE and QUOTE liquidity separately,
    /// always pushing the pool back towards equilibrium. The key mechanism that it uses to achieve this is by varying
    /// the degree of liquidity concentration on trades depending on their direction wrt equilibrium.
    ///
    /// A minimum liquidity map (`min_liquidity`) is maintained to hold tiny amounts of tokens to work around
    /// restrictions of the native Radix LP components.
    ///
    /// This struct also stores configuration and state information for this pair.
    struct PlazaPair {
        /// Pool configuration details like fee and other operational parameters.
        config: PairConfig,
        /// Contains dynamic values like the reference price and target ratio.
        state: PairState,
        /// ResourceAddress for the base token in the pair.
        base_address: ResourceAddress,
        /// ResourceAddress for the quote token in the pair.
        quote_address: ResourceAddress,
        /// Represents a pool that primarily holds base tokens plus potentially some quote tokens held in their place.  
        base_pool: Global<TwoResourcePool>,
        /// Represents a pool that primarily holds quote tokens plus potentially some base tokens held in their place.  
        quote_pool: Global<TwoResourcePool>,
        /// Maps pools to a Vault that holds a tiny amount of tokens to workaround native LP restrictions.
        min_liquidity: HashMap<                          
            ComponentAddress,
            Vault
        >,
    }

    impl PlazaPair {
        /// Instantiates a new PlazaPair.
        /// 
        /// The function sets the state of the newly created PlazaPair and creates two `TwoResourcePool`s (base and 
        /// quote). It asserts a series of conditions relating to the input parameters to ensure they are within
        /// acceptable ranges. It also creates two Vaults for the minimal liquidity. This is used in `add_liquidity`
        /// operations if the pool is empty. The pair will be initialised without liquidity, so liquidity needs
        /// to be added before any trades can take place.
        ///
        /// # Arguments
        ///
        /// * `owner_role` - the owner of PlazaPair.
        /// * `base_bucket` - The bucket for the base token.
        /// * `quote_bucket` - The bucket for the quote token.
        /// * `config` - A `PairConfig` instance containing the configuration for the PlazaPair.
        /// * `initial_price` - The initial price of the currency pair.
        ///
        /// # Panics
        ///
        /// This function will panic if constraints on the parameters are not met (such as invalid base amount,
        /// quote amount, dissolve delay, fee level or decay factor).
        ///
        /// # Returns
        ///
        /// * A `Global<PlazaPair>` instance.
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
            assert!(config.k_out < CLIP_K_OUT_1 || config.k_out == ONE || config.k_out > CLIP_K_OUT_2, "Invalid k_out value");
            assert!(config.fee >= ZERO && config.fee < ONE_TENTH, "Invalid fee level");
            assert!(config.decay_factor >= ZERO && config.decay_factor < ONE, "Invalid decay factor");
            assert!(initial_price > ZERO, "Invalid price");

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
            assert!(base_manager.resource_type().divisibility().unwrap() >= 6, "Bad base divisibility");
            assert!(quote_manager.resource_type().divisibility().unwrap() >= 6, "Bad quote divisibility");

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

        /// Adds liquidity to the appropriate pool in return for LP tokens.
        ///
        /// This function takes an `input_bucket` representing the amount of liquidity to be added to the pool. 
        /// The `is_quote` boolean determines whether the liquidity is added to the quote pool (true) or the 
        /// base pool (false). Depending on the state of the selected pool and if it's in shortage, the function
        /// performs various calculations to ensure the correct amount of LP tokens are minted and returned.
        ///
        /// # Arguments
        ///
        /// * `input_bucket`: A `Bucket` object representing the amount of liquidity being added to the pool.
        /// * `is_quote`: A flag indicating if the tokens should be added to the quote pool (true) or the base
        ///    pool (false).
        ///
        /// # Returns
        ///
        /// * A `Bucket` object representing the amount of LP tokens received in exchange for the added liquidity.
        ///
        /// # Panics
        ///
        /// * This function may panic if it cannot find the remainder bucket from the pool's `contribute`
        ///   call when the pool is not in shortage.
        ///
        /// # Events
        ///
        /// * An `AddLiquidityEvent` is emitted after the liquidity has been successfully added to the pool.
        pub fn add_liquidity(
            &mut self,
            input_bucket: Bucket,
        ) -> Bucket {
            let is_quote = input_bucket.resource_address() == self.quote_address;

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
            let min_liq_mgr = ResourceManager::from(min_liq_addr);
            let min_liq_div = min_liq_mgr.resource_type().divisibility().unwrap();
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

                    // Beef up amount of LP tokens by a factor of 100 for non-technical reasons
                    let scale_bucket_input = pool.protected_withdraw(
                        input_address,
                        input_amount * (ONE_HUNDRED - ONE) / ONE_HUNDRED,
                        WithdrawStrategy::Rounded(RoundingMode::ToZero)
                    );
                    let scale_bucket_minliq = pool.protected_withdraw(
                        min_liq_addr,
                        MIN_LIQUIDITY * (ONE_HUNDRED - ONE) / ONE_HUNDRED,
                        WithdrawStrategy::Rounded(RoundingMode::ToZero)
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
                        tiny_bucket.take(FEMTO.checked_round(min_liq_div, RoundingMode::AwayFromZero).unwrap())
                    );
                    let (lp_tokens, remainder) = pool.contribute((input_bucket, tiny_bucket));
                    if let Some(bucket) = remainder {
                        assert!(bucket.resource_address() == min_liq_addr, "Added too many tokens");
                        pool.protected_deposit(bucket);
                    }
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
                        WithdrawStrategy::Rounded(RoundingMode::ToZero)
                    );

                    // Finally add the liquidity and add back the remainder
                    let (lp_tokens, remainder) = pool.contribute((input_bucket, other_bucket));
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

        /// The function `remove_liquidity` is designed to process the withdrawal of liquidity from a specified
        /// pool component. It takes in a Bucket called `lp_bucket` which contains the liquidity pair to be redeemed
        /// and a boolean `is_quote` to flag whether the LP tokens represent the base or quote pool within the pair.
        ///
        /// # Arguments
        ///
        /// * `lp_bucket` - A Bucket instance that holds the liquidity pair to be removed from the pool.
        /// * `is_quote` - A boolean flag indicating the pool type. If true, refers to the quote_pool. If false, refers to the base_pool.
        ///
        /// This function retrieves and redeems the liquidity from the relevant pool which results in two buckets 
        /// (main and other bucket), representing the rest of the liquidity after the redemption. It also emits a 
        /// RemoveLiquidityEvent with the necessary details.
        ///
        /// # Returns
        ///
        /// * `(Bucket, Bucket)` - A tuple containing two Buckets which represent the remaining main_bucket and other_bucket 
        ///   after the liquidity redemption process.
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

        /// Executes a token swap operation using the 'input_bucket', adjusting the state of the liquidity pair and 
        /// managing the liquidity pools accordingly. It calculates the tokens received from the swap operation, 
        /// emits a 'SwapEvent' for logging, and processes any fee applied. In the absence of sufficient liquidity 
        /// in the pool or if the 'input_bucket' is not fully spent, it returns the remaining unspent tokens as 
        ///  'remainder'.
        ///
        /// # Arguments
        /// * `input_bucket: Bucket` - The mutable bucket carrying the tokens set for the swap. A precondition 
        /// for the operation is that the 'input_bucket' should not be empty, as it will cause the function to panic.
        ///
        /// # Returns
        /// The function returns a tuple:
        /// * `output_bucket: Bucket` - The bucket representing the tokens gained from the swap.
        /// * `remainder: Option<Bucket>` - Contains the unspent tokens after the swap operation. In case there is 
        ///   nothing left unspent, None is returned.
        ///
        /// # Panics
        /// The function will raise a panic if:
        ///  * The 'input_bucket' is found empty.
        ///  * The calculated output token amount is not equal to the actual token amount in the 'output_bucket'.
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

        /// Processes a token donation and deposits it into the appropriate liquidity pool. The function handles 
        /// both types of tokens, Quote and Base, with the help of a boolean flag. If the donated token is of the
        /// type that is currently in shortage, it recalculates the target ratio for the corresponding pool. 
        /// The token type of the `donation_bucket` must match that of the pool it's being donated to.
        ///
        /// # Arguments
        /// * `donation_bucket: Bucket` - The bucket containing the donated tokens.
        /// * `donation_is_quote: bool` - Specifies whether the tokens in the donation_bucket are of Quote type. 
        ///   If it's 'false', the tokens are considered of Base type.
        ///
        /// # Panics
        /// This function will panic if the token type in the `donation_bucket` does not match the token type to 
        /// which it's intended to be donated (Quote/Base as signaled by the `donation_is_quote` flag).
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

        /// Generates a quotation from the AMM for a token swap operation considering the amount of the input tokens 
        /// and whether the type of those tokens is Quote or Base. Calculates and returns the possible outcome 
        /// amount after trade, any remaining amount from the input tokens after the trade, the transaction fee 
        /// applied, allocated changes to liquidity pools, and the updated state of the pair. The function ensures
        /// that the input amount is greater than zero and that the traded amount doesn't exceed the input amount.
        ///
        /// # Arguments
        /// * `input_amount: Decimal` - Denotes the volume of tokens to be traded.
        /// * `input_is_quote: bool` - Indicates if the input token type is quote, 'false' for base tokens.
        ///
        /// # Returns
        /// Returns a tuple comprising:
        /// * `output_amount: Decimal` - The resultant amount of tokens possible to gain from the trade.
        /// * `remainder: Decimal` - Any unspent portion of the input after the trade.
        /// * `fee: Decimal` - The fee incurred from the trade.
        /// * `allocation: TradeAllocation` - Denotes the adjustments in the liquidity pools following the trade.
        /// * `new_state: PairState` - The updated state of the trading pair after the trade.
        ///
        /// # Panics
        /// The function will panic if:
        /// * The `input_amount` is zero or less.
        /// * If the traded volume exceeds the `input_amount`.
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
            let mut p_ref = factor * old_pref + (ONE - factor) * p_ref_ss;

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
                    // Set to equilibrium and switch pools
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

                    // Update running parameters for possible outgoing leg
                    output_amount = surplus;
                    amount_traded = adjusted_shortfall;
                    p_ref = ONE / p_ref;
                    p_ref_ss = p_ref;
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

        /// Determines the liquidity pool to be used and its affiliated target ratio based on the current pair state 
        /// and whether the input token type is Quote or Base. Additionally, it identifies if there is a need to 
        /// invert the operational mode of the pool based on the token type and its corresponding shortage.
        ///
        /// # Arguments
        /// * `state: &PairState` - Reference to the current Pair State.
        /// * `input_is_quote: bool` - Flag indicating if the input token type is quote ('false' for base tokens).
        ///
        /// # Returns
        /// A tuple consisting of:
        /// * `&Global<TwoResourcePool>` - The chosen liquidity pool reference.
        /// * `Decimal` - The target ratio associated with the selected pool.
        /// * `bool` - A boolean value indicating whether the resource pool's inputs require inversion.
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

        /// Performs an evaluation of a provided liquidity pool, extracting crucial numerical information relevant 
        /// for trading calculations. Specifically, it derives 'actual', 'surplus', and 'shortfall' amounts from
        /// the pool utilising the 'target_ratio' for shortfall computation.
        ///
        /// # Arguments
        /// * `pool: &Global<TwoResourcePool>` - A reference to the pool which is to be assessed.
        /// * `target_ratio: Decimal` - The specific ratio that is used in computing the shortfall.
        ///
        /// # Return
        /// Returns a tuple of Decimals containing:
        /// * `actual: Decimal` - Represents the current amount of desired primary tokens in the liquidity pool.
        /// * `surplus: Decimal` - Represents the current amount of less desired secondary tokens within the pool.
        /// * `shortfall: Decimal` - Represents the shortage of primary tokens in the pool.
        fn  assess_pool(&self, pool: &Global<TwoResourcePool>, target_ratio: Decimal) -> (Decimal, Decimal, Decimal) {
            let reserves = pool.get_vault_amounts();
            let actual = *reserves.get_index(0).map(|(_addr, amount)| amount).unwrap();
            let surplus = *reserves.get_index(1).map(|(_addr, amount)| amount).unwrap();
            let shortfall = target_ratio * actual - actual;
            (actual, surplus, shortfall)
        }
    }
}
