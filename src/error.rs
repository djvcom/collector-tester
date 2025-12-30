use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("container error: {0}")]
    Container(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("telemetry input error: {0}")]
    Input(String),

    #[error("docker API error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("timeout waiting for condition: {0}")]
    Timeout(String),

    #[error("testcontainers error: {0}")]
    TestContainers(#[from] testcontainers::TestcontainersError),

    #[error("opentelemetry error: {0}")]
    Otel(String),
}

pub type Result<T> = std::result::Result<T, Error>;
