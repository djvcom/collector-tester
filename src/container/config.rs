use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorConfig {
    #[serde(default)]
    pub receivers: Value,
    #[serde(default)]
    pub processors: Value,
    #[serde(default)]
    pub exporters: Value,
    #[serde(default)]
    pub extensions: Value,
    pub service: Value,
}

impl CollectorConfig {
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        Self::from_yaml_str(&content)
    }

    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml).map_err(|e| Error::Config(e.to_string()))
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|e| Error::Config(e.to_string()))
    }

    pub fn builder() -> CollectorConfigBuilder {
        CollectorConfigBuilder::new()
    }

    pub fn with_mock_exporter_endpoint(mut self, endpoint: &str) -> Result<Self> {
        let exporters = self
            .exporters
            .as_mapping_mut()
            .ok_or_else(|| Error::Config("exporters must be a mapping".to_string()))?;

        let otlp_config = serde_yaml::to_value(OtlpExporterConfig {
            endpoint: endpoint.to_string(),
            tls: TlsConfig { insecure: true },
        })
        .map_err(|e| Error::Config(e.to_string()))?;

        exporters.insert(Value::String("otlp/mock".to_string()), otlp_config);

        self.update_pipelines_exporter("otlp/mock")?;

        Ok(self)
    }

    fn update_pipelines_exporter(&mut self, exporter_name: &str) -> Result<()> {
        let service = self
            .service
            .as_mapping_mut()
            .ok_or_else(|| Error::Config("service must be a mapping".to_string()))?;

        let pipelines = service
            .get_mut(Value::String("pipelines".to_string()))
            .and_then(|p| p.as_mapping_mut())
            .ok_or_else(|| Error::Config("service.pipelines must be a mapping".to_string()))?;

        for (_pipeline_name, pipeline_config) in pipelines.iter_mut() {
            if let Some(mapping) = pipeline_config.as_mapping_mut() {
                let exporters_key = Value::String("exporters".to_string());
                let exporter_value = Value::String(exporter_name.to_string());

                if let Some(exporters) = mapping.get_mut(&exporters_key) {
                    if let Some(seq) = exporters.as_sequence_mut()
                        && !seq.contains(&exporter_value)
                    {
                        seq.push(exporter_value);
                    }
                } else {
                    mapping.insert(exporters_key, Value::Sequence(vec![exporter_value]));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OtlpExporterConfig {
    endpoint: String,
    tls: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TlsConfig {
    insecure: bool,
}

#[derive(Debug, Default)]
pub struct CollectorConfigBuilder {
    otlp_grpc_receiver: bool,
    otlp_http_receiver: bool,
    processors: Vec<(String, Value)>,
    debug_exporter: bool,
}

impl CollectorConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_otlp_grpc_receiver(mut self) -> Self {
        self.otlp_grpc_receiver = true;
        self
    }

    pub fn with_otlp_http_receiver(mut self) -> Self {
        self.otlp_http_receiver = true;
        self
    }

    pub fn with_processor(mut self, name: &str, config: Value) -> Self {
        self.processors.push((name.to_string(), config));
        self
    }

    pub fn with_debug_exporter(mut self) -> Self {
        self.debug_exporter = true;
        self
    }

    pub fn build(self) -> CollectorConfig {
        let mut receivers = serde_yaml::Mapping::new();
        let mut exporters = serde_yaml::Mapping::new();
        let mut processors = serde_yaml::Mapping::new();

        if self.otlp_grpc_receiver || self.otlp_http_receiver {
            let mut otlp_config = serde_yaml::Mapping::new();
            let mut protocols = serde_yaml::Mapping::new();

            if self.otlp_grpc_receiver {
                let mut grpc = serde_yaml::Mapping::new();
                grpc.insert(
                    Value::String("endpoint".to_string()),
                    Value::String("0.0.0.0:4317".to_string()),
                );
                protocols.insert(Value::String("grpc".to_string()), Value::Mapping(grpc));
            }

            if self.otlp_http_receiver {
                let mut http = serde_yaml::Mapping::new();
                http.insert(
                    Value::String("endpoint".to_string()),
                    Value::String("0.0.0.0:4318".to_string()),
                );
                protocols.insert(Value::String("http".to_string()), Value::Mapping(http));
            }

            otlp_config.insert(
                Value::String("protocols".to_string()),
                Value::Mapping(protocols),
            );
            receivers.insert(
                Value::String("otlp".to_string()),
                Value::Mapping(otlp_config),
            );
        }

        for (name, config) in &self.processors {
            processors.insert(Value::String(name.clone()), config.clone());
        }

        if self.debug_exporter {
            let mut debug_config = serde_yaml::Mapping::new();
            debug_config.insert(
                Value::String("verbosity".to_string()),
                Value::String("detailed".to_string()),
            );
            exporters.insert(
                Value::String("debug".to_string()),
                Value::Mapping(debug_config),
            );
        }

        let mut service = serde_yaml::Mapping::new();
        let mut pipelines = serde_yaml::Mapping::new();

        let receiver_list: Vec<Value> = if self.otlp_grpc_receiver || self.otlp_http_receiver {
            vec![Value::String("otlp".to_string())]
        } else {
            vec![]
        };

        let processor_list: Vec<Value> = self
            .processors
            .iter()
            .map(|(name, _)| Value::String(name.clone()))
            .collect();

        let exporter_list: Vec<Value> = if self.debug_exporter {
            vec![Value::String("debug".to_string())]
        } else {
            vec![]
        };

        for signal in ["traces", "metrics", "logs"] {
            let mut pipeline = serde_yaml::Mapping::new();
            pipeline.insert(
                Value::String("receivers".to_string()),
                Value::Sequence(receiver_list.clone()),
            );
            pipeline.insert(
                Value::String("processors".to_string()),
                Value::Sequence(processor_list.clone()),
            );
            pipeline.insert(
                Value::String("exporters".to_string()),
                Value::Sequence(exporter_list.clone()),
            );
            pipelines.insert(Value::String(signal.to_string()), Value::Mapping(pipeline));
        }

        service.insert(
            Value::String("pipelines".to_string()),
            Value::Mapping(pipelines),
        );

        CollectorConfig {
            receivers: Value::Mapping(receivers),
            processors: Value::Mapping(processors),
            exporters: Value::Mapping(exporters),
            extensions: Value::Mapping(serde_yaml::Mapping::new()),
            service: Value::Mapping(service),
        }
    }
}
