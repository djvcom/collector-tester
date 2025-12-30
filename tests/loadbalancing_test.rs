mod common;

use std::time::Duration;

use collector_tester::container::{CollectorTestHarness, find_free_port};
use mock_collector::{MockServer, Protocol};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol as OtelProtocol};

#[tokio::test]
async fn test_loadbalancing_distributes_spans() {
    // Start two backend mocks
    let backend_1 = MockServer::builder()
        .protocol(Protocol::Grpc)
        .start()
        .await
        .expect("failed to start backend 1");

    let backend_2 = MockServer::builder()
        .protocol(Protocol::Grpc)
        .start()
        .await
        .expect("failed to start backend 2");

    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let backend_1_addr = format!("127.0.0.1:{}", backend_1.addr().port());
    let backend_2_addr = format!("127.0.0.1:{}", backend_2.addr().port());

    let harness = CollectorTestHarness::builder(
        common::config_path("loadbalancing.yaml"),
        "BACKEND_1", // Placeholder - we set both backends via env vars
    )
    .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
    .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
    .env_var("BACKEND_1", &backend_1_addr)
    .env_var("BACKEND_2", &backend_2_addr)
    .start()
    .await
    .expect("failed to start harness");

    let endpoint = format!("http://127.0.0.1:{http_port}");
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(OtelProtocol::HttpBinary)
        .service_name("loadbalancing-test")
        .build()
        .expect("failed to build OTel SDK");

    // Send multiple spans - load balancer distributes by trace ID
    // Each root span gets a new trace ID, so they should distribute
    let tracer = opentelemetry::global::tracer("test");
    for i in 0..20 {
        let span = tracer
            .span_builder(format!("load-balanced-span-{i}"))
            .start(&tracer);
        opentelemetry::Context::current_with_span(span).span().end();
    }

    drop(_guard);

    // Wait for spans to arrive
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check both backends received spans
    let mut backend_1_count = 0;
    let mut backend_2_count = 0;

    backend_1
        .with_collector(|collector| {
            backend_1_count = collector.span_count();
        })
        .await;

    backend_2
        .with_collector(|collector| {
            backend_2_count = collector.span_count();
        })
        .await;

    println!("Backend 1 received: {backend_1_count} spans");
    println!("Backend 2 received: {backend_2_count} spans");

    // Assert both backends received some spans (load was distributed)
    assert!(
        backend_1_count > 0 && backend_2_count > 0,
        "expected both backends to receive spans, got backend_1={backend_1_count}, backend_2={backend_2_count}"
    );

    // Total should equal what we sent
    assert_eq!(
        backend_1_count + backend_2_count,
        20,
        "expected total of 20 spans"
    );

    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
    backend_1
        .shutdown()
        .await
        .expect("failed to shutdown backend 1");
    backend_2
        .shutdown()
        .await
        .expect("failed to shutdown backend 2");
}
