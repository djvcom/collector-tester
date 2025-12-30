use std::time::{Duration, Instant};

use opentelemetry::KeyValue;
use opentelemetry::trace::{SpanKind, TraceContextExt, Tracer};
use tokio::time::interval;

use super::sdk::TelemetryClient;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct LoadConfig {
    pub spans_per_second: u32,
    pub metrics_per_second: u32,
    pub logs_per_second: u32,
    pub duration: Duration,
    pub span_attributes_count: usize,
    pub unique_span_names: usize,
}

impl Default for LoadConfig {
    fn default() -> Self {
        Self {
            spans_per_second: 100,
            metrics_per_second: 50,
            logs_per_second: 50,
            duration: Duration::from_secs(60),
            span_attributes_count: 10,
            unique_span_names: 100,
        }
    }
}

#[derive(Debug, Default)]
pub struct LoadStats {
    pub spans_sent: usize,
    pub metrics_sent: usize,
    pub logs_sent: usize,
    pub duration: Duration,
}

impl LoadStats {
    fn rate(&self, count: usize) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs > 0.0 { count as f64 / secs } else { 0.0 }
    }

    pub fn spans_per_second(&self) -> f64 {
        self.rate(self.spans_sent)
    }

    pub fn metrics_per_second(&self) -> f64 {
        self.rate(self.metrics_sent)
    }
}

pub struct LoadGenerator<'a> {
    client: &'a TelemetryClient,
    config: LoadConfig,
}

impl<'a> LoadGenerator<'a> {
    pub fn new(client: &'a TelemetryClient, config: LoadConfig) -> Self {
        Self { client, config }
    }

    pub async fn run(&self) -> Result<LoadStats> {
        let start = Instant::now();
        let mut stats = LoadStats::default();

        if self.config.spans_per_second > 0 {
            let span_interval = Duration::from_secs_f64(1.0 / self.config.spans_per_second as f64);
            let mut span_ticker = interval(span_interval);
            let tracer = self.client.tracer("load-generator");

            while start.elapsed() < self.config.duration {
                span_ticker.tick().await;

                let span_name = format!(
                    "load-span-{}",
                    stats.spans_sent % self.config.unique_span_names
                );

                let attributes: Vec<_> = (0..self.config.span_attributes_count)
                    .map(|i| {
                        KeyValue::new(format!("attr-{i}"), format!("value-{}", stats.spans_sent))
                    })
                    .collect();

                let span = tracer
                    .span_builder(span_name)
                    .with_kind(SpanKind::Internal)
                    .with_attributes(attributes)
                    .start(&tracer);

                opentelemetry::Context::current_with_span(span).span().end();
                stats.spans_sent += 1;
            }
        }

        self.client.flush()?;
        stats.duration = start.elapsed();
        Ok(stats)
    }
}
