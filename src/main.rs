/// PM Whale Follower - Main entry point
/// Monitors blockchain for whale trades and executes copy trades

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use alloy::primitives::U256;
use futures::{SinkExt, StreamExt};
use rand::Rng;
use polymarket_copier::{ApiCreds, OrderArgs, RustClobClient, PreparedCreds, OrderResponse};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

mod models;

use polymarket_copier::trading::risk_guard::{RiskGuard, RiskGuardConfig, SafetyDecision, TradeSide, calc_liquidity_depth};
use polymarket_copier::config::{
    API_BASE_URL, BASE_PRICE_BUFFER, BOOK_TIMEOUT, ENABLE_PROB_SIZING, EVENT_ORDERS_FILLED_SIG,
    EXCHANGE_ADDRESSES, LOG_CSV_FILE, MIN_SHARES, MIN_TRADE_VALUE_USD, MIN_WHALE_SHARES,
    POSITION_SCALE, RESUBMIT_PRICE_STEP, WHALE_TOPIC_HEX, DEBUG_VERBOSE_ERRORS, WS_PING_TIMEOUT,
    gtd_expiry_seconds, max_resubmit_attempts, resubmit_max_buffer, should_chase_price,
    should_skip, tier_params, Config, ORDER_TIMEOUT,
};
use polymarket_copier::markets::market_cache;
use polymarket_copier::markets::tennis_markets;
use polymarket_copier::markets::soccer_markets;
use polymarket_copier::trading::orders;
use polymarket_client_sdk::clob::types::OrderType;
use polymarket_client_sdk::types::Decimal;
use std::sync::Arc;
use models::*;

const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";

// ============================================================================
// Thread-local buffers 
// ============================================================================

thread_local! {
    static CSV_BUF: RefCell<String> = RefCell::new(String::with_capacity(512));
    static SANITIZE_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TOKEN_ID_CACHE: RefCell<HashMap<[u8; 32], Arc<str>>> = RefCell::new(HashMap::with_capacity(256));
}

// ============================================================================
// Order Engine 
// ============================================================================

#[derive(Clone)]
struct OrderEngine {
    tx: mpsc::Sender<WorkItem>,
    #[allow(dead_code)]
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
    trading_enabled: bool,
}

