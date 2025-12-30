pub mod collector;
pub mod config;

use std::path::Path;
use std::time::Duration;

use mock_collector::{MockServer, Protocol, ServerHandle};
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::error::{Error, Result};

pub use collector::{CollectorImage, OTLP_GRPC_PORT, OTLP_HTTP_PORT};
pub use config::{CollectorConfig, CollectorConfigBuilder};

const CONTAINER_CONFIG_PATH: &str = "/etc/otelcol-contrib/config.yaml";
const COLLECTOR_IMAGE: &str = "otel/opentelemetry-collector-contrib";

fn find_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| Error::Container(format!("failed to find free port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| Error::Container(format!("failed to get local address: {e}")))?
        .port();
    Ok(port)
}

pub struct CollectorTestHarness {
    mock_server: ServerHandle,
    container: ContainerAsync<GenericImage>,
    container_id: String,
    collector_grpc_port: u16,
    collector_http_port: u16,
}

impl CollectorTestHarness {
    pub async fn start(config_path: impl AsRef<Path>) -> Result<Self> {
        Self::start_with_tag(config_path, "latest").await
    }

    pub async fn start_with_tag(config_path: impl AsRef<Path>, tag: &str) -> Result<Self> {
        let config_path = config_path.as_ref();

        let mock_server = MockServer::builder()
            .protocol(Protocol::Grpc)
            .host(std::net::IpAddr::from([0, 0, 0, 0]))
            .start()
            .await
            .map_err(|e| Error::Container(format!("failed to start mock server: {e}")))?;

        let mock_port = mock_server.addr().port();

        // Find free ports for the collector (host network shares ports with host)
        let collector_grpc_port = find_free_port()?;
        let collector_http_port = find_free_port()?;

        let config_content = std::fs::read_to_string(config_path)?;
        let config_content = config_content
            .replace("${MOCK_COLLECTOR_PORT}", &mock_port.to_string())
            .replace("${COLLECTOR_GRPC_PORT}", &collector_grpc_port.to_string())
            .replace("${COLLECTOR_HTTP_PORT}", &collector_http_port.to_string());

        let container = GenericImage::new(COLLECTOR_IMAGE, tag)
            .with_wait_for(WaitFor::seconds(5))
            .with_copy_to(CONTAINER_CONFIG_PATH, config_content.into_bytes())
            .with_startup_timeout(Duration::from_secs(30))
            .with_network("host")
            .start()
            .await?;

        let container_id = container.id().to_string();

        Ok(Self {
            mock_server,
            container,
            container_id,
            collector_grpc_port,
            collector_http_port,
        })
    }

    pub fn collector_grpc_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.collector_grpc_port)
    }

    pub fn collector_http_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.collector_http_port)
    }

    pub fn collector_traces_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/v1/traces", self.collector_http_port)
    }

    pub fn collector_grpc_port(&self) -> u16 {
        self.collector_grpc_port
    }

    pub fn collector_http_port(&self) -> u16 {
        self.collector_http_port
    }

    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    pub fn mock_server(&self) -> &ServerHandle {
        &self.mock_server
    }

    pub fn container(&self) -> &ContainerAsync<GenericImage> {
        &self.container
    }

    pub async fn shutdown(self) -> Result<()> {
        self.mock_server
            .shutdown()
            .await
            .map_err(|e| Error::Container(format!("failed to shutdown mock server: {e}")))?;
        Ok(())
    }
}
