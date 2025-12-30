use std::time::Duration;

use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::SdkTracerProvider;

use crate::error::{Error, Result};

pub struct TelemetryClient {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
    endpoint: String,
}

impl TelemetryClient {
    pub async fn new(endpoint: &str) -> Result<Self> {
        Self::with_service_name(endpoint, "collector-tester").await
    }

    pub async fn with_service_name(endpoint: &str, service_name: &str) -> Result<Self> {
        let resource = Resource::builder()
            .with_service_name(service_name.to_string())
            .build();

        let tracer_provider = Self::build_tracer_provider(endpoint, resource.clone()).await?;
        let meter_provider = Self::build_meter_provider(endpoint, resource.clone()).await?;
        let logger_provider = Self::build_logger_provider(endpoint, resource).await?;

        Ok(Self {
            tracer_provider,
            meter_provider,
            logger_provider,
            endpoint: endpoint.to_string(),
        })
    }

    async fn build_tracer_provider(
        endpoint: &str,
        resource: Resource,
    ) -> Result<SdkTracerProvider> {
        let exporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Input(format!("failed to create span exporter: {e}")))?;

        let provider = SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build();

        Ok(provider)
    }

    async fn build_meter_provider(endpoint: &str, resource: Resource) -> Result<SdkMeterProvider> {
        let exporter = MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Input(format!("failed to create metric exporter: {e}")))?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(1))
            .build();

        let provider = SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build();

        Ok(provider)
    }

    async fn build_logger_provider(
        endpoint: &str,
        resource: Resource,
    ) -> Result<SdkLoggerProvider> {
        let exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Input(format!("failed to create log exporter: {e}")))?;

        let provider = SdkLoggerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build();

        Ok(provider)
    }

    pub fn tracer(&self, name: &'static str) -> opentelemetry_sdk::trace::Tracer {
        self.tracer_provider.tracer(name)
    }

    pub fn meter(&self, name: &'static str) -> opentelemetry::metrics::Meter {
        self.meter_provider.meter(name)
    }

    pub fn logger_provider(&self) -> &SdkLoggerProvider {
        &self.logger_provider
    }

    pub fn tracer_provider(&self) -> &SdkTracerProvider {
        &self.tracer_provider
    }

    pub fn meter_provider(&self) -> &SdkMeterProvider {
        &self.meter_provider
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn flush(&self) -> Result<()> {
        self.tracer_provider
            .force_flush()
            .map_err(|e| Error::Otel(format!("failed to flush traces: {e}")))?;
        self.meter_provider
            .force_flush()
            .map_err(|e| Error::Otel(format!("failed to flush metrics: {e}")))?;
        self.logger_provider
            .force_flush()
            .map_err(|e| Error::Otel(format!("failed to flush logs: {e}")))?;
        Ok(())
    }

    pub fn shutdown(self) -> Result<()> {
        self.tracer_provider
            .shutdown()
            .map_err(|e| Error::Otel(format!("failed to shutdown traces: {e}")))?;
        self.meter_provider
            .shutdown()
            .map_err(|e| Error::Otel(format!("failed to shutdown metrics: {e}")))?;
        self.logger_provider
            .shutdown()
            .map_err(|e| Error::Otel(format!("failed to shutdown logs: {e}")))?;
        Ok(())
    }
}
