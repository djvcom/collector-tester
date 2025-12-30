#![allow(dead_code)]

use std::path::PathBuf;

use collector_tester::container::{CollectorTestHarness, CollectorTestHarnessBuilder, find_free_port};

pub fn config_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("common")
        .join("configs")
        .join(name)
}

pub struct TestPorts {
    pub grpc: u16,
    pub http: u16,
}

impl TestPorts {
    pub fn allocate() -> Self {
        Self {
            grpc: find_free_port().expect("failed to find free grpc port"),
            http: find_free_port().expect("failed to find free http port"),
        }
    }

    pub fn http_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.http)
    }

    pub fn http_traces_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/v1/traces", self.http)
    }
}

pub fn harness_with_ports(config_name: &str) -> (CollectorTestHarnessBuilder, TestPorts) {
    let ports = TestPorts::allocate();
    let builder = CollectorTestHarness::builder(config_path(config_name), "OTLP_EXPORTER_ENDPOINT")
        .env_var("COLLECTOR_GRPC_PORT", ports.grpc.to_string())
        .env_var("COLLECTOR_HTTP_PORT", ports.http.to_string());
    (builder, ports)
}
