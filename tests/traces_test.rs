mod common;

use std::time::Duration;

use collector_tester::container::{CollectorTestHarness, find_free_port};
use collector_tester::input::TelemetryClient;
use opentelemetry::trace::{TraceContextExt, Tracer};

#[tokio::test]
async fn test_basic_trace_processing() {
    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let harness =
        CollectorTestHarness::builder(common::config_path("basic.yaml"), "OTLP_EXPORTER_ENDPOINT")
            .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
            .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
            .start()
            .await
            .expect("failed to start harness");

    let endpoint = format!("http://127.0.0.1:{http_port}/v1/traces");
    let client = TelemetryClient::new(&endpoint)
        .await
        .expect("failed to create client");

    let tracer = client.tracer("test");
    let span = tracer.span_builder("test-operation").start(&tracer);
    let cx = opentelemetry::Context::current_with_span(span);
    cx.span().end();

    client.flush().expect("failed to flush");

    harness
        .mock_server()
        .wait_for_spans(1, Duration::from_secs(10))
        .await
        .expect("timed out waiting for spans");

    harness
        .mock_server()
        .with_collector(|collector| {
            assert_eq!(collector.span_count(), 1);

            let spans = collector.spans();
            assert!(!spans.is_empty());

            let span = &spans[0];
            assert_eq!(span.span().name, "test-operation");
        })
        .await;

    client.shutdown().expect("failed to shutdown client");
    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
}

#[tokio::test]
async fn test_attribute_processor() {
    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let harness =
        CollectorTestHarness::builder(common::config_path("basic.yaml"), "OTLP_EXPORTER_ENDPOINT")
            .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
            .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
            .start()
            .await
            .expect("failed to start harness");

    let endpoint = format!("http://127.0.0.1:{http_port}/v1/traces");
    let client = TelemetryClient::new(&endpoint)
        .await
        .expect("failed to create client");

    let tracer = client.tracer("test");
    let span = tracer
        .span_builder("attributed-span")
        .with_attributes(vec![opentelemetry::KeyValue::new(
            "original.attr",
            "original-value",
        )])
        .start(&tracer);
    let cx = opentelemetry::Context::current_with_span(span);
    cx.span().end();

    client.flush().expect("failed to flush");

    harness
        .mock_server()
        .wait_for_spans(1, Duration::from_secs(10))
        .await
        .expect("timed out waiting for spans");

    harness
        .mock_server()
        .with_collector(|collector| {
            collector
                .expect_span_with_name("attributed-span")
                .with_attribute("test.processed", "true")
                .assert_exists();
        })
        .await;

    client.shutdown().expect("failed to shutdown client");
    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
}
