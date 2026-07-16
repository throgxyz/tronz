use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};

use tokio::{net::TcpListener, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    Request, Response, Status,
    body::Body,
    codegen::{Service, http},
    server::NamedService,
    transport::Server,
};

mod proto {
    include!("generated.rs");
}

use proto::{
    BlockExtension, BlockHeader, Return, Transaction, TransactionExtension, TransferContract,
    block_header,
    wallet_server::{Wallet, WalletServer},
};

/// Behavior controls for the local observability test server.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ServerConfig {
    pub(crate) get_now_failures: usize,
    pub(crate) broadcast_status: Option<tonic::Code>,
    pub(crate) broadcast_result: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { get_now_failures: 0, broadcast_status: None, broadcast_result: true }
    }
}

#[derive(Clone)]
struct TestWallet {
    state: Arc<ServerState>,
    config: ServerConfig,
}

struct ServerState {
    get_now_failures: AtomicUsize,
    get_now_calls: AtomicUsize,
    broadcast_calls: AtomicUsize,
}

#[tonic::async_trait]
impl Wallet for TestWallet {
    async fn get_now_block2(
        &self,
        _request: Request<proto::EmptyMessage>,
    ) -> Result<Response<BlockExtension>, Status> {
        self.state.get_now_calls.fetch_add(1, Ordering::Relaxed);
        if self
            .state
            .get_now_failures
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            return Err(Status::unavailable("temporary test failure"));
        }

        Ok(Response::new(BlockExtension {
            block_header: Some(BlockHeader {
                raw_data: Some(block_header::Raw { timestamp: 1_234, number: 42 }),
            }),
            blockid: vec![7; 32],
        }))
    }

    async fn create_transaction2(
        &self,
        _request: Request<TransferContract>,
    ) -> Result<Response<TransactionExtension>, Status> {
        Ok(Response::new(TransactionExtension {
            transaction: Some(Transaction {
                raw_data: Some(proto::transaction::Raw { expiration: 2_000, timestamp: 1_000 }),
                signature: Vec::new(),
            }),
            txid: vec![9; 32],
            result: Some(Return { result: true, code: 0, message: Vec::new() }),
        }))
    }

    async fn broadcast_transaction(
        &self,
        _request: Request<Transaction>,
    ) -> Result<Response<Return>, Status> {
        self.state.broadcast_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(code) = self.config.broadcast_status {
            return Err(Status::new(code, "broadcast test failure"));
        }

        Ok(Response::new(Return {
            result: self.config.broadcast_result,
            code: if self.config.broadcast_result { 0 } else { 1 },
            message: if self.config.broadcast_result {
                Vec::new()
            } else {
                b"rejected by test node".to_vec()
            },
        }))
    }
}

/// Local HTTP/2 server exposing the production Wallet service paths.
pub(crate) struct TestServer {
    pub(crate) endpoint: String,
    state: Arc<ServerState>,
    task: JoinHandle<()>,
}

impl TestServer {
    pub(crate) async fn start(config: ServerConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test server");
        let address = listener.local_addr().expect("read test server address");
        let state = Arc::new(ServerState {
            get_now_failures: AtomicUsize::new(config.get_now_failures),
            get_now_calls: AtomicUsize::new(0),
            broadcast_calls: AtomicUsize::new(0),
        });
        let wallet = TestWallet { state: Arc::clone(&state), config };
        let wallet_service = ProtocolWallet::new(wallet);
        let task = tokio::spawn(async move {
            Server::builder()
                .add_service(wallet_service)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .expect("run test server");
        });

        Self { endpoint: format!("http://{address}"), state, task }
    }

    pub(crate) fn get_now_calls(&self) -> usize {
        self.state.get_now_calls.load(Ordering::Relaxed)
    }

    pub(crate) fn broadcast_calls(&self) -> usize {
        self.state.broadcast_calls.load(Ordering::Relaxed)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Rewrites production Wallet paths to the generated test service package.
struct ProtocolWallet<T> {
    inner: WalletServer<T>,
}

impl<T> Clone for ProtocolWallet<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<T> ProtocolWallet<T> {
    fn new(wallet: T) -> Self {
        Self { inner: WalletServer::new(wallet) }
    }
}

impl<T> Service<http::Request<Body>> for ProtocolWallet<T>
where
    T: Wallet,
{
    type Response = <WalletServer<T> as Service<http::Request<Body>>>::Response;
    type Error = <WalletServer<T> as Service<http::Request<Body>>>::Error;
    type Future = <WalletServer<T> as Service<http::Request<Body>>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <WalletServer<T> as Service<http::Request<Body>>>::poll_ready(&mut self.inner, cx)
    }

    fn call(&mut self, mut request: http::Request<Body>) -> Self::Future {
        let rewritten = match request.uri().path() {
            "/protocol.Wallet/GetNowBlock2" => Some("/tronz.observability.Wallet/GetNowBlock2"),
            "/protocol.Wallet/CreateTransaction2" => {
                Some("/tronz.observability.Wallet/CreateTransaction2")
            }
            "/protocol.Wallet/BroadcastTransaction" => {
                Some("/tronz.observability.Wallet/BroadcastTransaction")
            }
            _ => None,
        };
        if let Some(rewritten) = rewritten {
            *request.uri_mut() = http::Uri::from_static(rewritten);
        }
        self.inner.call(request)
    }
}

impl<T: Wallet> NamedService for ProtocolWallet<T> {
    const NAME: &'static str = "protocol.Wallet";
}
