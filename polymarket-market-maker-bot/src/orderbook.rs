use crate::order::Order;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct OrderBookManager {
    refresh_frequency: u64,
    get_orders_function: Option<Arc<dyn Fn() -> Vec<Order> + Send + Sync>>,
    get_balances_function: Option<Arc<dyn Fn() -> HashMap<String, f64> + Send + Sync>>,
    place_order_function: Option<Arc<dyn Fn(Order) -> Option<Order> + Send + Sync>>,
    cancel_order_function: Option<Arc<dyn Fn(&Order) -> bool + Send + Sync>>,
    cancel_all_orders_function: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    state: Arc<RwLock<Option<OrderBookState>>>,
    refresh_count: Arc<Mutex<u64>>,
    currently_placing_orders: Arc<Mutex<u64>>,
    orders_placed: Arc<Mutex<Vec<Order>>>,
    order_ids_cancelling: Arc<Mutex<std::collections::HashSet<String>>>,
    order_ids_cancelled: Arc<Mutex<std::collections::HashSet<String>>>,
}

// Order book snapshot - current state of orders & balances
pub struct OrderBook {
    pub orders: Vec<Order>,
    pub balances: HashMap<String, f64>,
    pub orders_being_placed: bool,
    pub orders_being_cancelled: bool,
}

// Manages orderbook state w/ background refresh - keeps it fresh w/o constant polling
pub struct OrderBookManager {
    refresh_frequency: u64,
    get_orders_function: Option<Arc<dyn Fn() -> Vec<Order> + Send + Sync>>,
    get_balances_function: Option<Arc<dyn Fn() -> HashMap<String, f64> + Send + Sync>>,
    place_order_function: Option<Arc<dyn Fn(Order) -> Option<Order> + Send + Sync>>,
    cancel_order_function: Option<Arc<dyn Fn(&Order) -> bool + Send + Sync>>,
    cancel_all_orders_function: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    state: Arc<RwLock<Option<OrderBookState>>>,
    refresh_count: Arc<Mutex<u64>>,
    currently_placing_orders: Arc<Mutex<u64>>,
    orders_placed: Arc<Mutex<Vec<Order>>>,
    order_ids_cancelling: Arc<Mutex<std::collections::HashSet<String>>>,
    order_ids_cancelled: Arc<Mutex<std::collections::HashSet<String>>>,
}

struct OrderBookState {
    orders: Vec<Order>,
    balances: HashMap<String, f64>,
}

