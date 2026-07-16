//! End-to-end gRPC tracing overhead benchmark.
//!
//! Run without a subscriber, with a disabled subscriber, and at DEBUG:
//!
//! ```text
//! cargo bench -p tronz-provider --bench observability
//! TRONZ_BENCH_TRACING=disabled cargo bench -p tronz-provider --bench observability
//! TRONZ_BENCH_TRACING=debug cargo bench -p tronz-provider --bench observability
//! ```

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use tronz_provider::transport::{
    TronTransport as _,
    grpc::{GrpcTransport, RetryConfig},
};
#[path = "../tests/support/mod.rs"]
mod support;

use support::{ServerConfig, TestServer};

fn configure_tracing() {
    let Ok(mode) = std::env::var("TRONZ_BENCH_TRACING") else {
        return;
    };

    let result = match mode.as_str() {
        "disabled" => tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_max_level(tracing::level_filters::LevelFilter::OFF)
                .with_writer(std::io::sink)
                .finish(),
        ),
        "debug" => tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .with_writer(std::io::sink)
                .finish(),
        ),
        other => panic!("unsupported TRONZ_BENCH_TRACING value: {other}"),
    };
    result.expect("install benchmark tracing subscriber");
}

fn observability(c: &mut Criterion) {
    configure_tracing();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build benchmark runtime");
    let server = runtime.block_on(TestServer::start(ServerConfig::default()));
    let transport = runtime.block_on(
        GrpcTransport::builder().with_retry(RetryConfig::disabled()).connect(&server.endpoint),
    );
    let transport = transport.expect("connect benchmark transport");

    c.bench_function("grpc_get_now_block", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            let block = transport.get_now_block().await.expect("benchmark RPC succeeds");
            black_box(block)
        });
    });

    assert!(server.get_now_calls() > 0, "benchmark did not reach the test server");
    assert_eq!(server.broadcast_calls(), 0, "benchmark unexpectedly broadcast a transaction");
}

criterion_group!(benches, observability);
criterion_main!(benches);
