//! Ollama Integration
//!
//! Client for interacting with Ollama API to manage models and inference.

use serde::{Deserialize, Serialize};

/// Ollama model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    /// Model name (e.g., "llama3.2:8b")
    pub name: String,
    /// Model size in bytes
    pub size: u64,
    /// Model digest
    pub digest: String,
    /// Modified at timestamp
    pub modified_at: String,
    /// Model family
    pub family: Option<String>,
    /// Parameter count
    pub parameter_size: Option<String>,
    /// Quantization level
    pub quantization_level: Option<String>,
    /// Is currently loaded in memory
    pub is_active: bool,
    /// VRAM usage if loaded
    pub vram_mb: Option<u64>,
}

/// Ollama running model info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaRunningModel {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub digest: String,
    pub expires_at: String,
    pub size_vram: u64,
}

/// Ollama API response for /api/tags
#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelTag>,
}

#[derive(Debug, Deserialize)]
struct ModelTag {
    name: String,
    size: u64,
    digest: String,
    modified_at: String,
    details: Option<ModelDetails>,
}

#[derive(Debug, Deserialize)]
struct ModelDetails {
    family: Option<String>,
    parameter_size: Option<String>,
    quantization_level: Option<String>,
}

/// Ollama API response for /api/ps
#[derive(Debug, Deserialize)]
struct PsResponse {
    models: Vec<RunningModelInfo>,
}

#[derive(Debug, Deserialize)]
struct RunningModelInfo {
    name: String,
    model: String,
    size: u64,
    digest: String,
    expires_at: String,
    size_vram: u64,
}

/// Ollama client for API interactions
pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(base_url: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        })
    }

    /// Check if Ollama is running
    pub async fn is_running(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .is_ok()
    }

    /// Get all available models
    pub async fn get_models(&self) -> Result<Vec<OllamaModel>, String> {
        // Get all models
        let tags_resp = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Failed to get models: {}", e))?;

        let tags: TagsResponse = tags_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse models response: {}", e))?;

        // Get running models
        let running = self.get_running_models().await.unwrap_or_default();
        let running_names: std::collections::HashSet<_> = running.iter()
            .map(|m| m.name.clone())
            .collect();

        let models = tags.models.into_iter().map(|m| {
            let is_active = running_names.contains(&m.name);
            let vram_mb = running.iter()
                .find(|r| r.name == m.name)
                .map(|r| r.size_vram / (1024 * 1024));

            OllamaModel {
                name: m.name,
                size: m.size,
                digest: m.digest,
                modified_at: m.modified_at,
                family: m.details.as_ref().and_then(|d| d.family.clone()),
                parameter_size: m.details.as_ref().and_then(|d| d.parameter_size.clone()),
                quantization_level: m.details.as_ref().and_then(|d| d.quantization_level.clone()),
                is_active,
                vram_mb,
            }
        }).collect();

        Ok(models)
    }

    /// Get currently loaded/running models
    pub async fn get_running_models(&self) -> Result<Vec<OllamaRunningModel>, String> {
        let resp = self.client
            .get(format!("{}/api/ps", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Failed to get running models: {}", e))?;

        let ps: PsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse running models: {}", e))?;

        Ok(ps.models.into_iter().map(|m| OllamaRunningModel {
            name: m.name,
            model: m.model,
            size: m.size,
            digest: m.digest,
            expires_at: m.expires_at,
            size_vram: m.size_vram,
        }).collect())
    }

    /// Unload a model from memory
    pub async fn unload_model(&self, name: &str) -> Result<(), String> {
        // Ollama unloads models by sending a generate request with keep_alive=0
        let body = serde_json::json!({
            "model": name,
            "keep_alive": 0
        });

        self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to unload model: {}", e))?;

        Ok(())
    }

    /// Preload a model into memory
    pub async fn preload_model(&self, name: &str) -> Result<(), String> {
        // Send empty generate request to load model
        let body = serde_json::json!({
            "model": name,
            "prompt": ""
        });

        self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to preload model: {}", e))?;

        Ok(())
    }

    /// Get model information
    pub async fn show_model(&self, name: &str) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "name": name
        });

        let resp = self.client
            .post(format!("{}/api/show", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to show model: {}", e))?;

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse model info: {}", e))
    }

    /// Get total VRAM used by all loaded models
    pub async fn total_vram_usage(&self) -> Result<u64, String> {
        let running = self.get_running_models().await?;
        Ok(running.iter().map(|m| m.size_vram).sum())
    }

    /// Unload all inactive models
    pub async fn unload_inactive(&self) -> Result<usize, String> {
        let models = self.get_models().await?;
        let mut unloaded = 0;

        for model in models {
            if model.is_active {
                // Model is loaded but check if it's actually being used
                // For now, just skip active models
                continue;
            }
        }

        Ok(unloaded)
    }

    /// Get Ollama version
    pub async fn version(&self) -> Result<String, String> {
        let resp = self.client
            .get(format!("{}/api/version", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Failed to get version: {}", e))?;

        #[derive(Deserialize)]
        struct VersionResponse {
            version: String,
        }

        let ver: VersionResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse version: {}", e))?;

        Ok(ver.version)
    }
}

impl std::fmt::Display for OllamaModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size_gb = self.size as f64 / (1024.0 * 1024.0 * 1024.0);
        write!(f, "{}", self.name)?;

        if let Some(ref params) = self.parameter_size {
            write!(f, " ({})", params)?;
        }

        write!(f, " - {:.1} GB", size_gb)?;

        if self.is_active {
            if let Some(vram) = self.vram_mb {
                write!(f, " [LOADED: {} MB VRAM]", vram)?;
            } else {
                write!(f, " [LOADED]")?;
            }
        }

        Ok(())
    }
}
