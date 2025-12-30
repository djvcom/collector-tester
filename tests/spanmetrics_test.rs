mod common;

use std::time::Duration;

use collector_tester::container::{CollectorTestHarness, find_free_port};
use collector_tester::monitor::ContainerMonitor;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol};

#[tokio::test]
async fn test_spanmetrics_generates_metrics_and_reports_memory() {
    let grpc_port = find_free_port().expect("failed to find free port");
    let http_port = find_free_port().expect("failed to find free port");

    let harness = CollectorTestHarness::builder(
        common::config_path("spanmetrics.yaml"),
        "OTLP_EXPORTER_ENDPOINT",
    )
    .env_var("COLLECTOR_GRPC_PORT", grpc_port.to_string())
    .env_var("COLLECTOR_HTTP_PORT", http_port.to_string())
    .start()
    .await
    .expect("failed to start harness");

    // Start memory monitoring
    let mut monitor = ContainerMonitor::new(harness.container_id())
        .await
        .expect("failed to create monitor");

    let endpoint = format!("http://127.0.0.1:{http_port}");
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(Protocol::HttpBinary)
        .service_name("spanmetrics-test")
        .build()
        .expect("failed to build OTel SDK");

    // Take initial memory snapshot
    let initial_mem = monitor
        .sample()
        .await
        .expect("failed to get initial memory");
    let initial_mb = initial_mem.usage_bytes as f64 / 1_000_000.0;
    println!("Initial memory: {:.2} MB", initial_mb);

    // Send spans with http.method attribute (configured dimension)
    // This creates state in the spanmetrics connector
    let tracer = opentelemetry::global::tracer("test");
    let span_count = 500;

    for i in 0..span_count {
        let method = ["GET", "POST", "PUT", "DELETE"][i % 4];
        let span = tracer
            .span_builder(format!("http-request-{}", i % 20))
            .with_attributes([opentelemetry::KeyValue::new(
                "http.method",
                method.to_string(),
            )])
            .start(&tracer);
        opentelemetry::Context::current_with_span(span).span().end();
    }

    drop(_guard);

    // Wait for spanmetrics connector to generate metrics
    harness
        .mock_server()
        .wait_for_metrics(1, Duration::from_secs(10))
        .await
        .expect("timed out waiting for spanmetrics");

    // Take final memory snapshot
    let final_mem = monitor.sample().await.expect("failed to get final memory");
    let final_mb = final_mem.usage_bytes as f64 / 1_000_000.0;

    // Verify metrics were generated from spans
    harness
        .mock_server()
        .with_collector(|collector| {
            let metric_count = collector.metric_count();
            println!("Generated {metric_count} metrics from {span_count} spans");
            assert!(metric_count > 0, "expected spanmetrics to generate metrics");

            // Check for expected metric names (spanmetrics generates duration histograms)
            collector
                .expect_metric_with_name("test.spanmetrics.duration")
                .assert_exists();
        })
        .await;

    // Report memory stats (no threshold assertion as per requirements)
    println!("Final memory: {:.2} MB", final_mb);
    println!("Memory delta: {:.2} MB", final_mb - initial_mb);
    println!(
        "Memory per 1000 spans: {:.2} KB",
        (final_mb - initial_mb) * 1024.0 * (1000.0 / span_count as f64)
    );

    harness
        .shutdown()
        .await
        .expect("failed to shutdown harness");
}
