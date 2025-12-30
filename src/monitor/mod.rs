pub mod memory;

use std::time::Duration;

use crate::container::CollectorTestHarness;
use crate::error::Result;
use crate::input::{LoadConfig, LoadGenerator, LoadStats, TelemetryClient};

pub use memory::{ContainerMonitor, MemoryAnalysis, MemorySnapshot};

pub struct LoadTestHarness {
    harness: CollectorTestHarness,
    monitor: ContainerMonitor,
    telemetry_endpoint: String,
}

impl LoadTestHarness {
    pub async fn new(harness: CollectorTestHarness, telemetry_endpoint: String) -> Result<Self> {
        let monitor = ContainerMonitor::new(harness.container_id()).await?;

        Ok(Self {
            harness,
            monitor,
            telemetry_endpoint,
        })
    }

    pub async fn run_load_test(
        &mut self,
        load_config: LoadConfig,
        monitor_interval: Duration,
    ) -> Result<LoadTestResult> {
        let client = TelemetryClient::new(&self.telemetry_endpoint)?;
        let monitor_duration = load_config.duration;
        let generator = LoadGenerator::new(&client, load_config);

        let (load_result, monitor_result) = tokio::join!(
            generator.run(),
            self.monitor
                .monitor_continuous(monitor_duration, monitor_interval)
        );

        let load_stats = load_result?;
        monitor_result?;
        let memory_analysis = self.monitor.analyse();

        client.shutdown()?;

        Ok(LoadTestResult {
            load_stats,
            memory_analysis,
        })
    }

    pub async fn shutdown(self) -> Result<()> {
        self.harness.shutdown().await
    }
}

#[derive(Debug)]
pub struct LoadTestResult {
    pub load_stats: LoadStats,
    pub memory_analysis: MemoryAnalysis,
}

impl LoadTestResult {
    pub fn has_memory_leak(&self, threshold_bytes_per_sec: f64) -> bool {
        self.memory_analysis
            .has_unbounded_growth(threshold_bytes_per_sec)
    }

    pub fn would_oom_in(&self, limit_bytes: u64, duration: Duration) -> bool {
        self.memory_analysis
            .would_exceed_limit_in(limit_bytes, duration)
    }

    pub fn summary(&self) -> String {
        format!(
            "Load Test Results:\n\
             - Spans sent: {} ({:.2}/s)\n\
             - Metrics sent: {} ({:.2}/s)\n\
             - Duration: {:?}\n\
             - Memory min: {:.2} MB\n\
             - Memory max: {:.2} MB\n\
             - Memory avg: {:.2} MB\n\
             - Growth rate: {:.2} MB/s",
            self.load_stats.spans_sent,
            self.load_stats.spans_per_second(),
            self.load_stats.metrics_sent,
            self.load_stats.metrics_per_second(),
            self.load_stats.duration,
            self.memory_analysis.min_mb(),
            self.memory_analysis.max_mb(),
            self.memory_analysis.avg_mb(),
            self.memory_analysis.growth_rate_mb_per_sec(),
        )
    }
}
