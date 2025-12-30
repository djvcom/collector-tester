use std::time::{Duration, Instant};

use bollard::Docker;
use bollard::query_parameters::StatsOptionsBuilder;
use futures_util::StreamExt;
use tokio::time::interval;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub timestamp: Instant,
    pub usage_bytes: u64,
    pub max_usage_bytes: u64,
    pub limit_bytes: Option<u64>,
}

impl MemorySnapshot {
    pub fn usage_mb(&self) -> f64 {
        self.usage_bytes as f64 / 1_000_000.0
    }

    pub fn max_usage_mb(&self) -> f64 {
        self.max_usage_bytes as f64 / 1_000_000.0
    }

    pub fn limit_mb(&self) -> Option<f64> {
        self.limit_bytes.map(|l| l as f64 / 1_000_000.0)
    }
}

pub struct ContainerMonitor {
    docker: Docker,
    container_id: String,
    samples: Vec<MemorySnapshot>,
}

impl ContainerMonitor {
    pub async fn new(container_id: &str) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self {
            docker,
            container_id: container_id.to_string(),
            samples: Vec::new(),
        })
    }

    pub async fn sample(&mut self) -> Result<MemorySnapshot> {
        let options = StatsOptionsBuilder::default()
            .stream(false)
            .one_shot(true)
            .build();

        let mut stream = self.docker.stats(&self.container_id, Some(options));

        if let Some(result) = stream.next().await {
            let stats = result?;

            let memory_stats = stats.memory_stats.unwrap_or_default();
            let snapshot = MemorySnapshot {
                timestamp: Instant::now(),
                usage_bytes: memory_stats.usage.unwrap_or(0),
                max_usage_bytes: memory_stats.max_usage.unwrap_or(0),
                limit_bytes: memory_stats.limit,
            };

            self.samples.push(snapshot.clone());
            Ok(snapshot)
        } else {
            Err(Error::Timeout(
                "no stats received from container".to_string(),
            ))
        }
    }

    pub async fn monitor_continuous(
        &mut self,
        duration: Duration,
        sample_interval: Duration,
    ) -> Result<Vec<MemorySnapshot>> {
        let start = Instant::now();
        let mut ticker = interval(sample_interval);

        while start.elapsed() < duration {
            ticker.tick().await;
            self.sample().await?;
        }

        Ok(self.samples.clone())
    }

    pub fn samples(&self) -> &[MemorySnapshot] {
        &self.samples
    }

    pub fn clear_samples(&mut self) {
        self.samples.clear();
    }

    pub fn analyse(&self) -> MemoryAnalysis {
        if self.samples.is_empty() {
            return MemoryAnalysis::default();
        }

        let usages: Vec<u64> = self.samples.iter().map(|s| s.usage_bytes).collect();

        MemoryAnalysis {
            min_bytes: usages.iter().copied().min().unwrap_or(0),
            max_bytes: usages.iter().copied().max().unwrap_or(0),
            avg_bytes: usages.iter().sum::<u64>() / usages.len() as u64,
            sample_count: self.samples.len(),
            growth_rate_bytes_per_sec: self.calculate_growth_rate(),
        }
    }

    fn calculate_growth_rate(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }

        let first = &self.samples[0];
        let last = &self.samples[self.samples.len() - 1];
        let duration_secs = last.timestamp.duration_since(first.timestamp).as_secs_f64();

        if duration_secs > 0.0 {
            (last.usage_bytes as f64 - first.usage_bytes as f64) / duration_secs
        } else {
            0.0
        }
    }
}

#[derive(Debug, Default)]
pub struct MemoryAnalysis {
    pub min_bytes: u64,
    pub max_bytes: u64,
    pub avg_bytes: u64,
    pub sample_count: usize,
    pub growth_rate_bytes_per_sec: f64,
}

impl MemoryAnalysis {
    pub fn min_mb(&self) -> f64 {
        self.min_bytes as f64 / 1_000_000.0
    }

    pub fn max_mb(&self) -> f64 {
        self.max_bytes as f64 / 1_000_000.0
    }

    pub fn avg_mb(&self) -> f64 {
        self.avg_bytes as f64 / 1_000_000.0
    }

    pub fn growth_rate_mb_per_sec(&self) -> f64 {
        self.growth_rate_bytes_per_sec / 1_000_000.0
    }

    pub fn has_unbounded_growth(&self, threshold_bytes_per_sec: f64) -> bool {
        self.growth_rate_bytes_per_sec > threshold_bytes_per_sec
    }

    pub fn would_exceed_limit_in(&self, limit_bytes: u64, duration: Duration) -> bool {
        if self.growth_rate_bytes_per_sec <= 0.0 {
            return false;
        }

        let projected_growth = self.growth_rate_bytes_per_sec * duration.as_secs_f64();
        self.max_bytes as f64 + projected_growth > limit_bytes as f64
    }
}