impl OrderEngine {
    async fn submit(&self, evt: ParsedEvent, is_live: Option<bool>) -> String {
        if !self.trading_enabled {
            return "🛑 Trading OFF".into();
        }

        let (resp_tx, resp_rx) = oneshot::channel();
        if let Err(e) = self.tx.try_send(WorkItem { event: evt, respond_to: resp_tx, is_live }) {
            return format!("QUEUE_ERR: {e}");
        }

        match tokio::time::timeout(ORDER_TIMEOUT, resp_rx).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => "WORKER_DROPPED".into(),
            Err(_) => "WORKER_TIMEOUT".into(),
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    ensure_csv()?;

    // Initialize market data caches
    market_cache::init_caches();

    // Start background cache refresh task
    let _cache_refresh_handle = market_cache::spawn_cache_refresh_task();

    let cfg = Config::from_env()?;
    
    let (client, creds) = build_worker_state(
        cfg.priv_key.clone(),
        cfg.funder_addr.clone(),
        ".clob_market_cache.json",
        ".clob_creds.json",
    ).await?;
    
    let prepared_creds = PreparedCreds::from_api_creds(&creds)?;
    let risk_config = cfg.to_risk_guard_config();

    let (order_tx, order_rx) = mpsc::channel(1024);
    let (resubmit_tx, resubmit_rx) = mpsc::unbounded_channel::<ResubmitRequest>();

    let client_arc = Arc::new(client);
    let creds_arc = Arc::new(prepared_creds.clone());
    let private_key_arc = Arc::new(cfg.priv_key.clone());

    start_order_worker(order_rx, client_arc.clone(), private_key_arc.clone(), cfg.trading_enabled, cfg.mock_mode, risk_config, resubmit_tx.clone());

    tokio::spawn(resubmit_worker(resubmit_rx, client_arc, creds_arc));

    let order_engine = OrderEngine {
        tx: order_tx,
        resubmit_tx,
        trading_enabled: cfg.trading_enabled,
    };

    // ANSI color codes for gradient effect (cyan to magenta)
    const CYAN: &str = "\x1b[36m";
    const BRIGHT_CYAN: &str = "\x1b[96m";
    const MAGENTA: &str = "\x1b[35m";
    const BRIGHT_MAGENTA: &str = "\x1b[95m";
    const RESET: &str = "\x1b[0m";
    
    println!();
    // POLY - gradient from cyan to bright cyan
    println!("{}██████╗  ██████╗ ██╗  ██╗   ██╗███╗   ███╗ █████╗ ██████╗ ██╗  ██╗███████╗████████╗    {}",
        CYAN, RESET);
    println!("{}██╔══██╗██╔═══██╗██║  ╚██╗ ██╔╝████╗ ████║██╔══██╗██╔══██╗██║ ██╔╝██╔════╝╚══██╔══╝    {}",
        BRIGHT_CYAN, RESET);
    println!("{}██████╔╝██║   ██║██║   ╚████╔╝ ██╔████╔██║███████║██████╔╝█████╔╝ █████╗     ██║       {}",
        BRIGHT_CYAN, RESET);
    println!("{}██╔═══╝ ██║   ██║██║    ╚██╔╝  ██║╚██╔╝██║██╔══██║██╔══██╗██╔═██╗ ██╔══╝     ██║       {}",
        MAGENTA, RESET);
    println!("{}██║     ╚██████╔╝███████╗██║   ██║ ╚═╝ ██║██║  ██║██║  ██║██║  ██╗███████╗   ██║       {}",
        BRIGHT_MAGENTA, RESET);
    println!("{}╚═╝      ╚═════╝ ╚══════╝╚═╝   ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝   ╚═╝       {}",
        BRIGHT_MAGENTA, RESET);
    println!();
    // MARKET - gradient from magenta to bright magenta
    println!("{} ██████╗ ██████╗ ██████╗ ██╗███████╗██████╗                                             {}",
        MAGENTA, RESET);
    println!("{}██╔════╝██╔═══██╗██╔══██╗██║██╔════╝██╔══██╗                                           {}",
        BRIGHT_MAGENTA, RESET);
    println!("{}██║     ██║   ██║██████╔╝██║█████╗  ██████╔╝                                           {}",
        BRIGHT_MAGENTA, RESET);
    println!("{}██║     ██║   ██║██╔═══╝ ██║██╔══╝  ██╔══██╗                                           {}",
        BRIGHT_MAGENTA, RESET);
    println!("{}╚██████╗╚██████╔╝██║     ██║███████╗██║  ██║                                           {}",
        BRIGHT_MAGENTA, RESET);
    println!("{} ╚═════╝ ╚═════╝ ╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝                                           {}",
        BRIGHT_MAGENTA, RESET);
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Trading: {}", if cfg.trading_enabled { "ON" } else { "OFF" });
    println!("   Mock:    {}", if cfg.mock_mode { "ON" } else { "OFF" });
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("{} Trading Parameters:{}", BRIGHT_CYAN, RESET);
    println!("   Base Price Buffer:    {:.2}", *BASE_PRICE_BUFFER);
    println!("   Position Scale:       {:.2} ({:.0}%)", *POSITION_SCALE, *POSITION_SCALE * 100.0);
    println!("   Min Trade Value:      ${:.2} USD", *MIN_TRADE_VALUE_USD);
    println!("   Min Shares:           {:.2}", *MIN_SHARES);
    println!("   Prob-Based Sizing:    {}", if *ENABLE_PROB_SIZING { "ON" } else { "OFF" });
    println!("   Min Whale Shares:     {:.2}", *MIN_WHALE_SHARES);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut backoff_secs = 1u64;
    let max_backoff_secs = 60u64;
    let mut consecutive_failures = 0u32;
    Ok(())
}

// ============================================================================
// Worker Setup
// ============================================================================

async fn build_worker_state(
    private_key: String,
    funder: String,
    cache_path: &str,
    creds_path: &str,
) -> Result<(RustClobClient, ApiCreds)> {
    let cache_path = cache_path.to_string();
    let creds_path = creds_path.to_string();
    let host = API_BASE_URL.to_string();

    tokio::task::spawn_blocking(move || -> Result<(RustClobClient, ApiCreds)> {
        let mut client = RustClobClient::new(&host, 137, &private_key, &funder)?
            .with_cache_path(&cache_path);
        let _ = client.load_cache();
        
        let _ = client.prewarm_connections();

        let creds: ApiCreds = if Path::new(&creds_path).exists() {
            let data = std::fs::read_to_string(&creds_path)?;
            serde_json::from_str(&data)?
        } else {
            let derived = client.derive_api_key(0)?;
            std::fs::write(&creds_path, serde_json::to_string_pretty(&derived)?)?;
            derived
        };

        Ok((client, creds))
    }).await?
}

fn start_order_worker(
    rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    private_key: Arc<String>,
    enable_trading: bool,
    mock_trading: bool,
    risk_config: RiskGuardConfig,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    std::thread::spawn(move || {
        let mut guard = RiskGuard::new(risk_config);
        order_worker(rx, client, private_key, enable_trading, mock_trading, &mut guard, resubmit_tx);
    });
}

fn order_worker(
    mut rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    private_key: Arc<String>,
    enable_trading: bool,
    mock_trading: bool,
    guard: &mut RiskGuard,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    let mut client_mut = (*client).clone();
    while let Some(work) = rx.blocking_recv() {
        let status = process_order(&work.event.order, &mut client_mut, private_key.as_str(), enable_trading, mock_trading, guard, &resubmit_tx, work.is_live);
        let _ = work.respond_to.send(status);
    }
}

// ============================================================================
// Order Processing
// ============================================================================

fn process_order(
    info: &OrderInfo,
    client: &mut RustClobClient,
    private_key: &str,
    enable_trading: bool,
    mock_trading: bool,
    guard: &mut RiskGuard,
    resubmit_tx: &mpsc::UnboundedSender<ResubmitRequest>,
    is_live: Option<bool>,
) -> String {
    if !enable_trading { return "🛑 Trading OFF".into(); }
    if mock_trading { return "🎭 Mock Only".into(); }

    let side_is_buy = info.order_type.starts_with("BUY");
    let whale_shares = info.shares;
    let whale_price = info.price_per_share;

    // Skip small trades to avoid negative expected value after fees
    if should_skip(whale_shares) {
        return format!("⏩ Too Small (< {:.0} shares)", *MIN_WHALE_SHARES);
    }

    // Risk guard safety check
    let eval = guard.check_fast(&info.clob_token_id, whale_shares);
    match eval.decision {
        SafetyDecision::Block => return format!("RISK_BLOCKED:{}", eval.reason.as_str()),
        SafetyDecision::FetchBook => {
            let side = if side_is_buy { TradeSide::Buy } else { TradeSide::Sell };
            match fetch_book_depth_blocking(client, &info.clob_token_id, side, whale_price) {
                Ok(depth) => {
                    let final_eval = guard.check_with_book(&info.clob_token_id, eval.consecutive_large, depth);
                    if final_eval.decision == SafetyDecision::Block {
                        return format!("RISK_BLOCKED:{}", final_eval.reason.as_str());
                    }
                }
                Err(e) => {
                    guard.trip(&info.clob_token_id);
                    return format!("RISK_BOOK_FAIL:{e}");
                }
            }
        }
        SafetyDecision::Allow => {}
    }

    let (buffer, order_action, size_multiplier) = tier_params(whale_shares, side_is_buy, &info.clob_token_id);

    // Polymarket valid price range: 0.01 to 0.99 (tick size 0.01)
    let limit_price = if side_is_buy {
        (whale_price + buffer).min(0.99)
    } else {
        (whale_price - buffer).max(0.01)
    };

    let (my_shares, size_type) = calculate_safe_size(whale_shares, limit_price, size_multiplier);
    if my_shares == 0.0 {
        return format!("SKIPPED_PROBABILITY ({})", size_type);
    }
    
    // Use SDK for order placement (handles authentication automatically)
    let token_id_str = info.clob_token_id.to_string();
    // For market orders (FAK): size max 4 decimals, for limit orders: size max 2 decimals
    let size_rounded = if order_action == "FAK" {
        (my_shares * 10000.0).floor() / 10000.0  // Round to 4 decimals
    } else {
        (my_shares * 100.0).floor() / 100.0  // Round to 2 decimals
    };
    let size_decimal = match Decimal::try_from(size_rounded) {
        Ok(d) => d,
        Err(e) => return format!("EXEC_FAIL: Invalid size: {}", e),
    };
    let price_decimal = match Decimal::try_from(limit_price) {
        Ok(d) => d,
        Err(e) => return format!("EXEC_FAIL: Invalid price: {}", e),
    };
    
    let order_type_sdk = if order_action == "FAK" {
        OrderType::FOK
    } else if order_action == "GTD" {
        OrderType::GTD
    } else {
        OrderType::GTC
    };

    // Log order details before submission
    eprintln!(
        "🚀 Sending {} | {} | {:.4} shares @ ${:.4} | Total: ${:.2} | Token: {}...",
        order_action,
        if side_is_buy { "BUY" } else { "SELL" },
        size_rounded,
        limit_price,
        size_rounded * limit_price,
        &token_id_str[..20]
    );

    // Run async SDK function from blocking context
    // Create a minimal runtime if we're in a blocking thread without one
    let private_key_str = private_key.to_string(); // Clone for async move
    let token_id_str_clone = token_id_str.clone();
    let async_task = async move {
        if side_is_buy && order_action == "FAK" {
            // Market buy order using USDC amount
            // For market orders: maker (USDC) max 2 decimals, taker (shares) max 4 decimals
            // Round USDC to 2 decimals, size is already rounded to 4 decimals above
            let usdc_amount = (size_decimal * price_decimal).round_dp(2);
            
            // Polymarket requires minimum $1 USDC for market buy orders
            // Use MAX(MIN_TRADE_VALUE_USD, 1.0) to respect user config but enforce Polymarket minimum
            let min_usdc = (*MIN_TRADE_VALUE_USD).max(1.0);
            let min_usdc_decimal = Decimal::try_from(min_usdc)
                .unwrap_or_else(|_| Decimal::from(1u64)); // Fallback to $1.00 if conversion fails
            if usdc_amount < min_usdc_decimal {
                return Err(anyhow!("Market buy order amount ${} is below minimum ${} USDC (Polymarket minimum: $1.00)", usdc_amount, min_usdc));
            }
            
            eprintln!("   💸 Market BUY | ${} USDC | FOK", usdc_amount);
            orders::buy_order(&private_key_str, &token_id_str_clone, usdc_amount, Some(order_type_sdk)).await
        } else if side_is_buy {
            // Limit buy order
            eprintln!("   💸 Limit BUY | {:.4} shares @ ${:.4} | {:?}", size_decimal, price_decimal, order_type_sdk);
            orders::buy_limit_order(&private_key_str, &token_id_str_clone, size_decimal, price_decimal, Some(order_type_sdk)).await
        } else {
            // Sell order
            eprintln!("   💸 Limit SELL | {:.4} shares @ ${:.4} | {:?}", size_decimal, price_decimal, order_type_sdk);
            orders::sell_order(&private_key_str, &token_id_str_clone, size_decimal, price_decimal, Some(order_type_sdk)).await
        }
    };
    
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're in a Tokio context, use the current runtime
            handle.block_on(async_task)
        }
        Err(_) => {
            // We're in a blocking thread without a runtime, create one
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async_task)
        }
    };

    match result {
        Ok(post_order_resp) => {
            // SDK returns PostOrderResponse - extract filled amounts
            // Check if order was successful (no error message means success)
            let status = if post_order_resp.error_msg.is_none() || post_order_resp.error_msg.as_ref().unwrap().is_empty() {
                200
            } else {
                400
            };
            
            // Log order response details
            eprintln!("   ✅ Response | Status: {:?} | ID: {:?} | Taking: {:?} | Making: {:?} | Err: {:?}",
                post_order_resp.status,
                post_order_resp.order_id,
                post_order_resp.taking_amount,
                post_order_resp.making_amount,
                post_order_resp.error_msg
            );
            
            let status_str = format!("{:?}", post_order_resp.status);
            let body_text = if post_order_resp.error_msg.is_some() {
                post_order_resp.error_msg.as_ref().unwrap().clone()
            } else {
                format!("{:?}", post_order_resp)
            };

            let order_resp: Option<OrderResponse> = if status == 200 {
                // Convert SDK response to our OrderResponse format
                Some(OrderResponse {
                    success: true,
                    error_msg: post_order_resp.error_msg.clone().unwrap_or_default(),
                    order_id: post_order_resp.order_id.to_string(),
                    transactions_hashes: post_order_resp.transaction_hashes.iter()
                        .map(|h| format!("{:?}", h))
                        .collect(),
                    status: status_str,
                    taking_amount: post_order_resp.taking_amount.to_string(),
                    making_amount: post_order_resp.making_amount.to_string(),
                })
            } else {
                None
            };

            let mut underfill_msg: Option<String> = None;
            if let Some(ref resp) = order_resp {
                if side_is_buy && order_action == "FAK" {
                    let filled_shares_val: f64 = resp.taking_amount.parse().unwrap_or(0.0);
                    let requested_shares = (my_shares * 100.0).floor() / 100.0;

                    if filled_shares_val < requested_shares && filled_shares_val > 0.0 {
                        let remaining_shares = requested_shares - filled_shares_val;

                        let min_threshold = (*MIN_SHARES).max(*MIN_TRADE_VALUE_USD / limit_price);
                        if remaining_shares >= min_threshold {
                            let resubmit_buffer = resubmit_max_buffer(whale_shares);
                            let max_price = (limit_price + resubmit_buffer).min(0.99);
                            let req = ResubmitRequest {
                                token_id: info.clob_token_id.to_string(),
                                whale_price,
                                failed_price: limit_price,  // Start at same price (already filled some)
                                size: (remaining_shares * 100.0).floor() / 100.0,
                                whale_shares,
                                side_is_buy: true,
                                attempt: 1,
                                max_price,
                                cumulative_filled: filled_shares_val,
                                original_size: requested_shares,
                                is_live: is_live.unwrap_or(false),
                            };
                            let _ = resubmit_tx.send(req);
                            underfill_msg = Some(format!(
                                " | \x1b[33m⚡ Underfill: {:.2}/{:.2} filled, resubmit {:.2}\x1b[0m",
                                filled_shares_val, my_shares, remaining_shares
                            ));
                        }
                    }
                }
            }

            if status == 400 && body_text.contains("FAK") && side_is_buy {
                let resubmit_buffer = resubmit_max_buffer(whale_shares);
                let max_price = (limit_price + resubmit_buffer).min(0.99);
                let rounded_size = (my_shares * 100.0).floor() / 100.0;
                let req = ResubmitRequest {
                    token_id: info.clob_token_id.to_string(),
                    whale_price,
                    failed_price: limit_price,
                    size: rounded_size,
                    whale_shares,
                    side_is_buy: true,
                    attempt: 1,
                    max_price,
                    cumulative_filled: 0.0,
                    original_size: rounded_size,
                    is_live: is_live.unwrap_or(false),
                };
                let _ = resubmit_tx.send(req);
            }

            // Extract filled shares and actual fill price for display (reuse parsed response)
            let (filled_shares, actual_fill_price) = order_resp.as_ref()
                .and_then(|r| {
                    let taking: f64 = r.taking_amount.parse().ok()?;
                    let making: f64 = r.making_amount.parse().ok()?;
                    if taking > 0.0 { Some((taking, making / taking)) } else { None }
                })
                .unwrap_or_else(|| {
                    if status == 200 { (my_shares, limit_price) } else { (0.0, limit_price) }
                });

            // Format with color-coded fill percentage
            let pink = "\x1b[38;5;199m";
            let reset = "\x1b[0m";
            let fill_color = get_fill_color(filled_shares, my_shares);
            let whale_color = get_whale_size_color(whale_shares);
            let status_str = if status == 200 { "200 OK" } else { "FAILED" };
            let mut base = format!(
                "{} [{}] | {}{:.2}/{:.2}{} filled @ {}{:.2}{} | {}whale {:.1}{} @ {:.2}",
                status_str, size_type, fill_color, filled_shares, my_shares, reset, pink, actual_fill_price, reset, whale_color, whale_shares, reset, whale_price
            );
            if let Some(msg) = underfill_msg {
                base.push_str(&msg);
            }
            if status != 200 {
                // Provide helpful guidance for common errors
                if body_text.contains("not enough balance") || body_text.contains("allowance") {
                    base.push_str(&format!(" | \x1b[33m🚨 Low Balance/No Allowance - Fix: 1) Check USDC on Polygon 2) Approve exchange at https://polymarket.com (make a test trade)\x1b[0m"));
                } else {
                    base.push_str(&format!(" | {}", body_text));
                }
            }
            base
        }
        Err(e) => {
            let error_msg = e.to_string();
            
            // If error is already formatted with INSUFFICIENT_BALANCE/ALLOWANCE, use it as-is
            if error_msg.contains("INSUFFICIENT_BALANCE/ALLOWANCE") || 
               error_msg.contains("Insufficient balance/allowance") {
                // Error already formatted by orders.rs, use it directly
                format!("EXEC_FAIL: {}", error_msg)
            } else {
                // Check error chain for balance/allowance issues
                let mut chain_msgs: Vec<String> = Vec::new();
                for err in e.chain() {
                    let chain_msg = err.to_string();
                    chain_msgs.push(chain_msg.clone());
                    if chain_msg.contains("not enough balance") || 
                       chain_msg.contains("allowance") ||
                       chain_msg.contains("INSUFFICIENT") {
                        // Found balance/allowance error in chain
                        return format!("EXEC_FAIL: INSUFFICIENT_BALANCE/ALLOWANCE - {} | Fix: 1) Deposit USDC to your Polygon wallet 2) Approve USDC spending at https://polymarket.com (make a test trade)", chain_msg);
                    }
                }
                
                // Check main error message
                if error_msg.contains("not enough balance") || 
                   error_msg.contains("allowance") {
                    format!("EXEC_FAIL: INSUFFICIENT_BALANCE/ALLOWANCE | Fix: 1) Deposit USDC to your Polygon wallet 2) Approve USDC spending at https://polymarket.com (make a test trade) 3) Ensure wallet has approved exchange contract")
                } else {
                    format!("EXEC_FAIL: {} | chain: {}", error_msg, chain_msgs.join(" -> "))
                }
            }
        }
    }
}

