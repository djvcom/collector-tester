mod common;

use std::time::Duration;

use collector_tester::container::CollectorTestHarness;
use mock_collector::{MockServer, Protocol};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol as OtelProtocol};

#[tokio::test]
async fn test_failover_to_fallback_on_primary_failure() {
    let primary_mock = MockServer::builder()
        .protocol(Protocol::HttpBinary)
        .start()
        .await
        .expect("failed to start primary mock");

    let fallback_mock = MockServer::builder()
        .protocol(Protocol::Grpc)
        .start()
        .await
        .expect("failed to start fallback mock");

    let ports = common::TestPorts::allocate();
    let primary_endpoint = format!("127.0.0.1:{}", primary_mock.addr().port());
    let fallback_endpoint = format!("127.0.0.1:{}", fallback_mock.addr().port());

    let harness =
        CollectorTestHarness::builder(common::config_path("failover.yaml"), "PRIMARY_ENDPOINT")
            .env_var("COLLECTOR_GRPC_PORT", ports.grpc.to_string())
            .env_var("COLLECTOR_HTTP_PORT", ports.http.to_string())
            .env_var("PRIMARY_ENDPOINT", &primary_endpoint)
            .env_var("FALLBACK_ENDPOINT", &fallback_endpoint)
            .start()
            .await
            .expect("failed to start harness");

    let endpoint = ports.http_endpoint();
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(OtelProtocol::HttpBinary)
        .service_name("failover-test")
        .build()
        .expect("failed to build OTel SDK");

    let tracer = opentelemetry::global::tracer("test");
    let span = tracer.span_builder("failover-test-span").start(&tracer);
    opentelemetry::Context::current_with_span(span).span().end();

    drop(_guard);

    fallback_mock
        .wait_for_spans(1, Duration::from_secs(15))
        .await
        .expect("timed out waiting for spans at fallback");

    fallback_mock
        .with_collector(|collector| {
            assert_eq!(collector.span_count(), 1);
            let spans = collector.spans();
            assert_eq!(spans[0].span().name, "failover-test-span");
        })
        .await;

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
