use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mock_collector::{MockServer, Protocol, ServerHandle};
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::error::{Error, Result};

const CONTAINER_CONFIG_PATH: &str = "/etc/otelcol-contrib/config.yaml";
const COLLECTOR_IMAGE: &str = "otel/opentelemetry-collector-contrib";
const DEFAULT_MOCK_HOST: &str = "127.0.0.1";

pub fn find_free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(Error::PortAllocation)?;
    Ok(listener.local_addr()?.port())
}

pub struct CollectorTestHarnessBuilder {
    config_path: PathBuf,
    exporter_endpoint_var: String,
    mock_host: String,
    tag: String,
    env_vars: HashMap<String, String>,
}

impl CollectorTestHarnessBuilder {
    pub fn new(config_path: impl AsRef<Path>, exporter_endpoint_var: impl Into<String>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            exporter_endpoint_var: exporter_endpoint_var.into(),
            mock_host: DEFAULT_MOCK_HOST.to_string(),
            tag: "latest".to_string(),
            env_vars: HashMap::new(),
        }
    }

    #[must_use]
    pub fn mock_host(mut self, host: impl Into<String>) -> Self {
        self.mock_host = host.into();
        self
    }

    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    #[must_use]
    pub fn env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    pub async fn start(self) -> Result<CollectorTestHarness> {
        let mock_server = MockServer::builder()
            .protocol(Protocol::Grpc)
            .host(std::net::IpAddr::from([0, 0, 0, 0]))
            .start()
            .await
            .map_err(|e| Error::MockServerStart(e.to_string()))?;

        let mock_port = mock_server.addr().port();
        let mock_endpoint = format!("{}:{}", self.mock_host, mock_port);

        let config_content = std::fs::read_to_string(&self.config_path)?;

        let mut container = GenericImage::new(COLLECTOR_IMAGE, &self.tag)
            .with_wait_for(WaitFor::seconds(5))
            .with_copy_to(CONTAINER_CONFIG_PATH, config_content.into_bytes())
            .with_env_var(&self.exporter_endpoint_var, &mock_endpoint)
            .with_startup_timeout(Duration::from_secs(30))
            .with_network("host");

        for (key, value) in &self.env_vars {
            container = container.with_env_var(key, value);
        }

        let container = container.start().await?;
        let container_id = container.id().to_string();

        Ok(CollectorTestHarness {
            mock_server,
            container,
            container_id,
            mock_host: self.mock_host,
        })
    }
}

pub struct CollectorTestHarness {
    mock_server: ServerHandle,
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    container_id: String,
    mock_host: String,
}

impl CollectorTestHarness {
    pub fn builder(
        config_path: impl AsRef<Path>,
        exporter_endpoint_var: impl Into<String>,
    ) -> CollectorTestHarnessBuilder {
        CollectorTestHarnessBuilder::new(config_path, exporter_endpoint_var)
    }

    pub fn mock_host(&self) -> &str {
        &self.mock_host
    }

    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    pub fn mock_server(&self) -> &ServerHandle {
        &self.mock_server
    }

    pub async fn shutdown(self) -> Result<()> {
        self.mock_server
            .shutdown()
            .await
            .map_err(|e| Error::MockServerShutdown(e.to_string()))?;
        Ok(())
    }
}
