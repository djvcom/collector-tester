use std::collections::HashMap;
use std::path::Path;

use opentelemetry::KeyValue;
use opentelemetry::trace::{SpanKind, Status, TraceContextExt, Tracer};
use serde::{Deserialize, Serialize};

use super::sdk::TelemetryClient;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFixture {
    pub spans: Vec<SpanFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanFixture {
    pub name: String,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub events: Vec<EventFixture>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<StatusFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFixture {
    pub name: String,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusFixture {
    pub code: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFixture {
    pub name: String,
    pub metric_type: MetricType,
    pub data_points: Vec<DataPointFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Counter,
    UpDownCounter,
    Gauge,
    Histogram,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPointFixture {
    pub value: f64,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFixture {
    pub body: String,
    pub severity: String,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

pub struct FixtureLoader;

impl FixtureLoader {
    pub fn load_traces(path: impl AsRef<Path>) -> Result<TraceFixture> {
        let content = std::fs::read_to_string(&path)?;
        Self::parse_traces(&content, path.as_ref())
    }

    pub fn load_metrics(path: impl AsRef<Path>) -> Result<Vec<MetricFixture>> {
        let content = std::fs::read_to_string(&path)?;
        Self::parse_metrics(&content, path.as_ref())
    }

    pub fn load_logs(path: impl AsRef<Path>) -> Result<Vec<LogFixture>> {
        let content = std::fs::read_to_string(&path)?;
        Self::parse_logs(&content, path.as_ref())
    }

    fn parse_traces(content: &str, path: &Path) -> Result<TraceFixture> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => serde_json::from_str(content).map_err(|e| Error::Config(e.to_string())),
            Some("yaml" | "yml") => {
                serde_yaml::from_str(content).map_err(|e| Error::Config(e.to_string()))
            }
            _ => Err(Error::Config(
                "unsupported fixture format (use .json or .yaml)".to_string(),
            )),
        }
    }

    fn parse_metrics(content: &str, path: &Path) -> Result<Vec<MetricFixture>> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => serde_json::from_str(content).map_err(|e| Error::Config(e.to_string())),
            Some("yaml" | "yml") => {
                serde_yaml::from_str(content).map_err(|e| Error::Config(e.to_string()))
            }
            _ => Err(Error::Config(
                "unsupported fixture format (use .json or .yaml)".to_string(),
            )),
        }
    }

    fn parse_logs(content: &str, path: &Path) -> Result<Vec<LogFixture>> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => serde_json::from_str(content).map_err(|e| Error::Config(e.to_string())),
            Some("yaml" | "yml") => {
                serde_yaml::from_str(content).map_err(|e| Error::Config(e.to_string()))
            }
            _ => Err(Error::Config(
                "unsupported fixture format (use .json or .yaml)".to_string(),
            )),
        }
    }
}

pub struct FixtureSender<'a> {
    client: &'a TelemetryClient,
}

impl<'a> FixtureSender<'a> {
    pub fn new(client: &'a TelemetryClient) -> Self {
        Self { client }
    }

    pub fn send_traces(&self, fixture: &TraceFixture) -> Result<()> {
        let tracer = self.client.tracer("fixture-sender");

        for span_fixture in &fixture.spans {
            let kind = span_fixture
                .kind
                .as_ref()
                .map(|k| match k.to_lowercase().as_str() {
                    "internal" => SpanKind::Internal,
                    "server" => SpanKind::Server,
                    "client" => SpanKind::Client,
                    "producer" => SpanKind::Producer,
                    "consumer" => SpanKind::Consumer,
                    _ => SpanKind::Internal,
                })
                .unwrap_or(SpanKind::Internal);

            let span_name = span_fixture.name.clone();
            let mut span_builder = tracer.span_builder(span_name).with_kind(kind);

            let attributes: Vec<KeyValue> = span_fixture
                .attributes
                .iter()
                .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                .collect();

            span_builder = span_builder.with_attributes(attributes);

            let span = span_builder.start(&tracer);
            let cx = opentelemetry::Context::current_with_span(span);

            for event in &span_fixture.events {
                let event_name = event.name.clone();
                let event_attrs: Vec<KeyValue> = event
                    .attributes
                    .iter()
                    .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                    .collect();

                cx.span().add_event(event_name, event_attrs);
            }

            if let Some(status) = &span_fixture.status {
                let status = match status.code.to_lowercase().as_str() {
                    "ok" => Status::Ok,
                    "error" => Status::error(status.message.clone().unwrap_or_default()),
                    _ => Status::Unset,
                };
                cx.span().set_status(status);
            }

            cx.span().end();
        }

        self.client.flush()?;
        Ok(())
    }

    pub fn send_metrics(&self, fixtures: &[MetricFixture]) -> Result<()> {
        let meter = self.client.meter("fixture-sender");

        for fixture in fixtures {
            let metric_name = fixture.name.clone();
            match fixture.metric_type {
                MetricType::Counter => {
                    let counter = meter.u64_counter(metric_name).build();
                    for dp in &fixture.data_points {
                        let attrs: Vec<KeyValue> = dp
                            .attributes
                            .iter()
                            .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                            .collect();
                        counter.add(dp.value as u64, &attrs);
                    }
                }
                MetricType::UpDownCounter => {
                    let counter = meter.i64_up_down_counter(metric_name).build();
                    for dp in &fixture.data_points {
                        let attrs: Vec<KeyValue> = dp
                            .attributes
                            .iter()
                            .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                            .collect();
                        counter.add(dp.value as i64, &attrs);
                    }
                }
                MetricType::Gauge => {
                    let gauge = meter.f64_gauge(metric_name).build();
                    for dp in &fixture.data_points {
                        let attrs: Vec<KeyValue> = dp
                            .attributes
                            .iter()
                            .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                            .collect();
                        gauge.record(dp.value, &attrs);
                    }
                }
                MetricType::Histogram => {
                    let histogram = meter.f64_histogram(metric_name).build();
                    for dp in &fixture.data_points {
                        let attrs: Vec<KeyValue> = dp
                            .attributes
                            .iter()
                            .map(|(k, v)| KeyValue::new(k.clone(), json_value_to_string(v)))
                            .collect();
                        histogram.record(dp.value, &attrs);
                    }
                }
            }
        }

        self.client.flush()?;
        Ok(())
    }
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => value.to_string(),
    }
}
