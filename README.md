# TOKEN2022 - Pumpfun Smart Contract Overview

This document outlines the intricacies of the Pumpfun smart contract that utilizes the new SPL token standard, TOKEN2022. This advanced token protocol extends the functionalities of conventional SPL tokens, providing developers with more versatile options for implementing tokenomics in decentralized applications.

## Why Choose TOKEN2022

TOKEN2022 represents a significant evolution of the SPL token program, offering various additional functionalities beyond what traditional SPL tokens can provide. These enhancements may include features such as tax handling, programmatic governance, or custom fee structures, which can facilitate more sophisticated financial operations within decentralized ecosystems.


## Development Environment

### Devnet Program Addresses

**Pumpfun + Raydium CLMM**
```
Fu6WXgEQeVBrsvHbwh8vStwLxjA12E9KYjPzXnJ1sQC7
```

**Pumpfun + Raydium CPMM**
```
GY4gideNhYWJLkgxDW7q9hS6U2SrKb9AmSUbJPsWhEKB
```

## Operational Procedures

### 1. Creating a Token Pool in Pumpfun

A pool can be initialized in Pumpfun by minting new TOKEN2022 tokens. This process is critical for creating a liquidity pool that can facilitate token swapping.

- **Pool Creation Transaction:**
  [View Transaction](https://solana.fm/tx/5QYCTaGHaareH5CoCMDeDCSxq785BfdMhKmbeKWizq7uAeVptkAuyY8N1QSc78N8YPKLi3fXTZxAfPMdzy76jT25?cluster=devnet-solana)

### 2. Purchasing TOKEN2022

Users can purchase TOKEN2022 through the Pumpfun platform, which incorporates transaction fees for tax and platform swap operations.

- **Purchase Transaction:**
  [View Transaction](https://solana.fm/tx/5unyZ9MekJeE8EULD4x9JKiNNCShfMnpk5edJzLpEMB6AY9g449an1y5hWmHkkJ8hwGCfpaVnb6TWL3SeqH14EYx?cluster=devnet-solana)

### 3. Selling TOKEN2022

The Pumpfun platform allows users to sell TOKEN2022. Similar to the buying process, selling involves associated transaction fees.

- **Sale Transaction:**
  [View Transaction](https://solana.fm/tx/2Wt2YhkU5Bj6kY9hgSLaPZ6AkjxsRZrijax59f9kRQo9fD61SkjhXPd587RTt9SDDQ4cdYNMySMBKZ5L5TJqYmyp?cluster=devnet-solana)

### 4. Liquidity Migration to Raydium CLMM 

After operating on Pumpfun, liquidity can be removed to migrate to the Raydium constant product market maker (CPMM) or constant product automated market maker (CLMM).

- **Liquidity Removal Transaction:**
  [View Transaction](https://solana.fm/tx/uX492XUVW7yEtxyxSyhqDm7jngB7xtr23Sh29WhVfHR88JuSDwyC387XDE69k4Q8dzPbfYGDeX2hMHsRMQg2LLH?cluster=devnet-solana)

### 5. Liquidity Migration to Raydium CPMM

After liquidity removal from Pumpfun, it can be migrated to Raydium CPMM through a dedicated migration operation.

- **Migration Transaction:**
  [View Transaction](https://solana.fm/tx/5iHdBwV2d9RsqmawRuUSRiJfb5k22ooZTpCJhigBiXpYrbep7pK4rYKyq2MQgtiSYYTzsDB1wKtrmtx45K93D7p5?cluster=devnet-solana)

## Conclusion

By leveraging TOKEN2022 and the Pumpfun smart contract, developers can build robust decentralized applications with enhanced token functionalities. The outlined procedures demonstrate how to effectively engage with the Pumpfun platform while utilizing the Raydium ecosystem for improved liquidity management.

## Troubleshooting

It is not representing the basic code so if you have any issue during the run of this repo, feel free to open issues.
Or contact here: [telegram](https://t.me/soulcrancerdev)