fn calculate_safe_size(whale_shares: f64, price: f64, size_multiplier: f64) -> (f64, SizeType) {
    let target_scaled = whale_shares * *POSITION_SCALE * size_multiplier;
    let safe_price = price.max(0.0001);
    let required_floor = (*MIN_TRADE_VALUE_USD / safe_price).max(*MIN_SHARES);

    if target_scaled >= required_floor {
        return (target_scaled, SizeType::Scaled);
    }

    if !*ENABLE_PROB_SIZING {
        return (required_floor, SizeType::Scaled);
    }

    let probability = target_scaled / required_floor;
    let pct = (probability * 100.0) as u8;
    if rand::thread_rng().r#gen::<f64>() < probability {
        (required_floor, SizeType::ProbHit(pct))
    } else {
        (0.0, SizeType::ProbSkip(pct))
    }
}

/// Get ANSI color code based on fill percentage
fn get_fill_color(filled: f64, requested: f64) -> &'static str {
    if requested <= 0.0 { return "\x1b[31m"; }  // Red if no request
    let pct = (filled / requested) * 100.0;
    if pct < 50.0 { "\x1b[31m" }                // Red
    else if pct < 75.0 { "\x1b[38;5;208m" }     // Orange
    else if pct < 90.0 { "\x1b[33m" }           // Yellow
    else { "\x1b[32m" }                          // Green
}

