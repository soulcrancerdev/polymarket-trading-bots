use crate::metrics::GAS_STATION_LATENCY;
use std::time::Instant;

#[derive(Clone)]
pub struct GasStation {
    strategy: GasStrategy,
    fixed_gas_price: u64,
    gas_station_url: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum GasStrategy {
    Fixed,
    Station,
    Web3,
}

impl GasStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "fixed" => Some(GasStrategy::Fixed),
            "station" => Some(GasStrategy::Station),
            "web3" => Some(GasStrategy::Web3),
            _ => None,
        }
    }

    pub fn value(&self) -> &'static str {
        match self {
            GasStrategy::Fixed => "fixed",
            GasStrategy::Station => "station",
            GasStrategy::Web3 => "web3",
        }
    }
}

pub struct GasStation {
    strategy: GasStrategy,
    fixed_gas_price: u64,
    gas_station_url: Option<String>,
}

impl GasStation {
    const DEFAULT_FIXED_GAS_PRICE: u64 = 100_000_000_000;

    pub fn new(
        strategy: GasStrategy,
        fixed_gas_price: Option<u64>,
        gas_station_url: Option<String>,
    ) -> Self {
        Self {
            strategy,
            fixed_gas_price: fixed_gas_price.unwrap_or(Self::DEFAULT_FIXED_GAS_PRICE),
            gas_station_url,
        }
    }

    pub async fn get_gas_price(&self) -> u64 {
        let start_time = Instant::now();
        let gas = match self.strategy {
            GasStrategy::Fixed => self.fixed_gas_price,
            GasStrategy::Station => self.get_gas_station_gas().await,
            GasStrategy::Web3 => {
                // Web3 gas price fetching would need ethers integration
                self.fixed_gas_price
            }
        };

        let duration = start_time.elapsed().as_secs_f64();
        GAS_STATION_LATENCY.observe(duration);

        gas
    }

    async fn get_gas_station_gas(&self) -> u64 {
        if let Some(ref url) = self.gas_station_url {
            if let Ok(resp) = reqwest::get(url).await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(fast) = json.get("fast").and_then(|v| v.as_u64()) {
                        return (fast as f64).ceil() as u64 * 1_000_000_000;
                    }
                }
            }
        }
        self.fixed_gas_price
    }
}

