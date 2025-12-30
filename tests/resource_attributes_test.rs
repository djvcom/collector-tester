mod common;

use std::time::Duration;

use collector_tester::container::{CollectorTestHarness, find_free_port};
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol};

#[tokio::test]
async fn test_resource_attribute_added_to_all_signals() {
    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let harness = CollectorTestHarness::builder(
        common::config_path("resource-attributes.yaml"),
        "OTLP_EXPORTER_ENDPOINT",
    )
    .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
    .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
    .start()
    .await
    .expect("failed to start harness");

    // Use HTTP protocol - it auto-appends /v1/traces, /v1/metrics, /v1/logs
    let endpoint = format!("http://127.0.0.1:{http_port}");
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(Protocol::HttpBinary)
        .service_name("test-service")
        .build()
        .expect("failed to build OTel SDK");

    // Send a trace
    let tracer = opentelemetry::global::tracer("test");
    let span = tracer.span_builder("test-span").start(&tracer);
    opentelemetry::Context::current_with_span(span).span().end();

    // Send a metric
    let meter = opentelemetry::global::meter("test");
    let counter = meter.u64_counter("test.counter").build();
    counter.add(1, &[]);

    // Send a log (tracing is automatically wired up)
    tracing::info!("test log message");

    // Drop guard to flush
    drop(_guard);

    // Wait for all signals
    let timeout = Duration::from_secs(5);

    harness
        .mock_server()
        .wait_for_spans(1, timeout)
        .await
        .expect("timed out waiting for spans");

    harness
        .mock_server()
        .wait_for_metrics(1, timeout)
        .await
        .expect("timed out waiting for metrics");

    harness
        .mock_server()
        .wait_for_logs(1, timeout)
        .await
        .expect("timed out waiting for logs");

    // Assert resource attribute on all signals
    harness
        .mock_server()
        .with_collector(|collector| {
            collector
                .expect_span_with_name("test-span")
                .with_resource_attributes([("telemetry.processed_by", "main-collector")])
                .assert_exists();

            collector
                .expect_metric_with_name("test.counter")
                .with_resource_attributes([("telemetry.processed_by", "main-collector")])
                .assert_exists();

            collector
                .expect_log_with_body("test log message")
                .with_resource_attributes([("telemetry.processed_by", "main-collector")])
                .assert_exists();
        })
        .await;

    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
}
