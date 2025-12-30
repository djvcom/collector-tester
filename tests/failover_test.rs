mod common;

use std::time::Duration;

use collector_tester::container::{CollectorTestHarness, find_free_port};
use mock_collector::{MockServer, Protocol};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol as OtelProtocol};

#[tokio::test]
async fn test_failover_to_fallback_on_primary_failure() {
    // Start primary mock with wrong protocol (HTTP when collector sends gRPC)
    // This should cause the collector to fail sending to primary
    let primary_mock = MockServer::builder()
        .protocol(Protocol::HttpBinary)
        .start()
        .await
        .expect("failed to start primary mock");

    // Start fallback mock with correct protocol (gRPC)
    let fallback_mock = MockServer::builder()
        .protocol(Protocol::Grpc)
        .start()
        .await
        .expect("failed to start fallback mock");

    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let primary_endpoint = format!("127.0.0.1:{}", primary_mock.addr().port());
    let fallback_endpoint = format!("127.0.0.1:{}", fallback_mock.addr().port());

    let harness = CollectorTestHarness::builder(
        common::config_path("failover.yaml"),
        "PRIMARY_ENDPOINT", // Not used for mock endpoint, just a placeholder
    )
    .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
    .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
    .env_var("PRIMARY_ENDPOINT", &primary_endpoint)
    .env_var("FALLBACK_ENDPOINT", &fallback_endpoint)
    .start()
    .await
    .expect("failed to start harness");

    let endpoint = format!("http://127.0.0.1:{http_port}");
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(OtelProtocol::HttpBinary)
        .service_name("failover-test")
        .build()
        .expect("failed to build OTel SDK");

    // Send a trace
    let tracer = opentelemetry::global::tracer("test");
    let span = tracer.span_builder("failover-test-span").start(&tracer);
    opentelemetry::Context::current_with_span(span).span().end();

    drop(_guard);

    // Wait for span to arrive at fallback
    fallback_mock
        .wait_for_spans(1, Duration::from_secs(15))
        .await
        .expect("timed out waiting for spans at fallback");

    // Assert fallback received the span
    fallback_mock
        .with_collector(|collector| {
            assert_eq!(collector.span_count(), 1);
            let spans = collector.spans();
            assert_eq!(spans[0].span().name, "failover-test-span");
        })
        .await;

    // Primary should have received nothing (wrong protocol)
    primary_mock
        .with_collector(|collector| {
            assert_eq!(
                collector.span_count(),
                0,
                "primary should not have received spans"
            );
        })
        .await;

    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
    primary_mock
        .shutdown()
        .await
        .expect("failed to shutdown primary");
    fallback_mock
        .shutdown()
        .await
        .expect("failed to shutdown fallback");
}
