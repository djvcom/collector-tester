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

impl LoadConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_spans_per_second(mut self, rate: u32) -> Self {
        self.spans_per_second = rate;
        self
    }

    pub fn with_metrics_per_second(mut self, rate: u32) -> Self {
        self.metrics_per_second = rate;
        self
    }

    pub fn with_logs_per_second(mut self, rate: u32) -> Self {
        self.logs_per_second = rate;
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_span_attributes_count(mut self, count: usize) -> Self {
        self.span_attributes_count = count;
        self
    }

    pub fn with_unique_span_names(mut self, count: usize) -> Self {
        self.unique_span_names = count;
        self
    }
}

#[derive(Debug, Default)]
pub struct LoadStats {
    pub spans_sent: usize,
    pub metrics_sent: usize,
    pub logs_sent: usize,
    pub duration: Duration,
    pub errors: usize,
}

impl LoadStats {
    pub fn spans_per_second(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.spans_sent as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn metrics_per_second(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.metrics_sent as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn logs_per_second(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.logs_sent as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        }
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

            while start.elapsed() < self.config.duration {
                span_ticker.tick().await;
                if self.generate_span(&mut stats).is_err() {
                    stats.errors += 1;
                }
            }
        }

        self.client.flush()?;
        stats.duration = start.elapsed();
        Ok(stats)
    }

    pub async fn run_all(&self) -> Result<LoadStats> {
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

                let mut attributes = Vec::with_capacity(self.config.span_attributes_count);
                for i in 0..self.config.span_attributes_count {
                    attributes.push(KeyValue::new(
                        format!("attr-{}", i),
                        format!("value-{}", uuid::Uuid::new_v4()),
                    ));
                }

                let span = tracer
                    .span_builder(span_name)
                    .with_kind(SpanKind::Internal)
                    .with_attributes(attributes)
                    .start(&tracer);

                let cx = opentelemetry::Context::current_with_span(span);
                cx.span().end();

                stats.spans_sent += 1;
            }
        }

        self.client.flush()?;
        stats.duration = start.elapsed();
        Ok(stats)
    }

    fn generate_span(&self, stats: &mut LoadStats) -> Result<()> {
        let tracer = self.client.tracer("load-generator");
        let span_name = format!(
            "load-span-{}",
            stats.spans_sent % self.config.unique_span_names
        );

        let mut attributes = Vec::with_capacity(self.config.span_attributes_count);
        for i in 0..self.config.span_attributes_count {
            attributes.push(KeyValue::new(
                format!("attr-{}", i),
                format!("value-{}", uuid::Uuid::new_v4()),
            ));
        }

        let span = tracer
            .span_builder(span_name)
            .with_kind(SpanKind::Internal)
            .with_attributes(attributes)
            .start(&tracer);

        let cx = opentelemetry::Context::current_with_span(span);
        cx.span().end();

        stats.spans_sent += 1;
        Ok(())
    }
}
