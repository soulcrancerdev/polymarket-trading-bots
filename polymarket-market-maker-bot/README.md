# poly-market-maker-rust

Market maker keeper for the Polymarket CLOB (Rust version).

## NOTE

This software is experimental and in active development.
Use at your own risk.

## Description

The keeper is an automated market maker for CLOB markets.
Places and cancels orders to keep open orders near the midpoint price according to one of two strategies.

## Requirements

- Rust 1.70+

## Setup

- Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

- Build the project: `cargo build --release`

- Create a `.env` file or set environment variables.

- Modify the entries in `config.env`.

- Modify the corresponding strategy config in `./config`, if desired.

## Usage

Run the keeper with:

```bash
cargo run --release -- \
  --private-key <your-private-key> \
  --rpc-url <rpc-url> \
  --clob-api-url <clob-api-url> \
  --condition-id <condition-id> \
  --strategy <amm|bands> \
  --strategy-config ./config/<strategy>.json
```

### Usage with Docker

- To build: `docker build -t poly-market-maker-rust .`
- To run: `docker run poly-market-maker-rust`

## Config

The `config.env` file defines 3 environment variables:

- `CONDITION_ID`, the condition id of the market in hex string format.
- `STRATEGY`, the strategy to use, either "Bands" or "AMM" (case insensitive)
- `CONFIG`, the path to the strategy config file.

## Strategies

- **AMM**: Automated Market Maker strategy
- **Bands**: Bands-based strategy

### Strategy Lifecycle

Every `sync_interval` (the default is 30s), the strategies do the following:

1. Fetch the current midpoint price from the CLOB
2. Compute expected orders.
3. Compare expected orders to open orders.
4. Compute open orders to cancel and new orders to place to achieve or approximate the expected orders.
5. Cancel orders.
6. Place new orders.

When the app receives a SIGTERM, all orders are cancelled and the app exits gracefully.

## Differences from Python Version

This Rust version maintains the same functionality as the Python version but with:

- Better performance and memory safety
- Async/await for concurrent operations
- Strong type safety
- No GIL (Global Interpreter Lock) limitations

## License

See LICENSE.txt

