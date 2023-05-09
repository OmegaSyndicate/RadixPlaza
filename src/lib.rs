use scrypto::prelude::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum PairState {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
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
    }

    impl PlazaPair {
        // Constructor to instantiate and deploy a new pair
        pub fn instantiate_pair(
            initial_base: Bucket,
            initial_quote: Bucket,
            price: Decimal,
        ) -> (ComponentAddress, Bucket, Bucket) {
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
                k_out: dec!("1.0"),
                base_lp: base_lp_bucket.resource_address(),
                quote_lp: quote_lp_bucket.resource_address(),
                lp_badge: Vault::with_bucket(lp_badge),
                fee: dec!("0.003"),
            }
            .instantiate()
            .globalize();

            (pair, base_lp_bucket, quote_lp_bucket)
        }

        // Get a quote from the AMM for trading tokens on the pair
        pub fn quote(&self, input_amount: Decimal, input_is_quote: bool) -> (Decimal, Decimal, Decimal, PairState) {
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
            let p0 = match self.state {
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

            // Set reference price to B/Q or Q/B depending on liquidity
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

            // Handle the incoming case
            if exchange_state == input_shortage {
                if amount_to_trade + input_actual >= input_target {
                    amount_to_trade -= input_target - input_actual;
                    output_amount = output_actual - output_target;

                    // Update variables for outgoing part of trade
                    exchange_state = PairState::Equilibrium;
                    output_actual = output_target;
                    p_ref = dec!(1) / p_ref;
                } else {
                    output_amount = self.calc_incoming(
                        amount_to_trade,
                        input_target,
                        input_actual,
                        p_ref,
                    );
                }
            }

            // Handle the equilibrium or input surplus case
            if exchange_state != input_shortage && amount_to_trade > dec!(0) {
                output_amount += self.calc_outgoing(
                    amount_to_trade,
                    output_target,
                    output_actual,
                    p_ref,
                );
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

            (output_amount - fee, fee, p0, exchange_state)
        }

        /// Swap a bucket of tokens along the AMM curve.
        pub fn swap(&mut self, input_tokens: Bucket) -> Bucket {
            // Determine if the input tokens are quote tokens or base tokens.
            let is_quote = input_tokens.resource_address() == self.quote_vault.resource_address();

            // Calculate the amount of output tokens and pair impact variables.
            let (output_amount, fee, new_p0, new_state) = self.quote(input_tokens.amount(), is_quote);

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
            //debug!("p0: {} -- state {}", new_p0, new_state);

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
            // Calibrate outgoing price curve to incoming at spot price.
            let shortage_before = target - actual;
            let target2 = target * target;
            let actual2 = actual * actual;
            let num = actual2 + self.k_in * (target2 - actual2);
            let den = actual2 + self.k_out * (target2 - actual2);
            let virtual_p_ref = num / den * p_ref;
            let virtual_surplus = shortage_before * (dec!(1) + self.k_out * shortage_before / actual) * virtual_p_ref;

            // Calculate scaled surplus and input amount using p_ref
            let surplus_after = (virtual_surplus + input_amount) / virtual_p_ref;

            // Check if the egress price curve exponent (k_out) is 1
            if self.k_out == dec!(1) {
                // Calculate the shortage before and after the operation
                let shortage_after = target * surplus_after / (target + surplus_after);

                // Calculate and return the difference in shortage
                shortage_after - shortage_before
            } else {
                // Handle other values for k_out
                let shortage_after = target + surplus_after
                    - Decimal::sqrt(
                        &(target * target
                        + (dec!(4) * self.k_out - dec!(2)) * target * surplus_after
                        + surplus_after * surplus_after)
                    ).unwrap();

                // Calculate and return the difference in shortage
                (shortage_after - shortage_before) / dec!(2) / (dec!(1) - self.k_out)
            }
        }
    }
}