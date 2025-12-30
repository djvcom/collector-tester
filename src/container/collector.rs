use testcontainers::Image;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::core::{ContainerPort, WaitFor};

pub const OTLP_GRPC_PORT: ContainerPort = ContainerPort::Tcp(4317);
pub const OTLP_HTTP_PORT: ContainerPort = ContainerPort::Tcp(4318);
pub const METRICS_PORT: ContainerPort = ContainerPort::Tcp(8888);

#[derive(Debug, Clone)]
pub struct CollectorImage {
    tag: String,
}

impl CollectorImage {
    pub fn new(tag: impl Into<String>) -> Self {
        Self { tag: tag.into() }
    }

    pub fn latest() -> Self {
        Self::new("latest")
    }
}

impl Default for CollectorImage {
    fn default() -> Self {
        Self::latest()
    }
}

impl Image for CollectorImage {
    fn name(&self) -> &str {
        "otel/opentelemetry-collector-contrib"
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        // Wait for the OTLP HTTP endpoint to accept requests
        vec![WaitFor::http(
            HttpWaitStrategy::new("/v1/traces")
                .with_port(OTLP_HTTP_PORT)
                .with_expected_status_code(405_u16), // GET returns 405, but means it's listening
        )]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[OTLP_GRPC_PORT, OTLP_HTTP_PORT, METRICS_PORT]
    }
}
