use crate::gas::GasStation;
use crate::metrics::CHAIN_REQUESTS_COUNTER;
use ethers::prelude::*;
use std::str::FromStr;

#[derive(Clone)]
pub struct Contracts {
    provider: Provider<Http>,
    gas_station: GasStation,
    address: Address,
}

const DECIMALS: u64 = 1_000_000;

pub struct Contracts {
    provider: Provider<Http>,
    gas_station: GasStation,
    address: Address,
}

impl Contracts {
    pub fn new(provider: Provider<Http>, gas_station: GasStation, address: Address) -> Self {
        Self {
            provider,
            gas_station,
            address,
        }
    }

    pub async fn token_balance_of(
        &self,
        token: Address,
        address: Address,
        token_id: Option<u64>,
    ) -> f64 {
        if token_id.is_none() {
            self.balance_of_erc20(token, address).await
        } else {
            self.balance_of_erc1155(token, address, token_id.unwrap()).await
        }
    }

    async fn balance_of_erc20(&self, token: Address, address: Address) -> f64 {
        // ERC20 balanceOf implementation would go here
        // For now, return 0.0
        0.0
    }

    async fn balance_of_erc1155(
        &self,
        _token: Address,
        _address: Address,
        _token_id: u64,
    ) -> f64 {
        // ERC1155 balanceOf implementation would go here
        // For now, return 0.0
        0.0
    }

    pub async fn gas_balance(&self, address: Address) -> f64 {
        match self.provider.get_balance(address, None).await {
            Ok(balance) => {
                CHAIN_REQUESTS_COUNTER.inc();
                balance.as_u128() as f64 / 1e18
            }
            Err(e) => {
                CHAIN_REQUESTS_COUNTER.inc();
                log::error!("Error get_balance: {}", e);
                0.0
            }
        }
    }

    pub async fn max_approve_erc20(
        &self,
        _token: Address,
        _owner: Address,
        _spender: Address,
    ) -> Option<H256> {
        // ERC20 approve implementation would go here
        None
    }

    pub async fn max_approve_erc1155(
        &self,
        _token: Address,
        _owner: Address,
        _spender: Address,
    ) -> Option<H256> {
        // ERC1155 setApprovalForAll implementation would go here
        None
    }
}

