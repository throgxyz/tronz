# tronz Examples

The examples are workspace packages that compile against the current checkout.
All network examples default to the Nile testnet. Read-only examples need no
private key; write examples require a funded Nile key in `TRON_PRIVATE_KEY`.

```bash
cargo run -p examples-queries --example query
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
```

Set `TRON_API_KEY` to use a TronGrid API key.

## Catalog

| Package | Examples |
| --- | --- |
| `examples-queries` | `query`, `address_formats`, `amount_math`, `connect_custom`, `list_witnesses`, `governance_list` |
| `examples-signers` | `signer_generate`, `signer_local`, `signer_mnemonic`, `signer_keystore` |
| `examples-transfers` | `transfer_trx`, `transfer_trx_memo` |
| `examples-staking` | `stake`, `stake_v1`, `stake_bandwidth`, `delegate`, `undelegate`, `unfreeze`, `cancel_unfreeze`, `withdraw_unfreeze`, `claim_rewards`, `vote_witness` |
| `examples-contracts` | `contract_call`, `contract_send`, `contract_deploy`, `contract_dynamic_abi`, `contract_estimate_energy`, `contract_revert`, `decode_log`, `decode_receipt` |
| `examples-trc10` | `trc10_query`, `trc10_by_name`, `trc10_balance`, `trc10_transfer`, `trc10_issue` |
| `examples-trc20` | `trc20`, `trc20_approve`, `trc20_transfer_from`, `trc20_decode_transfer_event` |
| `examples-accounts` | `account_create`, `account_update`, `account_permissions` |

## Useful Commands

```bash
# Read-only
cargo run -p examples-queries --example query
cargo run -p examples-queries --example list_witnesses
cargo run -p examples-queries --example governance_list
cargo run -p examples-trc10 --example trc10_query

# Write paths on Nile
TRON_PRIVATE_KEY=<hex> cargo run -p examples-transfers --example transfer_trx
TRON_PRIVATE_KEY=<hex> cargo run -p examples-staking --example stake
TRON_PRIVATE_KEY=<hex> cargo run -p examples-trc10 --example trc10_issue
TRON_PRIVATE_KEY=<hex> cargo run -p examples-contracts --example contract_deploy
```

## Environment Variables

| Variable | Description |
| --- | --- |
| `TRON_PRIVATE_KEY` | Funded Nile private key for write examples. |
| `TRON_API_KEY` | Optional TronGrid API key. |
| `TRON_ADDRESS` | Address to query in read examples. |
| `TRON_TO` | Recipient or target address in write examples. |
| `TRON_CONTRACT` | TRC20 or smart contract address. |
| `TRON_TX_ID` | Transaction id used by receipt/log decoding examples. |
| `TRON_AMOUNT_SUN` | TRX amount in sun for TRX transfer examples. |
| `TRON_FREEZE_SUN` | Amount to stake in staking examples. |
| `TRON_TOKEN_ID` | Numeric TRC10 token id. |