/// Get ANSI color code based on whale share count (gradient from small to large)
fn get_whale_size_color(shares: f64) -> &'static str {
    if shares < 500.0 { "\x1b[90m" }              // Gray (very small)
    else if shares < 1000.0 { "\x1b[36m" }        // Cyan (small)
    else if shares < 2000.0 { "\x1b[34m" }        // Blue (medium-small)
    else if shares < 5000.0 { "\x1b[32m" }        // Green (medium)
    else if shares < 8000.0 { "\x1b[33m" }        // Yellow (medium-large)
    else if shares < 15000.0 { "\x1b[38;5;208m" } // Orange (large)
    else { "\x1b[35m" }                           // Magenta (huge)
}

fn fetch_book_depth_blocking(
    client: &RustClobClient,
    token_id: &str,
    side: TradeSide,
    threshold: f64,
) -> Result<f64, &'static str> {
    let url = format!("{}/book?token_id={}", API_BASE_URL, token_id);
    let resp = client.http_client()
        .get(&url)
        .timeout(Duration::from_millis(500))
        .send()
        .map_err(|_| "NETWORK")?;
    
    if !resp.status().is_success() { return Err("HTTP_ERROR"); }
    
    let book: Value = resp.json().map_err(|_| "PARSE")?;
    let key = if side == TradeSide::Buy { "asks" } else { "bids" };

    // Stack array instead of Vec - avoids heap allocation for max 10 items
    let mut levels: [(f64, f64); 10] = [(0.0, 0.0); 10];
    let mut count = 0;
    if let Some(arr) = book[key].as_array() {
        for lvl in arr.iter().take(10) {
            if let (Some(p), Some(s)) = (
                lvl["price"].as_str().and_then(|s| s.parse().ok()),
                lvl["size"].as_str().and_then(|s| s.parse().ok()),
            ) {
                levels[count] = (p, s);
                count += 1;
            }
        }
    }

    Ok(calc_liquidity_depth(side, &levels[..count], threshold))
}

