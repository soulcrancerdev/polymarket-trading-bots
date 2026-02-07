use crate::constants::OK;
use crate::metrics::CLOB_REQUESTS_LATENCY;
use crate::utils::randomize_default_price;
use std::time::Instant;

const DEFAULT_PRICE: f64 = 0.5;

#[derive(Clone)]
pub struct ClobApi {
    host: String,
    chain_id: u64,
    private_key: String,
    api_key: Option<String>,
}

#[derive(serde::Serialize)]
struct OrderArgs {
    price: f64,
    size: f64,
    side: String,
    token_id: u64,
}

impl ClobApi {
    pub fn new(host: String, chain_id: u64, private_key: String) -> Self {
        Self {
            host,
            chain_id,
            private_key,
            api_key: None,
        }
    }

    pub async fn get_address(&self) -> String {
        // Implementation would derive address from private key
        "".to_string()
    }

    pub async fn get_collateral_address(&self) -> String {
        // Implementation would fetch from API
        "".to_string()
    }

    pub async fn get_conditional_address(&self) -> String {
        // Implementation would fetch from API
        "".to_string()
    }

    pub async fn get_exchange(&self) -> String {
        // Implementation would fetch from API
        "".to_string()
    }

    pub async fn get_price(&self, token_id: u64) -> f64 {
        // Fetch midpoint price from CLOB - fallback to random if it fails
        let start_time = Instant::now();
        let url = format!("{}/midpoint/{}", self.host, token_id);

        match reqwest::get(&url).await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(mid) = json.get("mid").and_then(|v| v.as_f64()) {
                        let duration = start_time.elapsed().as_secs_f64();
                        CLOB_REQUESTS_LATENCY.observe(duration); // Track latency
                        return mid; // Got it!
                    }
                }
            }
            Err(e) => {
                log::error!("Error fetching current price from the CLOB API: {}", e);
                let duration = start_time.elapsed().as_secs_f64();
                CLOB_REQUESTS_LATENCY.observe(duration);
            }
        }

        self.rand_price() // Fallback if API fails
    }

    fn rand_price(&self) -> f64 {
        // Return randomized default price - better than nothing imo
        let price = randomize_default_price(DEFAULT_PRICE);
        log::info!(
            "Could not fetch price from CLOB API, returning random price: {}",
            price
        );
        price
    }

    pub async fn get_orders(&self, condition_id: &str) -> Vec<serde_json::Value> {
        let start_time = Instant::now();
        let url = format!("{}/orders?market={}", self.host, condition_id);

        match reqwest::get(&url).await {
            Ok(resp) => {
                if let Ok(orders) = resp.json::<Vec<serde_json::Value>>().await {
                    let duration = start_time.elapsed().as_secs_f64();
                    CLOB_REQUESTS_LATENCY.observe(duration);
                    return orders;
                }
            }
            Err(e) => {
                log::error!("Error fetching keeper open orders from the CLOB API: {}", e);
                let duration = start_time.elapsed().as_secs_f64();
                CLOB_REQUESTS_LATENCY.observe(duration);
            }
        }

        Vec::new()
    }

    pub async fn place_order(
        &self,
        price: f64,
        size: f64,
        side: &str,
        token_id: u64,
    ) -> Option<String> {
        // Place order on CLOB - returns order ID if successful
        log::info!(
            "Placing a new order: Order[price={},size={},side={},token_id={}]",
            price,
            size,
            side,
            token_id
        );

        let start_time = Instant::now();
        let url = format!("{}/order", self.host);
        let order_args = OrderArgs {
            price,
            size,
            side: side.to_string(),
            token_id,
        };

        let client = reqwest::Client::new();
        match client.post(&url).json(&order_args).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("success").and_then(|v| v.as_bool()) == Some(true) {
                        // Order placed successfully
                        if let Some(order_id) = json.get("orderID").and_then(|v| v.as_str()) {
                            let duration = start_time.elapsed().as_secs_f64();
                            CLOB_REQUESTS_LATENCY.observe(duration);
                            log::info!(
                                "Successfully placed new order: Order[id={},price={},size={},side={},tokenID={}]!",
                                order_id, price, size, side, token_id
                            );
                            return Some(order_id.to_string()); // Return the ID
                        }
                    }
                    if let Some(err_msg) = json.get("errorMsg").and_then(|v| v.as_str()) {
                        log::error!("Could not place new order! CLOB returned error: {}", err_msg);
                    }
                }
            }
            Err(e) => {
                log::error!("Request exception: failed placing new order: {}", e);
                let duration = start_time.elapsed().as_secs_f64();
                CLOB_REQUESTS_LATENCY.observe(duration);
            }
        }

        None // Failed to place
    }

    pub async fn cancel_order(&self, order_id: &str) -> bool {
        // Cancel single order by ID
        log::info!("Cancelling order {}...", order_id);
        if order_id.is_empty() {
            log::debug!("Invalid order_id"); // Nbd, just skip it
            return true;
        }

        let start_time = Instant::now();
        let url = format!("{}/order/{}", self.host, order_id);
        let client = reqwest::Client::new();

        match client.delete(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let duration = start_time.elapsed().as_secs_f64();
                    CLOB_REQUESTS_LATENCY.observe(duration);
                    return true; // Success!
                }
            }
            Err(e) => {
                log::error!("Error cancelling order: {}: {}", order_id, e);
                let duration = start_time.elapsed().as_secs_f64();
                CLOB_REQUESTS_LATENCY.observe(duration);
            }
        }

        false // Failed
    }

    pub async fn cancel_all_orders(&self) -> bool {
        // Cancel all our orders - used on shutdown
        log::info!("Cancelling all open keeper orders..");
        let start_time = Instant::now();
        let url = format!("{}/orders", self.host);
        let client = reqwest::Client::new();

        match client.delete(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    let duration = start_time.elapsed().as_secs_f64();
                    CLOB_REQUESTS_LATENCY.observe(duration);
                    return true; // All cancelled
                }
            }
            Err(e) => {
                log::error!("Error cancelling all orders: {}", e);
                let duration = start_time.elapsed().as_secs_f64();
                CLOB_REQUESTS_LATENCY.observe(duration);
            }
        }

        false // Failed
    }
}

