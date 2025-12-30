use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Other(String),

    #[error("docker API error: {0}")]
    Docker(#[from] bollard::errors::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("testcontainers error: {0}")]
    TestContainers(#[from] testcontainers::TestcontainersError),
}

pub type Result<T> = std::result::Result<T, Error>;