async fn handle_event(evt: ParsedEvent, order_engine: &OrderEngine, http_client: &reqwest::Client) {
    // Check live status from cache, fallback to API lookup
    let is_live = match market_cache::get_is_live(&evt.order.clob_token_id) {
        Some(v) => Some(v),
        None => fetch_is_live(&evt.order.clob_token_id, http_client).await,
    };

    let status = order_engine.submit(evt.clone(), is_live).await;

    tokio::time::sleep(Duration::from_secs_f32(2.8)).await;

    // Fetch order book for post-trade logging
    let bests = fetch_best_book(&evt.order.clob_token_id, &evt.order.order_type, http_client).await;
    let ((bp, bs), (sp, ss)) = bests.unwrap_or_else(|| (("N/A".into(), "N/A".into()), ("N/A".into(), "N/A".into())));
    let is_live = is_live.unwrap_or(false);

    // Highlight best price in bright pink
    let pink = "\x1b[38;5;199m";
    let reset = "\x1b[0m";
    let colored_bp = format!("{}{}{}", pink, bp, reset);

    let live_display = if is_live {
        format!("\x1b[34mlive: true\x1b[0m")
    } else {
        "live: false".to_string()
    };

    // Tennis market indicator (green)
    let tennis_display = if tennis_markets::get_tennis_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[32m(TENNIS)\x1b[0m "
    } else {
        ""
    };

    // Soccer market indicator (cyan)
    let soccer_display = if soccer_markets::get_soccer_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[36m(SOCCER)\x1b[0m "
    } else {
        ""
    };

    println!(
        "💥 Block {} | {} | ${:.0} | {} | Best: {} @ {} | 2nd: {} @ {} | Live: {}",
        evt.block_number, 
        format!("{}{}{}", tennis_display, soccer_display, evt.order.order_type),
        evt.order.usd_value, 
        status, 
        colored_bp, bs, 
        sp, ss, 
        live_display
    );

    let ts: DateTime<Utc> = Utc::now();
    let row = CSV_BUF.with(|buf| {
        SANITIZE_BUF.with(|sbuf| {
            let mut b = buf.borrow_mut();
            let mut sb = sbuf.borrow_mut();
            sanitize_csv(&status, &mut sb);
            b.clear();
            let _ = write!(b,
                "{},{},{},{:.2},{:.6},{:.4},{},{},{},{},{},{},{},{}",
                ts.format("%Y-%m-%d %H:%M:%S%.3f"),
                evt.block_number, evt.order.clob_token_id, evt.order.usd_value,
                evt.order.shares, evt.order.price_per_share, evt.order.order_type,
                sb, bp, bs, sp, ss, evt.tx_hash, is_live
            );
            b.clone()
        })
    });
    let _ = tokio::task::spawn_blocking(move || append_csv_row(row)).await;
}

// ============================================================================
// Resubmitter Worker (handles FAK failures with price escalation)
// ============================================================================

async fn resubmit_worker(
    mut rx: mpsc::UnboundedReceiver<ResubmitRequest>,
    client: Arc<RustClobClient>,
    creds: Arc<PreparedCreds>,
) {
    // println!("⚡ Resubmit service started");

    while let Some(req) = rx.recv().await {
        let max_attempts = max_resubmit_attempts(req.whale_shares);
        let is_last_attempt = req.attempt >= max_attempts;

        // Calculate increment: chase only if should_chase_price returns true
        let increment = if should_chase_price(req.whale_shares, req.attempt) {
            RESUBMIT_PRICE_STEP
        } else {
            0.0  // Flat retry
        };
        let new_price = if req.side_is_buy {
            (req.failed_price + increment).min(0.99)
        } else {
            (req.failed_price - increment).max(0.01)
        };

        // Check if we've exceeded max buffer (skip check for GTD - last attempt always goes through)
        if !is_last_attempt && req.side_is_buy && new_price > req.max_price {
            let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
            println!(
                "⛔ Resubmit Aborted | Try #{} | ${:.2} > Max ${:.2} | Filled: {:.2}/{:.2} ({:.0}%)",
                req.attempt, new_price, req.max_price, req.cumulative_filled, req.original_size, fill_pct
            );
            continue;
        }

        let client_clone = Arc::clone(&client);
        let creds_clone = Arc::clone(&creds);
        let token_id = req.token_id.clone();
        let size = req.size;
        let attempt = req.attempt;
        let whale_price = req.whale_price;
        let max_price = req.max_price;
        let is_live = req.is_live;

        // Submit order: FAK for early attempts, GTD with expiry for last attempt
        let result = tokio::task::spawn_blocking(move || {
            submit_resubmit_order_sync(&client_clone, &creds_clone, &token_id, new_price, size, is_live, is_last_attempt)
        }).await;

        match result {
            Ok(Ok((true, _, filled_this_attempt))) => {
                if is_last_attempt {
                    // GTD order placed on book - we don't know fill amount yet
                    println!(
                        "\x1b[32m✅ Resubmit GTD Sent | Try #{} | ${:.2} | Size: {:.2} | Prior: {:.2}/{:.2}\x1b[0m",
                        attempt, new_price, size, req.cumulative_filled, req.original_size
                    );
                } else {
                    // FAK order - check if partial fill
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let remaining = size - filled_this_attempt;

                    // If partial fill, continue with remaining size
                    if remaining > 1.0 && filled_this_attempt > 0.0 {
                        println!(
                            "\x1b[33m⚡ Resubmit Partial | Try #{} | ${:.2} | Filled: {:.2}/{:.2} ({:.0}%) | Left: {:.2}\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct, remaining
                        );
                        let next_req = ResubmitRequest {
                            token_id: req.token_id,
                            whale_price,
                            failed_price: new_price,
                            size: remaining,
                            whale_shares: req.whale_shares,
                            side_is_buy: req.side_is_buy,
                            attempt: attempt + 1,
                            max_price,
                            cumulative_filled: total_filled,
                            original_size: req.original_size,
                            is_live: req.is_live,
                        };
                        let _ = process_resubmit_chain(&client, &creds, next_req).await;
                    } else {
                        println!(
                            "\x1b[32m🎯 Resubmit Success | Try #{} | ${:.2} | Filled: {:.2}/{:.2} ({:.0}%)\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct
                        );
                    }
                }
            }
            Ok(Ok((false, body, filled_this_attempt))) => {
                if attempt < max_attempts {
                    // Re-queue with updated price
                    let next_req = ResubmitRequest {
                        token_id: req.token_id,
                        whale_price,
                        failed_price: new_price,
                        size: req.size,
                        whale_shares: req.whale_shares,
                        side_is_buy: req.side_is_buy,
                        attempt: attempt + 1,
                        max_price,
                        cumulative_filled: req.cumulative_filled + filled_this_attempt,
                        original_size: req.original_size,
                        is_live: req.is_live,
                    };
                    let next_increment = if should_chase_price(req.whale_shares, attempt + 1) {
                        RESUBMIT_PRICE_STEP
                    } else {
                        0.0
                    };
                    println!(
                        "🔄 Retry | Try #{} Failed (FAK) | Next: ${:.2} | Max: {}",
                        attempt, new_price + next_increment, max_attempts
                    );
                    if req.whale_shares < 1000.0 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    let _ = process_resubmit_chain(
                        &client,
                        &creds,
                        next_req,
                    ).await;
                } else {
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let error_msg = if DEBUG_VERBOSE_ERRORS { body.clone() } else { body.chars().take(80).collect::<String>() };
                    println!(
                        "❌ Resubmit Failed | Try #{} | ${:.2} | Filled: {:.2}/{:.2} ({:.0}%) | Err: {}",
                        attempt, new_price, total_filled, req.original_size, fill_pct, error_msg
                    );
                }
            }
            Ok(Err(e)) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                println!(
                    "🚨 Resubmit Error | Try #{} | Filled: {:.2}/{:.2} ({:.0}%) | Err: {}",
                    attempt, req.cumulative_filled, req.original_size, fill_pct, e
                );
            }
            Err(e) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                println!(
                    "🚨 Task Error | Filled: {:.2}/{:.2} ({:.0}%) | Err: {}",
                    req.cumulative_filled, req.original_size, fill_pct, e
                );
            }
        }
    }
}

