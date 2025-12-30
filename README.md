# collector-tester

A Rust library for testing OpenTelemetry Collector configurations in isolation.

Spins up a real collector container, sends telemetry through it, and asserts on what comes out the other side. Useful for validating attribute processing, routing, filtering, failover behaviour, and anything else that happens between receivers and exporters.

## Usage

```rust
use collector_tester::container::CollectorTestHarness;

let harness = CollectorTestHarness::builder("config.yaml", "EXPORTER_ENDPOINT")
    .env_var("COLLECTOR_HTTP_PORT", "4318")
    .start()
    .await?;

// Send telemetry, then assert on what the mock server received
harness.mock_server()
    .wait_for_spans(1, Duration::from_secs(5))
    .await?;

harness.mock_server()
    .with_collector(|collector| {
        collector
            .expect_span_with_name("my-span")
            .with_attribute("processed", "true")
            .assert_exists();
    })
    .await;
```

## Requirements

- Docker (for testcontainers)
- Rust 1.75+

## Running tests

```bash
cargo test
```

## Licence

MIT
