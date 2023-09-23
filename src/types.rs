use scrypto::prelude::*;

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub enum Shortage {
    BaseShortage,
    Equilibrium,
    QuoteShortage,
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct PairState {
    pub p0: Decimal,                    // Equilibrium price
    pub shortage: Shortage,             // Current state of the pair
    pub target_ratio: Decimal,          // Ratio between target and actual
    pub last_outgoing: i64,             // Timestamp of last outgoing trade
    pub last_out_spot: Decimal,         // Last outgoing spot price
}

#[derive(ScryptoSbor, Copy, Clone, PartialEq)]
pub struct PairConfig {
    pub k_in: Decimal,                  // Ingress price curve exponent
    pub k_out: Decimal,                 // Egress price curve exponent
    pub fee: Decimal,                   // Trading fee fraction
    pub decay_factor: Decimal,          // Price filter decay factor per minute
}

#[derive(ScryptoSbor, Copy, Clone)]
pub struct TradeAllocation {
    pub base_base: Decimal,             // Change in base tokens from base pool
    pub base_quote: Decimal,            // Change in quote tokens from base pool
    pub quote_base: Decimal,            // Change in base tokens from quote pool
    pub quote_quote: Decimal,           // Change in quote tokens from quote pool
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