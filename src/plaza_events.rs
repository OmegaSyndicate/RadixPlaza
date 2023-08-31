use scrypto::prelude::*;

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct AddBaseLiquidityEvent {
    pub input_amount: Decimal,
    pub lp_tokens: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct AddQuoteLiquidityEvent {
    pub input_amount: Decimal,
    pub lp_tokens: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct SwapBaseToQuoteEvent {
    pub base_in: Decimal,
    pub quote_out: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
pub struct SwapQuoteToBaseEvent {
    pub quote_in: Decimal,
    pub base_out: Decimal,
}