async fn process_resubmit_chain(
    client: &Arc<RustClobClient>,
    creds: &Arc<PreparedCreds>,
    mut req: ResubmitRequest,
) {
    let max_attempts = max_resubmit_attempts(req.whale_shares);

    while req.attempt <= max_attempts {
        let is_last_attempt = req.attempt >= max_attempts;

        // Calculate increment: chase only if should_chase_price returns true
        let increment = if should_chase_price(req.whale_shares, req.attempt) {
            RESUBMIT_PRICE_STEP
        } else {
            0.0  // Flat retry
        };
        let new_price = if req.side_is_buy {
            (req.failed_price + increment).min(0.99)
        } else {
            (req.failed_price - increment).max(0.01)
        };

        // Check if we've exceeded max buffer (skip check for GTD - last attempt always goes through)
        if !is_last_attempt && req.side_is_buy && new_price > req.max_price {
            let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
            println!(
                "⛔ Chain Aborted | Try #{} | ${:.2} > Max ${:.2} | Filled: {:.2}/{:.2} ({:.0}%)",
                req.attempt, new_price, req.max_price, req.cumulative_filled, req.original_size, fill_pct
            );
            return;
        }

        let client_clone = Arc::clone(&client);
        let creds_clone = Arc::clone(&creds);
        let token_id = req.token_id.clone();
        let size = req.size;
        let attempt = req.attempt;
        let is_live = req.is_live;

        // Submit order: FAK for early attempts, GTD with expiry for last attempt
        let result = tokio::task::spawn_blocking(move || {
            submit_resubmit_order_sync(&client_clone, &creds_clone, &token_id, new_price, size, is_live, is_last_attempt)
        }).await;

        match result {
            Ok(Ok((true, _, filled_this_attempt))) => {
                if is_last_attempt {
                    // GTD order placed on book - we don't know fill amount yet
                    println!(
                        "\x1b[32m✅ Chain GTD Sent | Try #{} | ${:.2} | Size: {:.2} | Prior: {:.2}/{:.2}\x1b[0m",
                        attempt, new_price, req.size, req.cumulative_filled, req.original_size
                    );
                    return;
                } else {
                    // FAK order - check if partial fill
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let remaining = req.size - filled_this_attempt;

                    // If partial fill, continue with remaining size
                    if remaining > 1.0 && filled_this_attempt > 0.0 {
                        println!(
                            "\x1b[33m⚡ Chain Partial | Try #{} | ${:.2} | Filled: {:.2}/{:.2} ({:.0}%) | Left: {:.2}\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct, remaining
                        );
                        req.cumulative_filled = total_filled;
                        req.size = remaining;
                        req.failed_price = new_price;
                        req.attempt += 1;
                        continue;
                    } else {
                        println!(
                            "\x1b[32m🎯 Chain Success | Try #{} | ${:.2} | Filled: {:.2}/{:.2} ({:.0}%)\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct
                        );
                        return;
                    }
                }
            }
            Ok(Ok((false, body, filled_this_attempt))) if body.contains("FAK") && attempt < max_attempts => {
                req.cumulative_filled += filled_this_attempt;
                req.failed_price = new_price;
                req.attempt += 1;
                // Small trades get 50ms delay to let orderbook refresh
                if req.whale_shares < 1000.0 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                continue;
            }
            Ok(Ok((false, body, filled_this_attempt))) => {
                let total_filled = req.cumulative_filled + filled_this_attempt;
                let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(total_filled, req.original_size);
                let reset = "\x1b[0m";
                let error_msg = if DEBUG_VERBOSE_ERRORS { body.clone() } else { body.chars().take(80).collect::<String>() };
                println!(
                    "❌ Chain Failed | Try #{}/{} | ${:.2} | {}Filled: {:.2}/{:.2} ({:.0}%){} | Err: {}",
                    attempt, max_attempts, new_price, fill_color, total_filled, req.original_size, fill_pct, reset, error_msg
                );
                return;
            }
            Ok(Err(e)) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🚨 Chain Error | Try #{} | {}Filled: {:.2}/{:.2} ({:.0}%){} | Err: {}",
                    attempt, fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
                return;
            }
            Err(e) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🚨 Chain Task Error | {}Filled: {:.2}/{:.2} ({:.0}%){} | Err: {}",
                    fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
                return;
            }
        }
    }
}

