mod common;

use std::time::Duration;

use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol};

#[tokio::test]
async fn test_resource_attribute_added_to_all_signals() {
    let (builder, ports) = common::harness_with_ports("resource-attributes.yaml");
    let harness = builder.start().await.expect("failed to start harness");

    let endpoint = ports.http_endpoint();
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(Protocol::HttpBinary)
        .service_name("test-service")
        .build()
        .expect("failed to build OTel SDK");

    let tracer = opentelemetry::global::tracer("test");
    let span = tracer.span_builder("test-span").start(&tracer);
    opentelemetry::Context::current_with_span(span).span().end();

    let meter = opentelemetry::global::meter("test");
    let counter = meter.u64_counter("test.counter").build();
    counter.add(1, &[]);

    tracing::info!("test log message");

    drop(_guard);

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
