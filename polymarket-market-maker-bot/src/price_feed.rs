use crate::clob_api::ClobApi;
use crate::market::Market;
use crate::token::Token;
use async_trait::async_trait;

#[derive(Clone)]
pub struct PriceFeedClob {
    market: Market,
    clob_api: ClobApi,
}

#[async_trait]
pub trait PriceFeed: Send + Sync {
    async fn get_price(&self, token: Token) -> f64;
}

pub struct PriceFeedClob {
    market: Market,
    clob_api: ClobApi,
}

impl PriceFeedClob {
    pub fn new(market: Market, clob_api: ClobApi) -> Self {
        Self { market, clob_api }
    }
}

#[async_trait]
impl PriceFeed for PriceFeedClob {
    async fn get_price(&self, token: Token) -> f64 {
        let token_id = self.market.token_id(token);
        self.clob_api.get_price(token_id).await
    }
}

