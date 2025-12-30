mod common;

use std::time::Duration;

use collector_tester::monitor::ContainerMonitor;
use opentelemetry::trace::{TraceContextExt, Tracer};
use opentelemetry_configuration::{OtelSdkBuilder, Protocol};

#[tokio::test]
async fn test_spanmetrics_generates_metrics_and_reports_memory() {
    let (builder, ports) = common::harness_with_ports("spanmetrics.yaml");
    let harness = builder.start().await.expect("failed to start harness");

    let mut monitor = ContainerMonitor::new(harness.container_id())
        .await
        .expect("failed to create monitor");

    let endpoint = ports.http_endpoint();
    let _guard = OtelSdkBuilder::new()
        .endpoint(&endpoint)
        .protocol(Protocol::HttpBinary)
        .service_name("spanmetrics-test")
        .build()
        .expect("failed to build OTel SDK");

    let initial_mem = monitor
        .sample()
        .await
        .expect("failed to get initial memory");
    let initial_mb = initial_mem.usage_bytes as f64 / 1_000_000.0;
    println!("Initial memory: {initial_mb:.2} MB");

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

    harness
        .mock_server()
        .wait_for_metrics(1, Duration::from_secs(10))
        .await
        .expect("timed out waiting for spanmetrics");

    let final_mem = monitor.sample().await.expect("failed to get final memory");
    let final_mb = final_mem.usage_bytes as f64 / 1_000_000.0;

    harness
        .mock_server()
        .with_collector(|collector| {
            let metric_count = collector.metric_count();
            println!("Generated {metric_count} metrics from {span_count} spans");
            assert!(metric_count > 0, "expected spanmetrics to generate metrics");

            collector
                .expect_metric_with_name("test.spanmetrics.duration")
                .assert_exists();
        })
        .await;

    println!("Final memory: {final_mb:.2} MB");
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