/// Returns (success, body_text, filled_shares)
fn submit_resubmit_order_sync(
    client: &RustClobClient,
    creds: &PreparedCreds,
    token_id: &str,
    price: f64,
    size: f64,
    is_live: bool,
    is_last_attempt: bool,
) -> anyhow::Result<(bool, String, f64)> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut client = client.clone();

    // Only use GTD with expiry on the LAST attempt; earlier attempts use FAK
    let (expiration, order_type) = if is_last_attempt {
        let expiry_secs = gtd_expiry_seconds(is_live);
        let expiry_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + expiry_secs;
        (Some(expiry_timestamp.to_string()), "GTD")
    } else {
        (None, "FAK")
    };

    // Round to micro-units (6 decimals) then back to avoid floating-point truncation issues
    // e.g., 40.80 stored as 40.7999999... would truncate to 40799999 instead of 40800000
    let price_micro = (price * 1_000_000.0).round() as i64;
    let size_micro = (size * 1_000_000.0).round() as i64;
    let rounded_price = price_micro as f64 / 1_000_000.0;
    let rounded_size = size_micro as f64 / 1_000_000.0;

    let args = OrderArgs {
        token_id: token_id.to_string(),
        price: rounded_price,
        size: rounded_size,
        side: "BUY".into(),
        fee_rate_bps: None,
        nonce: Some(0),
        expiration,
        taker: None,
        order_type: Some(order_type.to_string()),
    };

    let signed = client.create_order(args)?;
    let body = signed.post_body(&creds.api_key, order_type);
    let resp = client.post_order_fast(body, creds)?;

    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();

    // Parse filled amount from successful responses
    // GTD orders return taking_amount=0 since they're placed on book, not immediately filled
    // For GTD, return 0 - caller handles GTD success messaging separately
    let filled_shares = if status.is_success() && order_type == "FAK" {
        serde_json::from_str::<OrderResponse>(&body_text)
            .ok()
            .and_then(|r| r.taking_amount.parse::<f64>().ok())
            .unwrap_or(0.0)
    } else {
        0.0
    };

    Ok((status.is_success(), body_text, filled_shares))
}

async fn fetch_is_live(token_id: &str, client: &reqwest::Client) -> Option<bool> {
    // Fetch market info to get slug
    let market_url = format!("{}/markets?clob_token_ids={}", GAMMA_API_BASE, token_id);
    let resp = client.get(&market_url).timeout(Duration::from_secs(2)).send().await.ok()?;
    let val: Value = resp.json().await.ok()?;
    let slug = val.get(0)?.get("slug")?.as_str()?.to_string();

    // Fetch live status from events API
    let event_url = format!("{}/events/slug/{}", GAMMA_API_BASE, slug);
    let resp = client.get(&event_url).timeout(Duration::from_secs(2)).send().await.ok()?;
    let val: Value = resp.json().await.ok()?;

    Some(val["live"].as_bool().unwrap_or(false))
}

async fn fetch_best_book(token_id: &str, order_type: &str, client: &reqwest::Client) -> Option<((String, String), (String, String))> {
    let url = format!("{}/book?token_id={}", API_BASE_URL, token_id);
    let resp = client.get(&url).timeout(BOOK_TIMEOUT).send().await.ok()?;
    if !resp.status().is_success() { return None; }
    
    let val: Value = resp.json().await.ok()?;
    let key = if order_type.starts_with("BUY") { "asks" } else { "bids" };
    let entries = val.get(key)?.as_array()?;

    let is_buy = order_type.starts_with("BUY");
    
    let (best, second): (Option<(&Value, f64)>, Option<(&Value, f64)>) = 
        entries.iter().fold((None, None), |(best, second), entry| {
            let price: f64 = match entry.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
                Some(p) => p,
                None => return (best, second),
            };
            
            let better = |candidate: f64, current: f64| {
                if is_buy { candidate < current } else { candidate > current }
            };
            
            match best {
                Some((_, bp)) if better(price, bp) => (Some((entry, price)), best),
                Some((_, _bp)) => {
                    let new_second = match second {
                        Some((_, sp)) if better(price, sp) => Some((entry, price)),
                        None => Some((entry, price)),
                        _ => second,
                    };
                    (best, new_second)
                }
                None => (Some((entry, price)), second),
            }
        });

    let b = best?.0;
    let best_price = b.get("price")?.to_string();
    let best_size = b.get("size")?.to_string();
    
    let (second_price, second_size) = second
        .and_then(|(e, _)| {
            let p = e.get("price")?.to_string();
            let s = e.get("size")?.to_string();
            Some((p, s))
        })
        .unwrap_or_else(|| ("N/A".into(), "N/A".into()));
    
    Some(((best_price, best_size), (second_price, second_size)))
}

