use scrypto::prelude::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum PairState {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
}

#[derive(ScryptoSbor)]
pub struct PairParams {
    p0: Decimal,
    // Target amount of base tokens
    base_target: Decimal,
    // Target amount of quote tokens
    quote_target: Decimal,
    // Current state of the pair
    state: PairState,
    // Ingress price curve exponent
    k_in: Decimal,
    // Egress price curve exponent
    k_out: Decimal,
    // Trading fee
    fee: Decimal,
    // Timestamp of last trade
    last_trade: i64,
    // Timestamp of last trade
    last_outgoing: i64,
    // Timestamp of last trade
    last_out_spot: Decimal,
}

impl fmt::Display for PairState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PairState::BaseShortage => write!(f, "BaseShortage"),
            PairState::Equilibrium => write!(f, "Equilibrium"),
            PairState::QuoteShortage => write!(f, "QuoteShortage"),
        }
    }
}

#[blueprint]
mod plazapair {
    // PlazaPair struct represents a liquidity pair in the trading platform
    struct PlazaPair {
        // Price at equilibrium
        p0: Decimal,
        // Target amount of base tokens
        base_target: Decimal,
        // Target amount of quote tokens
        quote_target: Decimal,
        // Vault holding base tokens
        base_vault: Vault,
        // Vault holding quote tokens
        quote_vault: Vault,
        // Current state of the pair
        state: PairState,
        // Ingress price curve exponent
        k_in: Decimal,
        // Egress price curve exponent
        k_out: Decimal,
        // Resource address of base LP tokens
        base_lp: ResourceAddress,
        // Resource address of quote LP tokens
        quote_lp: ResourceAddress,
        // Vault holding admin badge
        lp_badge: Vault,
        // Trading fee
        fee: Decimal,
        // Timestamp of last trade
        last_trade: i64,
        // Timestamp of last trade
        last_outgoing: i64,
        // Timestamp of last trade
        last_out_spot: Decimal,
    }

    impl PlazaPair {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_pair(
            initial_base: Bucket,
            initial_quote: Bucket,
            price: Decimal,
        ) -> (PlazaPairComponent, Bucket, Bucket) {
            // Create internal admin badge
            let lp_badge: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "admin badge")
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(1);

            // Create LP tokens for the base token providers, starting at 1:1
            let base_lp_bucket: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "PlazaPair Base LP")
                .metadata("symbol", "PLAZALP")
                .mintable(rule!(require(lp_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lp_badge.resource_address())), LOCKED)
                .mint_initial_supply(initial_base.amount());

            // Create LP tokens for the quote token providers, starting at 1:1
            let quote_lp_bucket: Bucket = ResourceBuilder::new_fungible()
                .metadata("name", "PlazaPair Quote LP")
                .metadata("symbol", "PLAZALP")
                .mintable(rule!(require(lp_badge.resource_address())), LOCKED)
                .burnable(rule!(require(lp_badge.resource_address())), LOCKED)
                .mint_initial_supply(initial_quote.amount());

            // Instantiate a PlazaPair component
            let pair = Self {
                p0: price,
                base_target: initial_base.amount(),
                quote_target: initial_quote.amount(),
                base_vault: Vault::with_bucket(initial_base),
                quote_vault: Vault::with_bucket(initial_quote),
                state: PairState::Equilibrium,
                k_in: dec!("0.4"),
                k_out: dec!("1"),
                base_lp: base_lp_bucket.resource_address(),
                quote_lp: quote_lp_bucket.resource_address(),
                lp_badge: Vault::with_bucket(lp_badge),
                fee: dec!("0.003"),
                last_trade: Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch,
                last_outgoing: Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch,
                last_out_spot: price,
            }
            .instantiate();

