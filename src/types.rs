use scrypto::prelude::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum Shortage {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct PairState {
    pub p0: Decimal,                // Equilibrium price
    pub base_target: Decimal,       // Target amount of base tokens
    pub quote_target: Decimal,      // Target amount of quote tokens
    pub shortage: Shortage,         // Current state of the pair
    pub last_trade: i64,            // Timestamp of last trade
    pub last_outgoing: i64,         // Timestamp of last outgoing trade
    pub last_spot: Decimal,         // Last outgoing spot price
}

impl PairState {
    pub fn set_output_target(&mut self, output_target: Decimal, input_is_quote: bool) {
        if input_is_quote {
            self.base_target = output_target;
        } else {
            self.quote_target = output_target;
        }
    }

    pub fn set_input_target(&mut self, input_target: Decimal, input_is_quote: bool) {
        if input_is_quote {
            self.quote_target = input_target;
        } else {
            self.base_target = input_target;
        }
    }
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct PairConfig {
    pub k_in: Decimal,              // Ingress price curve exponent
    pub k_out: Decimal,             // Egress price curve exponent
    pub fee: Decimal,               // Trading fee
}

impl fmt::Display for Shortage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Shortage::BaseShortage => write!(f, "BaseShortage"),
            Shortage::Equilibrium => write!(f, "Equilibrium"),
            Shortage::QuoteShortage => write!(f, "QuoteShortage"),
        }
    }
}