// ============================================================================
// Event Parsing
// ============================================================================

fn parse_event(message: String) -> Option<ParsedEvent> {
    let msg: WsMessage = serde_json::from_str(&message).ok()?;
    let result = msg.params?.result?;
    
    // just to double check! 
    if result.topics.len() < 3 { return None; }
    
    let has_target = result.topics.get(2)
        .map(|t| t.eq_ignore_ascii_case(WHALE_TOPIC_HEX.as_str()))
        .unwrap_or(false);
    if !has_target { return None; }

    let hex_data = &result.data;
    if hex_data.len() < 2 + 64 * 4 { return None; }

    let (maker_id, maker_bytes) = parse_u256_hex_slice_with_bytes(hex_data, 2, 66)?;
    let (taker_id, taker_bytes) = parse_u256_hex_slice_with_bytes(hex_data, 66, 130)?;

    let (clob_id, token_bytes, maker_amt, taker_amt, base_type) =
        if maker_id.is_zero() && !taker_id.is_zero() {
            let m = parse_u256_hex_slice(hex_data, 130, 194)?;
            let t = parse_u256_hex_slice(hex_data, 194, 258)?;
            (taker_id, taker_bytes, m, t, "BUY")
        } else if taker_id.is_zero() && !maker_id.is_zero() {
            let m = parse_u256_hex_slice(hex_data, 130, 194)?;
            let t = parse_u256_hex_slice(hex_data, 194, 258)?;
            (maker_id, maker_bytes, m, t, "SELL")
        } else {
            return None;
        };

    let shares = if base_type == "BUY" { u256_to_f64(&taker_amt)? } else { u256_to_f64(&maker_amt)? } / 1e6;
    if shares <= 0.0 { return None; }
    
    let usd = if base_type == "BUY" { u256_to_f64(&maker_amt)? } else { u256_to_f64(&taker_amt)? } / 1e6;
    let price = usd / shares;
    
    let mut order_type = base_type.to_string();
    if result.topics[0].eq_ignore_ascii_case(EVENT_ORDERS_FILLED_SIG) {
        order_type.push_str("_FILL");
    }

    Some(ParsedEvent {
        block_number: result.block_number.as_deref()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or_default(),
        tx_hash: result.transaction_hash.unwrap_or_default(),
        order: OrderInfo {
            order_type,
            clob_token_id: u256_to_dec_cached(&token_bytes, &clob_id),
            usd_value: usd,
            shares,
            price_per_share: price,
        },
    })
}

// ============================================================================
// Hex Parsing Helpers
// ============================================================================

#[inline]
fn parse_u256_hex_slice_with_bytes(full: &str, start: usize, end: usize) -> Option<(U256, [u8; 32])> {
    let slice = full.get(start..end)?;
    let clean = slice.strip_prefix("0x").unwrap_or(slice);
    if clean.len() > 64 { return None; }

    let mut hex_buf = [b'0'; 64];
    hex_buf[64 - clean.len()..].copy_from_slice(clean.as_bytes());

    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex_nibble(hex_buf[i * 2])?;
        let lo = hex_nibble(hex_buf[i * 2 + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Some((U256::from_be_slice(&out), out))
}

#[inline]
fn parse_u256_hex_slice(full: &str, start: usize, end: usize) -> Option<U256> {
    parse_u256_hex_slice_with_bytes(full, start, end).map(|(v, _)| v)
}

fn u256_to_dec_cached(bytes: &[u8; 32], val: &U256) -> Arc<str> {
    TOKEN_ID_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(s) = cache.get(bytes) { return Arc::clone(s); }  // Cheap Arc clone
        let s: Arc<str> = val.to_string().into();
        cache.insert(*bytes, Arc::clone(&s));
        s
    })
}

fn u256_to_f64(v: &U256) -> Option<f64> {
    if v.bit_len() <= 64 { Some(v.as_limbs()[0] as f64) }
    else { v.to_string().parse().ok() }
}

// Hex nibble lookup table - 2-3x faster than branching
const HEX_NIBBLE_LUT: [u8; 256] = {
    let mut lut = [255u8; 256];
    let mut i = b'0';
    while i <= b'9' {
        lut[i as usize] = i - b'0';
        i += 1;
    }
    let mut i = b'a';
    while i <= b'f' {
        lut[i as usize] = i - b'a' + 10;
        i += 1;
    }
    let mut i = b'A';
    while i <= b'F' {
        lut[i as usize] = i - b'A' + 10;
        i += 1;
    }
    lut
};

#[inline(always)]
fn hex_nibble(b: u8) -> Option<u8> {
    let val = HEX_NIBBLE_LUT[b as usize];
    if val == 255 { None } else { Some(val) }
}

// ============================================================================
// CSV Helpers
// ============================================================================

fn ensure_csv() -> Result<()> {
    if !Path::new(LOG_CSV_FILE).exists() {
        let mut f = File::create(LOG_CSV_FILE)?;
        writeln!(f, "timestamp,block,clob_asset_id,usd_value,shares,price_per_share,direction,order_status,best_price,best_size,second_price,second_size,tx_hash,is_live")?;
    }
    Ok(())
}

fn append_csv_row(row: String) {
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open(LOG_CSV_FILE) {
        let _ = writeln!(f, "{}", row);
    }
}

#[inline]
fn sanitize_csv(value: &str, out: &mut String) {
    out.clear();
    if !value.bytes().any(|b| b == b',' || b == b'\n' || b == b'\r') {
        out.push_str(value);
        return;
    }
    out.reserve(value.len());
    for &b in value.as_bytes() {
        out.push(match b { b',' => ';', b'\n' | b'\r' => ' ', _ => b as char });
    }
}