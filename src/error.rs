use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to build {signal} exporter: {message}")]
    ExporterBuild { signal: Signal, message: String },

    #[error("failed to flush {signal}: {message}")]
    Flush { signal: Signal, message: String },

    #[error("failed to shutdown {signal}: {message}")]
    Shutdown { signal: Signal, message: String },

    #[error("failed to find free port: {0}")]
    PortAllocation(#[source] std::io::Error),

    #[error("failed to start mock server: {0}")]
    MockServerStart(String),

    #[error("failed to shutdown mock server: {0}")]
    MockServerShutdown(String),

    #[error("no stats received from container")]
    NoContainerStats,

    #[error("docker API error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("testcontainers error: {0}")]
    TestContainers(#[from] testcontainers::TestcontainersError),
}

#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Traces,
    Metrics,
    Logs,
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Signal::Traces => write!(f, "traces"),
            Signal::Metrics => write!(f, "metrics"),
            Signal::Logs => write!(f, "logs"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
