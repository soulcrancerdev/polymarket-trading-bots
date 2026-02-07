use crate::ct_helpers::CTHelpers;
use crate::token::Token;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone)]
pub struct Market {
    pub condition_id: String,
    pub token_ids: std::collections::HashMap<Token, u64>,
}

pub struct Market {
    pub condition_id: String,
    pub token_ids: std::collections::HashMap<Token, u64>,
}

impl Market {
    pub fn new(condition_id: String, collateral_address: String) -> Self {
        let mut token_ids = std::collections::HashMap::new();
        token_ids.insert(
            Token::A,
            CTHelpers::get_token_id(&condition_id, &collateral_address, 0),
        );
        token_ids.insert(
            Token::B,
            CTHelpers::get_token_id(&condition_id, &collateral_address, 1),
        );

        Self {
            condition_id,
            token_ids,
        }
    }

    pub fn token_id(&self, token: Token) -> u64 {
        *self.token_ids.get(&token).unwrap()
    }

    pub fn token(&self, token_id: u64) -> Option<Token> {
        for (token, id) in &self.token_ids {
            if *id == token_id {
                return Some(*token);
            }
        }
        None
    }
}

impl fmt::Display for Market {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Market[condition_id={}, token_id_a={}, token_id_b={}]",
            self.condition_id,
            self.token_ids.get(&Token::A).unwrap(),
            self.token_ids.get(&Token::B).unwrap()
        )
    }
}

