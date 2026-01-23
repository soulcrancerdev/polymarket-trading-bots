### run order & one‑line descriptions

**Recommended run order:**

1. **`validate_setup.exe`**  
   - **Description**: Checks your `.env` config and environment for missing/invalid settings before you risk any money.

2. **`approve_tokens.exe`**  
   - **Description**: Sends on‑chain approvals so Polymarket contracts can spend your USDC and Conditional Tokens.

3. **`confirmed_block_bot.exe`** 
   - **Description**: Runs the copy‑trading bot that waits for block confirmation, this is more reliable.

4. **`mempool_bot.exe`**
   - **Description**: Runs the mempool‑based copy‑trading bot that watches pending transactions and mirrors whale trades fast.

5. **`trade_monitor.exe`** *(optional utility)*  
   - **Description**: Listens for your own fills and logs them to CSV so you can review what actually got filled.

6. **`test_order_types.exe`** *(optional safety test)*  
   - **Description**: Sends a tiny real FAK order to Polymarket so you can inspect the response format and confirm order flow.