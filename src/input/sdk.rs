use std::time::Duration;

use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::{SdkLogger, SdkLoggerProvider};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::SdkTracerProvider;

use crate::error::{Error, Result, Signal};

pub struct TelemetryClient {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
    logger_provider: SdkLoggerProvider,
}

impl TelemetryClient {
    pub fn new(endpoint: &str) -> Result<Self> {
        Self::with_service_name(endpoint, "collector-tester")
    }

    pub fn with_service_name(endpoint: &str, service_name: &str) -> Result<Self> {
        let resource = Resource::builder()
            .with_service_name(service_name.to_string())
            .build();

        let tracer_provider = Self::build_tracer_provider(endpoint, resource.clone())?;
        let meter_provider = Self::build_meter_provider(endpoint, resource.clone())?;
        let logger_provider = Self::build_logger_provider(endpoint, resource)?;

        Ok(Self {
            tracer_provider,
            meter_provider,
            logger_provider,
        })
    }

    fn build_tracer_provider(endpoint: &str, resource: Resource) -> Result<SdkTracerProvider> {
        let exporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::ExporterBuild {
                signal: Signal::Traces,
                message: e.to_string(),
            })?;

        Ok(SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build())
    }

    fn build_meter_provider(endpoint: &str, resource: Resource) -> Result<SdkMeterProvider> {
        let exporter = MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::ExporterBuild {
                signal: Signal::Metrics,
                message: e.to_string(),
            })?;

        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(1))
            .build();

        Ok(SdkMeterProvider::builder()
            .with_resource(resource)
            .with_reader(reader)
            .build())
    }

    fn build_logger_provider(endpoint: &str, resource: Resource) -> Result<SdkLoggerProvider> {
        let exporter = LogExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::ExporterBuild {
                signal: Signal::Logs,
                message: e.to_string(),
            })?;

        Ok(SdkLoggerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build())
    }

    pub fn tracer(&self, name: &'static str) -> opentelemetry_sdk::trace::Tracer {
        self.tracer_provider.tracer(name)
    }

    pub fn meter(&self, name: &'static str) -> opentelemetry::metrics::Meter {
        self.meter_provider.meter(name)
    }

    pub fn logger(&self) -> OpenTelemetryTracingBridge<SdkLoggerProvider, SdkLogger> {
        OpenTelemetryTracingBridge::new(&self.logger_provider)
    }

    #[must_use = "flush result should be handled"]
    pub fn flush(&self) -> Result<()> {
        self.flush_traces()?;
        self.flush_metrics()?;
        self.flush_logs()?;
        Ok(())
    }

    #[must_use = "flush result should be handled"]
    pub fn flush_traces(&self) -> Result<()> {
        self.tracer_provider
            .force_flush()
            .map_err(|e| Error::Flush {
                signal: Signal::Traces,
                message: e.to_string(),
            })
    }

    #[must_use = "flush result should be handled"]
    pub fn flush_metrics(&self) -> Result<()> {
        self.meter_provider.force_flush().map_err(|e| Error::Flush {
            signal: Signal::Metrics,
            message: e.to_string(),
        })
    }

    #[must_use = "flush result should be handled"]
    pub fn flush_logs(&self) -> Result<()> {
        self.logger_provider
            .force_flush()
            .map_err(|e| Error::Flush {
                signal: Signal::Logs,
                message: e.to_string(),
            })
    }

    pub fn shutdown(self) -> Result<()> {
        self.tracer_provider
            .shutdown()
            .map_err(|e| Error::Shutdown {
                signal: Signal::Traces,
                message: e.to_string(),
            })?;
        self.meter_provider
            .shutdown()
            .map_err(|e| Error::Shutdown {
                signal: Signal::Metrics,
                message: e.to_string(),
            })?;
        self.logger_provider
            .shutdown()
            .map_err(|e| Error::Shutdown {
                signal: Signal::Logs,
                message: e.to_string(),
            })?;
        Ok(())
    }
}
