use crate::token::Token;
use serde::{Deserialize, Serialize};
use std::fmt;

// Order side - buy or sell, nbd
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn value(&self) -> &'static str {
        // Return string value for API calls
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        // Parse from string - case insensitive
        match s.to_uppercase().as_str() {
            "BUY" => Some(Side::Buy),
            "SELL" => Some(Side::Sell),
            _ => None, // Invalid side
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

// Order struct - represents a single order on the book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub size: f64, // Order size
    pub price: f64, // Order price
    pub side: Side, // Buy or sell
    pub token: Token, // Which token (A or B)
    pub id: Option<String>, // Order ID from CLOB (None if not placed yet)
}

impl Order {
    pub fn new(
        size: f64,
        price: f64,
        side: Side,
        token: Token,
        id: Option<String>,
    ) -> Self {
        // Create new order - simple constructor
        Self {
            size,
            price,
            side,
            token,
            id,
        }
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Order[id={:?}, price={}, size={}, side={}, token={}]",
            self.id, self.price, self.size, self.side, self.token
        )
    }
}

