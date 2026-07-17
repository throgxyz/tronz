//! In-process `protocol.WalletSolidity` server for transport tests.
//!
//! Responses are queued per method, and calls record their method and API key.

use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status, transport::Server};

/// Generated protobuf types.
pub mod pb {
    tonic::include_proto!("protocol");
}

use pb::{
    Account, BlockExtention, BytesMessage, EmptyMessage, EstimateEnergyMessage, NumberMessage,
    Transaction, TransactionExtention, TransactionInfo, TransactionInfoList,
    wallet_solidity_server::{WalletSolidity, WalletSolidityServer},
};

type Queue<T> = VecDeque<Result<T, Status>>;

#[derive(Default)]
struct State {
    account: Queue<Account>,
    now_block: Queue<BlockExtention>,
    block_by_num: Queue<BlockExtention>,
    tx_by_id: Queue<Transaction>,
    tx_info: Queue<TransactionInfo>,
    tx_info_by_block: Queue<TransactionInfoList>,
    tx_count: Queue<NumberMessage>,
    constant: Queue<TransactionExtention>,
    estimate: Queue<EstimateEnergyMessage>,
    seen_api_keys: Vec<Option<String>>,
    seen_methods: Vec<&'static str>,
}

/// Controls responses and inspects recorded calls.
#[derive(Clone)]
pub struct Handle {
    state: Arc<Mutex<State>>,
}

macro_rules! pushers {
    ($($method:ident => $field:ident : $ty:ty),* $(,)?) => {
        impl Handle {
            $(
                #[doc = concat!("Queue the next `", stringify!($field), "` response.")]
                pub fn $method(&self, response: Result<$ty, Status>) -> &Self {
                    self.state.lock().unwrap().$field.push_back(response);
                    self
                }
            )*
        }
    };
}

pushers! {
    push_account => account: Account,
    push_now_block => now_block: BlockExtention,
    push_block_by_num => block_by_num: BlockExtention,
    push_transaction => tx_by_id: Transaction,
    push_transaction_info => tx_info: TransactionInfo,
    push_transaction_info_by_block => tx_info_by_block: TransactionInfoList,
    push_transaction_count => tx_count: NumberMessage,
    push_constant => constant: TransactionExtention,
    push_estimate => estimate: EstimateEnergyMessage,
}

impl Handle {
    /// The `TRON-PRO-API-KEY` header seen on each call, in order.
    pub fn seen_api_keys(&self) -> Vec<Option<String>> {
        self.state.lock().unwrap().seen_api_keys.clone()
    }

    /// The RPC method name seen on each call, in order.
    pub fn seen_methods(&self) -> Vec<&'static str> {
        self.state.lock().unwrap().seen_methods.clone()
    }
}

struct TestWalletSolidity {
    state: Arc<Mutex<State>>,
}

impl TestWalletSolidity {
    fn record<T>(&self, request: &Request<T>, method: &'static str) {
        let key = request
            .metadata()
            .get("tron-pro-api-key")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut st = self.state.lock().unwrap();
        st.seen_api_keys.push(key);
        st.seen_methods.push(method);
    }

    fn take<T>(
        &self,
        pick: impl FnOnce(&mut State) -> &mut Queue<T>,
    ) -> Result<Response<T>, Status> {
        let mut st = self.state.lock().unwrap();
        match pick(&mut st).pop_front() {
            Some(Ok(value)) => Ok(Response::new(value)),
            Some(Err(status)) => Err(status),
            None => Err(Status::unavailable("no queued response")),
        }
    }
}

// Keep these methods explicit: `#[async_trait]` cannot expand methods generated
// by an inner `macro_rules!`.
#[tonic::async_trait]
impl WalletSolidity for TestWalletSolidity {
    async fn get_account(&self, request: Request<Account>) -> Result<Response<Account>, Status> {
        self.record(&request, "GetAccount");
        self.take(|s| &mut s.account)
    }

    async fn get_now_block2(
        &self,
        request: Request<EmptyMessage>,
    ) -> Result<Response<BlockExtention>, Status> {
        self.record(&request, "GetNowBlock2");
        self.take(|s| &mut s.now_block)
    }

    async fn get_block_by_num2(
        &self,
        request: Request<NumberMessage>,
    ) -> Result<Response<BlockExtention>, Status> {
        self.record(&request, "GetBlockByNum2");
        self.take(|s| &mut s.block_by_num)
    }

    async fn get_transaction_by_id(
        &self,
        request: Request<BytesMessage>,
    ) -> Result<Response<Transaction>, Status> {
        self.record(&request, "GetTransactionById");
        self.take(|s| &mut s.tx_by_id)
    }

    async fn get_transaction_info_by_id(
        &self,
        request: Request<BytesMessage>,
    ) -> Result<Response<TransactionInfo>, Status> {
        self.record(&request, "GetTransactionInfoById");
        self.take(|s| &mut s.tx_info)
    }

    async fn get_transaction_info_by_block_num(
        &self,
        request: Request<NumberMessage>,
    ) -> Result<Response<TransactionInfoList>, Status> {
        self.record(&request, "GetTransactionInfoByBlockNum");
        self.take(|s| &mut s.tx_info_by_block)
    }

    async fn get_transaction_count_by_block_num(
        &self,
        request: Request<NumberMessage>,
    ) -> Result<Response<NumberMessage>, Status> {
        self.record(&request, "GetTransactionCountByBlockNum");
        self.take(|s| &mut s.tx_count)
    }

    async fn trigger_constant_contract(
        &self,
        request: Request<pb::TriggerSmartContract>,
    ) -> Result<Response<TransactionExtention>, Status> {
        self.record(&request, "TriggerConstantContract");
        self.take(|s| &mut s.constant)
    }

    async fn estimate_energy(
        &self,
        request: Request<pb::TriggerSmartContract>,
    ) -> Result<Response<EstimateEnergyMessage>, Status> {
        self.record(&request, "EstimateEnergy");
        self.take(|s| &mut s.estimate)
    }
}

/// Spawns a `WalletSolidity` server on an ephemeral loopback port.
pub async fn spawn() -> (SocketAddr, Handle) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let addr = listener.local_addr().expect("local addr");
    let state = Arc::new(Mutex::new(State::default()));
    let service = TestWalletSolidity { state: Arc::clone(&state) };

    tokio::spawn(async move {
        Server::builder()
            .add_service(WalletSolidityServer::new(service))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .expect("test server");
    });

    (addr, Handle { state })
}
