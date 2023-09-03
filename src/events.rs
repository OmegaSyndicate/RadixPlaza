use scrypto::prelude::*;

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
