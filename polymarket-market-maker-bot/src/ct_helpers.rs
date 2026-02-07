use sha3::{Digest, Keccak256};
use std::str::FromStr;

pub struct CTHelpers;

impl CTHelpers {
    const P: &'static str = "21888242871839275222246405745257275088696311157297823662689037894645226208583";

    pub fn get_token_id(condition_id: &str, collateral_address: &str, token_index: u32) -> u64 {
        let index_set = 1u64 << token_index;
        let collection_id = Self::get_collection_id(condition_id, index_set);
        Self::get_position_id(collateral_address, &collection_id)
    }

    pub fn get_collection_id(condition_id: &str, index_set: u64) -> String {
        let x1 = Self::get_x1(condition_id, index_set);
        let odd = (x1 >> 255) == 1;
        let p = num_bigint::BigUint::from_str(Self::P).unwrap();
        let mut a = x1 % &p;

        loop {
            a += num_bigint::BigUint::from(1u64);
            let yy = a.modpow(&num_bigint::BigUint::from(3u64), &p) + num_bigint::BigUint::from(3u64);
            let yy = yy % &p;
            let exp = (&p - num_bigint::BigUint::from(1u64)) >> 1;
            if yy.modpow(&exp, &p) == num_bigint::BigUint::from(1u64) {
                break;
            }
        }

        if odd {
            a += num_bigint::BigUint::from(1u64) << 254;
        }

        format!("{:#066x}", a)
    }

    fn get_x1(condition_id: &str, index_set: u64) -> num_bigint::BigUint {
        let condition_bytes = hex::decode(condition_id.strip_prefix("0x").unwrap_or(condition_id))
            .unwrap();
        let index_bytes = index_set.to_be_bytes();
        let mut input = Vec::new();
        input.extend_from_slice(&condition_bytes);
        input.extend_from_slice(&index_bytes);

        let hash = Keccak256::digest(&input);
        num_bigint::BigUint::from_bytes_be(&hash)
    }

    fn get_position_id(collateral_address: &str, collection_id: &str) -> u64 {
        let collateral_bytes = hex::decode(
            collateral_address.strip_prefix("0x").unwrap_or(collateral_address),
        )
        .unwrap();
        let collection_bytes = hex::decode(collection_id.strip_prefix("0x").unwrap_or(collection_id))
            .unwrap();
        let mut input = Vec::new();
        input.extend_from_slice(&collateral_bytes);
        input.extend_from_slice(&collection_bytes);

        let hash = Keccak256::digest(&input);
        let hash_bytes = hash.as_slice();
        u64::from_be_bytes([
            hash_bytes[24], hash_bytes[25], hash_bytes[26], hash_bytes[27],
            hash_bytes[28], hash_bytes[29], hash_bytes[30], hash_bytes[31],
        ])
    }
}

