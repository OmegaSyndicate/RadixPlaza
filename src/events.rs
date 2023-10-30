use scrypto::prelude::*;
use crate::pair::plazapair::PlazaPair;
use crate::types::PairConfig;

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct AddLiquidityEvent {
    pub is_quote: bool,
    pub token_amount: Decimal,
    pub lp_amount: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct RemoveLiquidityEvent {
    pub is_quote: bool,
    pub main_amount: Decimal,
    pub other_amount: Decimal,
    pub lp_amount: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct SwapEvent {
    pub base_amount: Decimal,
    pub quote_amount: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct PairCreated {
    pub base_token: ResourceAddress,
    pub config: PairConfig,
    pub p0: Decimal,
    pub component: Global<PlazaPair>,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct TokenDeListed {
    pub base_token: ResourceAddress,
    pub component: Global<PlazaPair>,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct TokenBlacklisted {
    pub token: ResourceAddress,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct TokenDeBlacklisted {
    pub token: ResourceAddress,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct PairRelisted {
    pub token: ResourceAddress,
    pub pair: Global<PlazaPair>,
}