//! TRX transfer builder.

use tronz_primitives::{Address, Bytes, Trx};

use super::{builder_exits, resolve_owner};
use crate::{
    error::{Error, Result},
    provider::{PendingTransaction, TronProvider},
    types::{ContractType, TransactionRequest, TransferContract},
};

/// Builds a TRX transfer (`send_trx`).
#[derive(Debug)]
pub struct TransferBuilder<'a, P> {
    provider: &'a P,
    owner: Option<Address>,
    to: Option<Address>,
    amount: Option<Trx>,
    memo: Option<Bytes>,
    permission_id: Option<i32>,
}

impl<'a, P: TronProvider> TransferBuilder<'a, P> {
    /// Start a new transfer builder.
    pub fn new(provider: &'a P) -> Self {
        Self { provider, owner: None, to: None, amount: None, memo: None, permission_id: None }
    }

    /// Override the owner address (defaults to the provider's signer address).
    pub fn from(mut self, from: Address) -> Self {
        self.owner = Some(from);
        self
    }

    /// Set the recipient.
    pub fn to(mut self, to: Address) -> Self {
        self.to = Some(to);
        self
    }

    /// Set the amount.
    pub fn amount(mut self, amount: Trx) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Attach a memo.
    pub fn memo(mut self, memo: impl Into<Bytes>) -> Self {
        self.memo = Some(memo.into());
        self
    }

    /// The request this builder describes, without contacting the node.
    pub fn into_request(self) -> Result<TransactionRequest> {
        let owner = resolve_owner(self.owner, self.provider)?;
        let to = self.to.ok_or(Error::missing_field("to"))?;
        let amount = self.amount.ok_or(Error::missing_field("amount"))?;

        Ok(TransactionRequest {
            contract: Some(ContractType::Transfer(TransferContract {
                owner_address: owner,
                to_address: to,
                amount,
            })),
            memo: self.memo,
            permission_id: self.permission_id,
            ..Default::default()
        })
    }

    builder_exits!();
}

#[cfg(test)]
mod tests {
    use tronz_primitives::Address;

    use super::*;
    use crate::{provider::RootProvider, transport::mock::MockTransport};

    fn addr(b: u8) -> Address {
        Address::from_evm_bytes({
            let mut a = [0u8; 20];
            a[19] = b;
            a
        })
    }

    fn mock_provider() -> RootProvider {
        RootProvider::new(MockTransport::new())
    }

    #[tokio::test]
    async fn missing_to_returns_error() {
        let provider = mock_provider();
        let err = provider
            .send_trx()
            .from(addr(1))
            .amount(Trx::from_sun(1_000_000).unwrap())
            .send()
            .await
            .err()
            .unwrap();
        assert!(err.is_local_usage_error());
    }

    #[tokio::test]
    async fn missing_amount_returns_error() {
        let provider = mock_provider();
        let err = provider.send_trx().from(addr(1)).to(addr(2)).send().await.err().unwrap();
        assert!(err.is_local_usage_error());
    }
    #[tokio::test]
    async fn build_carries_the_permission_id_and_leaves_the_transaction_unsigned() {
        use prost::Message as _;

        use crate::types::RawTransaction;

        let node_tx = crate::proto::Transaction {
            raw_data: Some(crate::proto::transaction::Raw {
                contract: vec![crate::proto::transaction::Contract::default()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let transport = MockTransport::new();
        transport.push_ok(
            "transfer_trx",
            RawTransaction::from_node_encoded(node_tx.encode_to_vec(), &[]).unwrap(),
        );

        let raw = RootProvider::new(transport)
            .send_trx()
            .from(addr(1))
            .to(addr(2))
            .amount(Trx::from_sun(1_000_000).unwrap())
            .permission_id(2)
            .build()
            .await
            .unwrap();

        let decoded = crate::proto::Transaction::decode(raw.encoded()).unwrap();
        assert_eq!(decoded.raw_data.unwrap().contract[0].permission_id, 2);
        assert!(decoded.signature.is_empty());
    }
}