            (pair, base_lp_bucket, quote_lp_bucket)
        }

        pub fn add_liquidity(&mut self, input_bucket: Bucket) -> Bucket {
            // Ensure the bucket is not empty
            assert!(input_bucket.amount() > dec!(0), "Empty bucket provided");
        
            // Determine if the bucket is for the quote or the base pool
            let is_quote = self.quote_vault.resource_address() == input_bucket.resource_address();
        
            // Based on the bucket type, choose the correct vault, target and resource address
            let (vault, mut target_value, lp_address) = if is_quote {
                (&mut self.quote_vault, self.quote_target, self.quote_lp)
            } else {
                (&mut self.base_vault, self.base_target, self.base_lp)
            };
        
            // Borrow the liquidity pool manager resource & check supply
            let lp_manager = borrow_resource_manager!(lp_address);
            let lp_outstanding = lp_manager.total_supply();
        
            // Calculate the new LP amount
            let new_lp_value = if lp_outstanding == dec!(0) { 
                // If the LP supply is zero, the new LP value is the input amount
                input_bucket.amount()
            } else {
                // Otherwise, it's calculated based on the input amount, target and LP supply
                input_bucket.amount() / target_value * lp_outstanding
            };
        
            // Add the new liquidity to the target and deposit into the vault
            target_value += input_bucket.amount();
            vault.put(input_bucket);
        
            // Authorize the minting of new LP value and return the updated bucket
            self.lp_badge.authorize(|| lp_manager.mint(new_lp_value))
        }

        // Exchange LP tokens for the underlying liquidity held in the pair
        // TODO -- RESET TARGETS ON PRICE X-OVER
        // TODO -- ENSURE HEALTH WITH ZERO LIQ
        pub fn remove_liquidity(&mut self, lp_tokens: Bucket) -> (Bucket, Bucket) {
            let lp_manager = borrow_resource_manager!(lp_tokens.resource_address());
            let lp_outstanding = lp_manager.total_supply();
            let is_quote = lp_tokens.resource_address() == self.quote_lp;

            // Determine which vault and target values should be used
            let (this_vault, other_vault, this_target, other_target, is_shortage) = if is_quote {
                (
                    &mut self.quote_vault,
                    &mut self.base_vault,
                    &mut self.quote_target,
                    self.base_target,
                    self.state == PairState::QuoteShortage,
                )
            } else {
                (
                    &mut self.base_vault,
                    &mut self.quote_vault,
                    &mut self.base_target,
                    self.quote_target,
                    self.state == PairState::BaseShortage,
                )
            };

            // Calculate fraction of liquidity being withdrawn
            let fraction = lp_tokens.amount() / lp_outstanding;

            // Calculate how many tokens are represented by the withdrawn LP tokens
            let (this_amount, other_amount) = if is_shortage {                
                let surplus = other_vault.amount() - other_target;
                (
                    fraction * this_vault.amount(),
                    fraction * surplus,
                )
            } else {
                (
                    fraction * *this_target,
                    dec!(0),
                )
            };

            // Burn the LP tokens and update the target value
            self.lp_badge.authorize(|| { lp_tokens.burn(); });
            *this_target -= fraction * *this_target;

            // Take liquidity from the vault and return to the caller
            (this_vault.take(this_amount), other_vault.take(other_amount))
        }

        /// Swap a bucket of tokens along the AMM curve.
        pub fn swap(&mut self, input_tokens: Bucket) -> Bucket {
            // Determine if the input tokens are quote tokens or base tokens.
            let is_quote = input_tokens.resource_address() == self.quote_vault.resource_address();

            // Calculate the amount of output tokens and pair impact variables.
            let (output_amount, fee, new_p0, new_state, t) = self.quote(input_tokens.amount(), is_quote);

            // Log trade event
            let (token_in, token_out) = if is_quote {
                ("quote", "base")
            } else {
                ("base", "quote")
            };
            info!("SWAP: {} {} for {} {}.", input_tokens.amount(), token_in, output_amount, token_out);

            // Adjust pair state variables.
            self.p0 = new_p0;
            self.state = new_state;
            self.last_trade = t;
            debug!("p0: {} -- state {}", new_p0, new_state);

            // Update the target values and select the input and output vaults based on input_tokens type.
            let (input_vault, output_vault) = if is_quote {
                self.base_target += fee;
                (&mut self.quote_vault, &mut self.base_vault)
            } else {
                self.quote_target += fee;
                (&mut self.base_vault, &mut self.quote_vault)
            };

            // Transfer the tokens.
            input_vault.put(input_tokens);
            output_vault.take(output_amount)
        }

        // Getter function to identify related LP tokens
        pub fn get_lp_tokens(&self) -> (ResourceAddress, ResourceAddress) {
            (self.base_lp, self.quote_lp)
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(&self, input_amount: Decimal, input_is_quote: bool) -> (Decimal, Decimal, Decimal, PairState, i64) {
            // Compute time since previous trade
            let t = Clock::current_time_rounded_to_minutes().seconds_since_unix_epoch;
            let delta_t = if t > self.last_trade { 
                t - self.last_trade
            } else { 
                0
            };

            // Compute decay factor with a 15 minute time constant
            let factor = Decimal::powi(&dec!("0.9355"), delta_t / 60);

            // Collect current actual balances
            let (base_actual, quote_actual) = (self.base_vault.amount(), self.quote_vault.amount());

            // Set the input and output vaults and targets based on input_tokens type
            let (input_actual, mut output_actual, input_target, output_target) =
                if input_is_quote {
                    (
                        quote_actual,
                        base_actual,
                        self.quote_target,
                        self.base_target,
                    )
                } else {
                    (
                        base_actual,
                        quote_actual,
                        self.base_target,
                        self.quote_target,
                    )
                };            

            // Calculate the base[quote] price in equilibrium
            let new_p0 = match self.state {
                PairState::BaseShortage => {
                    let quote_surplus = quote_actual - self.quote_target;
                    self.calc_p0(quote_surplus, self.base_target, base_actual)
                }
                PairState::Equilibrium => self.p0,
                PairState::QuoteShortage => {
                    let base_surplus = base_actual - self.base_target;
                    dec!(1) / self.calc_p0(base_surplus, self.quote_target, quote_actual)
                }
            };
            let p0 = factor * new_p0 + (dec!(1) - factor) * self.p0;

            // Set reference price to B[Q] or Q[B] depending on liquidity
            let mut p_ref = match self.state {
                PairState::BaseShortage => p0,
                PairState::Equilibrium => if input_is_quote { p0 } else { dec!(1) / p0 },
                PairState::QuoteShortage => dec!(1) / p0
            };

            // Determine which state represents input shortage based on input type
            let input_shortage = match input_is_quote {
                true => PairState::QuoteShortage,
                false => PairState::BaseShortage
            };

            // Define running counters
            let mut amount_to_trade = input_amount;
            let mut output_amount = dec!(0);
            let mut exchange_state = self.state;

            // Handle the incoming case (trading towards equilibrium)
            if exchange_state == input_shortage {
                if amount_to_trade + input_actual >= input_target {
                    // Trading past equilibrium
                    amount_to_trade -= input_target - input_actual;
                    output_amount = output_actual - output_target;

                    // Update variables for second part of trade
                    exchange_state = PairState::Equilibrium;
                    output_actual = output_target;
                    p_ref = dec!(1) / p_ref;
                } else {
                    // Trading towards but short of equilibrium
                    output_amount = self.calc_incoming(
                        amount_to_trade,
                        input_target,
                        input_actual,
                        p_ref,
                    );
                }
            }

            // Handle the trading away from equilbrium case
            if exchange_state != input_shortage && amount_to_trade > dec!(0) {
                // Calibrate outgoing price curve to incoming at spot price.
                let target2 = output_target * output_target;
                let actual2 = output_actual * output_actual;
                let num_new = (dec!(1) - factor) * (actual2 + self.k_in * (target2 - actual2));
                let num_old = factor * actual2 * self.last_out_spot;
                let den = actual2 + self.k_out * (target2 - actual2);
                let virtual_p_ref = (num_new + num_old) / den * p_ref;
                
                // Calculate output amount based on outgoing curve
                output_amount += self.calc_outgoing(
                    amount_to_trade,
                    output_target,
                    output_actual,
                    virtual_p_ref,
                );

                // Select what the new exchange state should be
                exchange_state = match input_shortage {
                    PairState::QuoteShortage => PairState::BaseShortage,
                    PairState::BaseShortage => PairState::QuoteShortage,
                    PairState::Equilibrium => {
                        if input_is_quote {
                            PairState::BaseShortage
                        } else {
                            PairState::QuoteShortage
                        }
                    }
                }
            }

            // Apply trading fee
            let fee = self.fee * output_amount;

            (output_amount - fee, fee, p0, exchange_state, t)
        }

        // Calculate the price at equilibrium (p0) given surplus, target, and actual token amounts
        fn calc_p0(&self, surplus: Decimal, target: Decimal, actual: Decimal) -> Decimal {
            // TODO: ADD FILTER!!

            // Calculate the shortage of tokens
            let shortage = target - actual;

            // Calculate the price at equilibrium (p0) using the given formula
            surplus / shortage / (dec!(1) + self.k_in * shortage / actual)
        }
        
        // Calculate the incoming amount of output tokens given input_amount, target, actual, and p_ref
        fn calc_incoming(
            &self,
            input_amount: Decimal,
            target: Decimal,
            actual: Decimal,
            p_ref: Decimal,
        ) -> Decimal {
            // Ensure the sum of the actual and input amounts does not exceed the target
            assert!(actual + input_amount <= target, "Infeasible amount");
            
            // Calculate the existing surplus as per AMM curve
            let surplus_before = (target - actual) * p_ref * target / actual;

            // Calculate the new surplus as per the AMM curve
            let actual_after = actual + input_amount;
            let surplus_after = (target - actual_after) * p_ref * target / actual_after;

            // The difference is the output amount
            surplus_before - surplus_after
        }

        // Calculate the outgoing amount of output tokens given input_amount, surplus, target, actual, and p_ref
        fn calc_outgoing(
            &self,
            input_amount: Decimal,
            target: Decimal,
            actual: Decimal,
            p_ref: Decimal,
        ) -> Decimal {
            // Calculate current shortage
            let shortage_before = target - actual;

            // Calculate scaled surplus and input amount using p_ref and trading curve
            let scaled_surplus_before = shortage_before * (dec!(1) + self.k_out * shortage_before / actual) * p_ref;
            let scaled_surplus_after = (scaled_surplus_before + input_amount) / p_ref;

            // Check if the egress price curve exponent (k_out) is 1
            if self.k_out == dec!(1) {
                // Calculate the shortage before and after the operation
                let shortage_after = target * scaled_surplus_after / (target + scaled_surplus_after);

                // Calculate and return the difference in shortage
                shortage_after - shortage_before
            } else {
                // Handle other values for k_out
                let shortage_after = target + scaled_surplus_after
                    - Decimal::sqrt(
                        &(target * target
                        + (dec!(4) * self.k_out - dec!(2)) * target * scaled_surplus_after
                        + scaled_surplus_after * scaled_surplus_after)
                    ).unwrap();

                // Calculate and return the difference in shortage
                // TODO: fix shortage_before scaling
                (shortage_after - shortage_before) / dec!(2) / (dec!(1) - self.k_out)
            }
        }
    }
}