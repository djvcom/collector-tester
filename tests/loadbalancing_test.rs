mod common;

use std::time::Duration;

use collector_tester::container::CollectorTestHarness;
use mock_collector::{MockServer, Protocol};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol as OtelProtocol};

#[tokio::test]
async fn test_loadbalancing_distributes_spans() {
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

    let ports = common::TestPorts::allocate();
    let backend_1_addr = format!("127.0.0.1:{}", backend_1.addr().port());
    let backend_2_addr = format!("127.0.0.1:{}", backend_2.addr().port());

    let harness =
        CollectorTestHarness::builder(common::config_path("loadbalancing.yaml"), "BACKEND_1")
            .env_var("COLLECTOR_GRPC_PORT", ports.grpc.to_string())
            .env_var("COLLECTOR_HTTP_PORT", ports.http.to_string())
            .env_var("BACKEND_1", &backend_1_addr)
            .env_var("BACKEND_2", &backend_2_addr)
            .start()
            .await
            .expect("failed to start harness");

    let endpoint = ports.http_endpoint();
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(OtelProtocol::HttpBinary)
        .service_name("loadbalancing-test")
        .build()
        .expect("failed to build OTel SDK");

    let tracer = opentelemetry::global::tracer("test");
    for i in 0..20 {
        let span = tracer
            .span_builder(format!("load-balanced-span-{i}"))
            .start(&tracer);
        opentelemetry::Context::current_with_span(span).span().end();
    }

    drop(_guard);

    tokio::time::sleep(Duration::from_secs(3)).await;

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

    assert!(
        backend_1_count > 0 && backend_2_count > 0,
        "expected both backends to receive spans, got backend_1={backend_1_count}, backend_2={backend_2_count}"
    );

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