impl OrderBookManager {
    pub fn new(refresh_frequency: u64) -> Self {
        Self {
            refresh_frequency,
            get_orders_function: None,
            get_balances_function: None,
            place_order_function: None,
            cancel_order_function: None,
            cancel_all_orders_function: None,
            state: Arc::new(RwLock::new(None)),
            refresh_count: Arc::new(Mutex::new(0)),
            currently_placing_orders: Arc::new(Mutex::new(0)),
            orders_placed: Arc::new(Mutex::new(Vec::new())),
            order_ids_cancelling: Arc::new(Mutex::new(std::collections::HashSet::new())),
            order_ids_cancelled: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    pub fn get_orders_with<F>(&mut self, f: F)
    where
        F: Fn() -> Vec<Order> + Send + Sync + 'static,
    {
        self.get_orders_function = Some(Arc::new(f));
    }

    pub fn get_balances_with<F>(&mut self, f: F)
    where
        F: Fn() -> HashMap<String, f64> + Send + Sync + 'static,
    {
        self.get_balances_function = Some(Arc::new(f));
    }

    pub fn place_orders_with<F>(&mut self, f: F)
    where
        F: Fn(Order) -> Option<Order> + Send + Sync + 'static,
    {
        self.place_order_function = Some(Arc::new(f));
    }

    pub fn cancel_orders_with<F>(&mut self, f: F)
    where
        F: Fn(&Order) -> bool + Send + Sync + 'static,
    {
        self.cancel_order_function = Some(Arc::new(f));
    }

    pub fn cancel_all_orders_with<F>(&mut self, f: F)
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.cancel_all_orders_function = Some(Arc::new(f));
    }

    pub fn start(&self) {
        // Start background refresh loop - keeps orderbook fresh
        let state = Arc::clone(&self.state);
        let get_orders = Arc::clone(self.get_orders_function.as_ref().unwrap());
        let get_balances = self.get_balances_function.as_ref().map(Arc::clone);
        let refresh_frequency = self.refresh_frequency;
        let refresh_count = Arc::clone(&self.refresh_count);
        let orders_placed = Arc::clone(&self.orders_placed);
        let order_ids_cancelled = Arc::clone(&self.order_ids_cancelled);
        let order_ids_cancelling = Arc::clone(&self.order_ids_cancelling);

        tokio::spawn(async move {
            loop {
                // Fetch fresh data from API
                let orders = get_orders();
                let balances = get_balances.as_ref().map(|f| f());

                let mut state_guard = state.write().await;
                *state_guard = Some(OrderBookState {
                    orders: orders.clone(),
                    balances: balances.unwrap_or_default(),
                }); // Update state

                {
                    let mut count = refresh_count.lock().unwrap();
                    *count += 1; // Track refresh count
                }

                drop(state_guard);
                sleep(Duration::from_secs(refresh_frequency)).await; // Wait before next refresh
            }
        });
    }

    pub async fn get_order_book(&self) -> OrderBook {
        // Get current orderbook snapshot - wait if not ready yet
        loop {
            let state_guard = self.state.read().await;
            if state_guard.is_some() {
                break; // Got it!
            }
            drop(state_guard);
            sleep(Duration::from_millis(500)).await; // Wait a bit
        }

        let state_guard = self.state.read().await;
        let state = state_guard.as_ref().unwrap();
        let orders_placed = self.orders_placed.lock().unwrap();
        let order_ids_cancelling = self.order_ids_cancelling.lock().unwrap();
        let order_ids_cancelled = self.order_ids_cancelled.lock().unwrap();

        let mut orders = state.orders.clone();
        // Add orders we just placed (not in API response yet)
        for order in orders_placed.iter() {
            if !orders.iter().any(|o| o.id == order.id) {
                orders.push(order.clone());
            }
        }

        // Remove orders we're cancelling or already cancelled
        orders.retain(|order| {
            order.id.as_ref().map_or(true, |id| {
                !order_ids_cancelling.contains(id) && !order_ids_cancelled.contains(id)
            })
        });

        let orders_being_placed = *self.currently_placing_orders.lock().unwrap() > 0;
        let orders_being_cancelled = !order_ids_cancelling.is_empty();

        OrderBook {
            orders,
            balances: state.balances.clone(),
            orders_being_placed,
            orders_being_cancelled,
        }
    }

    pub async fn place_orders(&self, orders: Vec<Order>) {
        // Place multiple orders concurrently - each in its own task
        if orders.is_empty() {
            return; // Nbd, nothing to do
        }

        {
            let mut count = self.currently_placing_orders.lock().unwrap();
            *count += orders.len() as u64; // Track how many we're placing
        }

        let place_fn = Arc::clone(self.place_order_function.as_ref().unwrap());
        let orders_placed = Arc::clone(&self.orders_placed);
        let currently_placing = Arc::clone(&self.currently_placing_orders);

        for order in orders {
            let place_fn = Arc::clone(&place_fn);
            let orders_placed = Arc::clone(&orders_placed);
            let currently_placing = Arc::clone(&currently_placing);

            tokio::spawn(async move {
                // Place each order async
                if let Some(new_order) = place_fn(order) {
                    orders_placed.lock().unwrap().push(new_order); // Track successful placements
                }
                let mut count = currently_placing.lock().unwrap();
                *count -= 1; // Decrement counter when done
            });
        }
    }

    pub async fn cancel_orders(&self, orders: Vec<Order>) {
        if orders.is_empty() {
            return;
        }

        let cancel_fn = Arc::clone(self.cancel_order_function.as_ref().unwrap());
        let order_ids_cancelling = Arc::clone(&self.order_ids_cancelling);
        let order_ids_cancelled = Arc::clone(&self.order_ids_cancelled);

        for order in orders {
            if let Some(ref id) = order.id {
                order_ids_cancelling.lock().unwrap().insert(id.clone());
            }

            let cancel_fn = Arc::clone(&cancel_fn);
            let order_ids_cancelling = Arc::clone(&order_ids_cancelling);
            let order_ids_cancelled = Arc::clone(&order_ids_cancelled);
            let order_id = order.id.clone();

            tokio::spawn(async move {
                if cancel_fn(&order) {
                    if let Some(ref id) = order_id {
                        order_ids_cancelled.lock().unwrap().insert(id.clone());
                        order_ids_cancelling.lock().unwrap().remove(id);
                    }
                } else {
                    if let Some(ref id) = order_id {
                        order_ids_cancelling.lock().unwrap().remove(id);
                    }
                }
            });
        }
    }

    pub async fn cancel_all_orders(&self) {
        // Cancel everything - used on shutdown, keeps trying until all gone
        loop {
            let orderbook = self.get_order_book().await;
            if orderbook.orders.is_empty() {
                break; // All done!
            }

            let order_ids: Vec<String> = orderbook
                .orders
                .iter()
                .filter_map(|order| order.id.clone())
                .collect(); // Get all order IDs

            {
                let mut cancelling = self.order_ids_cancelling.lock().unwrap();
                for id in &order_ids {
                    cancelling.insert(id.clone()); // Mark as cancelling
                }
            }

            if let Some(ref cancel_all_fn) = self.cancel_all_orders_function {
                if cancel_all_fn() {
                    // Successfully cancelled
                    let mut cancelled = self.order_ids_cancelled.lock().unwrap();
                    let mut cancelling = self.order_ids_cancelling.lock().unwrap();
                    for id in &order_ids {
                        cancelled.insert(id.clone()); // Move to cancelled
                        cancelling.remove(id);
                    }
                }
            }

            self.wait_for_stable_order_book().await; // Wait for things to settle
            sleep(Duration::from_secs(2)).await; // Give it a sec before checking again
        }
    }

    pub async fn wait_for_stable_order_book(&self) {
        loop {
            let orderbook = self.get_order_book().await;
            if !orderbook.orders_being_cancelled && !orderbook.orders_being_placed {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }
}

