//! Fabricated node answers, for tests that need one without a node.
//!
//! The types a node returns are `#[non_exhaustive]`, so only this crate can build
//! them. These builders exist so that crates testing against a mock transport
//! still can — and so that a field added to one of those types is filled in once,
//! here, rather than in every test that fabricates one.

use tronz_primitives::{Address, B256, Trx, TxId};

use crate::types::{
    BlockInfo, ContractResult, DelegatedResource, ExchangeInfo, MarketOrderInfo, MarketOrderState,
    ResourceReceipt, TransactionInfo, TxStatus, WitnessInfo,
};

/// A block at `number`, with a zero hash.
pub fn block(number: i64, timestamp: i64) -> BlockInfo {
    BlockInfo::new(number, B256::ZERO, timestamp)
}

/// A receipt for a transaction that reached `status`, costing nothing.
pub fn transaction_info(status: TxStatus) -> TransactionInfo {
    TransactionInfo {
        tx_id: TxId::ZERO,
        block_number: 1,
        block_timestamp: 1,
        status,
        fee: Trx::ZERO,
        energy_usage_total: 0,
        energy_fee: Trx::ZERO,
        net_usage: 0,
        net_fee: Trx::ZERO,
        receipt: ResourceReceipt::default(),
        contract_result: ContractResult::Default,
        contract_address: None,
        logs: vec![],
        internal_transactions: vec![],
        revert_reason: None,
    }
}

/// An active witness holding `vote_count` votes.
pub fn witness(vote_count: i64) -> WitnessInfo {
    WitnessInfo {
        address: Address::ZERO,
        vote_count,
        url: "https://sr.example".to_string(),
        total_produced: 0,
        total_missed: 0,
        is_active: true,
    }
}

/// A delegation of `bandwidth` and `energy`, in sun, that never expires.
pub fn delegated_resource(bandwidth: i64, energy: i64) -> DelegatedResource {
    DelegatedResource {
        from: Address::ZERO,
        to: Address::ZERO,
        bandwidth_amount: Trx::from_sun_unchecked(bandwidth),
        energy_amount: Trx::from_sun_unchecked(energy),
        bandwidth_expire_time_ms: 0,
        energy_expire_time_ms: 0,
    }
}

/// An open order selling TRX for TRC10 token `1000001`.
pub fn market_order(owner: Address) -> MarketOrderInfo {
    MarketOrderInfo {
        order_id: B256::ZERO,
        owner_address: owner,
        create_time: 0,
        sell_token_id: "_".into(),
        sell_token_quantity: 1_000_000,
        buy_token_id: "1000001".into(),
        buy_token_quantity: 500_000,
        sell_token_quantity_remain: 1_000_000,
        sell_token_quantity_return: 0,
        state: MarketOrderState::Active,
    }
}

/// An exchange pairing TRX against TRC10 token `1000001`.
pub fn exchange(exchange_id: i64) -> ExchangeInfo {
    ExchangeInfo {
        exchange_id,
        creator_address: Address::from_evm_bytes([0u8; 20]),
        create_time: 0,
        first_token_id: "_".into(),
        first_token_balance: 1_000_000,
        second_token_id: "1000001".into(),
        second_token_balance: 500_000,
    }
}
