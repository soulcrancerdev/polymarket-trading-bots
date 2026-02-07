use serde::{Deserialize, Serialize};
use std::fmt;

// Token enum - binary market has two tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Token {
    A, // First outcome
    B, // Second outcome
}

impl Token {
    pub fn complement(self) -> Token {
        // Get the other token - A -> B, B -> A
        match self {
            Token::A => Token::B,
            Token::B => Token::A,
        }
    }

    pub fn value(&self) -> &'static str {
        // String representation for logging/debugging
        match self {
            Token::A => "TokenA",
            Token::B => "TokenB",
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value())
    }
}

pub const COLLATERAL: &str = "Collateral";

