//! Text embedding generation for semantic search.
//!
//! Uses Candle ML to run sentence-transformers models locally.
//! Default model: `sentence-transformers/all-MiniLM-L6-v2` (384 dimensions).

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::path::PathBuf;
use std::sync::Arc;
use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

/// Error type for embedding operations.
#[derive(Debug)]
pub enum EmbedError {
    /// Model loading failed.
    ModelLoad(String),
    /// Tokenization failed.
    Tokenize(String),
    /// Inference failed.
    Inference(String),
    /// HuggingFace hub error.
    Hub(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::ModelLoad(e) => write!(f, "Model load error: {}", e),
            EmbedError::Tokenize(e) => write!(f, "Tokenization error: {}", e),
            EmbedError::Inference(e) => write!(f, "Inference error: {}", e),
            EmbedError::Hub(e) => write!(f, "HuggingFace hub error: {}", e),
        }
    }
}

impl std::error::Error for EmbedError {}

impl From<candle_core::Error> for EmbedError {
    fn from(e: candle_core::Error) -> Self {
        EmbedError::Inference(e.to_string())
    }
}

impl From<hf_hub::api::sync::ApiError> for EmbedError {
    fn from(e: hf_hub::api::sync::ApiError) -> Self {
        EmbedError::Hub(e.to_string())
    }
}

impl From<tokenizers::Error> for EmbedError {
    fn from(e: tokenizers::Error) -> Self {
        EmbedError::Tokenize(e.to_string())
    }
}

/// Trait for generating text embeddings.
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding vector for text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    /// Embedding dimension (e.g., 384 for MiniLM).
    fn dimension(&self) -> usize;
}

/// Local embedding using Candle ML with sentence-transformers models.
///
/// Uses `all-MiniLM-L6-v2` by default, which produces 384-dimensional embeddings.
/// Model is auto-downloaded from HuggingFace on first use.
pub struct CandleEmbedding {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    normalize: bool,
}

impl CandleEmbedding {
    /// Default model ID for sentence embeddings.
    pub const DEFAULT_MODEL: &'static str = "sentence-transformers/all-MiniLM-L6-v2";

    /// Load embedding model from HuggingFace.
    ///
    /// Model is cached at `~/.cache/huggingface/hub/`.
    /// First call downloads ~22MB, subsequent calls use cache.
    pub fn load(model_id: &str) -> Result<Self, EmbedError> {
        let device = Device::Cpu;

        // Download model files from HuggingFace
        let api = Api::new()?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        let config_path = repo
            .get("config.json")
            .map_err(|e| EmbedError::Hub(format!("Failed to get config.json: {}", e)))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| EmbedError::Hub(format!("Failed to get tokenizer.json: {}", e)))?;
        let weights_path = repo
            .get("model.safetensors")
            .map_err(|e| EmbedError::Hub(format!("Failed to get model.safetensors: {}", e)))?;

        Self::load_from_files(&config_path, &tokenizer_path, &weights_path, device)
    }

    /// Load from local files (for testing or custom models).
    pub fn load_from_files(
        config_path: &PathBuf,
        tokenizer_path: &PathBuf,
        weights_path: &PathBuf,
        device: Device,
    ) -> Result<Self, EmbedError> {
        // Load config
        let config_str = std::fs::read_to_string(config_path)
            .map_err(|e| EmbedError::ModelLoad(format!("Failed to read config: {}", e)))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| EmbedError::ModelLoad(format!("Failed to parse config: {}", e)))?;

        // Load tokenizer
        let mut tokenizer = Tokenizer::from_file(tokenizer_path)?;

        // Configure tokenizer for batch processing
        let padding = PaddingParams {
            strategy: tokenizers::PaddingStrategy::BatchLongest,
            ..Default::default()
        };
        tokenizer.with_padding(Some(padding));

        let truncation = TruncationParams {
            max_length: 512,
            ..Default::default()
        };
        tokenizer
            .with_truncation(Some(truncation))
            .map_err(|e| EmbedError::Tokenize(e.to_string()))?;

        // Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .map_err(|e| EmbedError::ModelLoad(format!("Failed to load weights: {}", e)))?
        };

        let model = BertModel::load(vb, &config)
            .map_err(|e| EmbedError::ModelLoad(format!("Failed to build model: {}", e)))?;

        Ok(Self {
            model,
            tokenizer,
            device,
            normalize: true,
        })
    }

    /// Create with default MiniLM model.
    pub fn new() -> Result<Self, EmbedError> {
        Self::load(Self::DEFAULT_MODEL)
    }

    /// Mean pooling over token embeddings.
    fn mean_pooling(
        &self,
        embeddings: &Tensor,
        attention_mask: &Tensor,
    ) -> Result<Tensor, EmbedError> {
        // Expand attention mask to match embedding dimensions
        let (_batch, seq_len, hidden) = embeddings.dims3()?;
        let mask = attention_mask
            .unsqueeze(2)?
            .expand((attention_mask.dim(0)?, seq_len, hidden))?
            .to_dtype(embeddings.dtype())?;

        // Masked sum
        let masked = embeddings.mul(&mask)?;
        let sum = masked.sum(1)?;

        // Divide by sum of mask (number of real tokens)
        let mask_sum = mask.sum(1)?.clamp(1e-9, f64::MAX)?;
        let pooled = sum.div(&mask_sum)?;

        Ok(pooled)
    }

    /// L2 normalize embeddings.
    fn normalize_l2(&self, embeddings: &Tensor) -> Result<Tensor, EmbedError> {
        let norm = embeddings.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = embeddings.broadcast_div(&norm)?;
        Ok(normalized)
    }
}

impl EmbeddingProvider for CandleEmbedding {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        // Tokenize
        let encoding = self.tokenizer.encode(text, true)?;

        let input_ids = Tensor::new(encoding.get_ids(), &self.device)?.unsqueeze(0)?;
        let attention_mask =
            Tensor::new(encoding.get_attention_mask(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = input_ids.zeros_like()?;

        // Forward pass
        let embeddings = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))?;

        // Mean pooling
        let pooled = self.mean_pooling(&embeddings, &attention_mask)?;

        // Normalize if enabled
        let result = if self.normalize {
            self.normalize_l2(&pooled)?
        } else {
            pooled
        };

        // Convert to Vec<f32>
        let result = result.squeeze(0)?;
        let vec: Vec<f32> = result.to_vec1()?;

        Ok(vec)
    }

    fn dimension(&self) -> usize {
        384 // MiniLM-L6-v2
    }
}

/// Thread-safe wrapper around an embedding provider.
pub struct SharedEmbedding {
    inner: Arc<dyn EmbeddingProvider>,
}

impl SharedEmbedding {
    pub fn new<T: EmbeddingProvider + 'static>(provider: T) -> Self {
        Self {
            inner: Arc::new(provider),
        }
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.inner.embed(text)
    }

    pub fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

impl Clone for SharedEmbedding {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

// ============================================================================
// EmbeddingService - Background embedding with queue
// ============================================================================

use bevy::prelude::Resource;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, RwLock};
use std::thread::{self, JoinHandle};

use super::{AssetKey, AssetMetadata, BlobStore};

/// Request to embed an asset.
#[derive(Debug)]
pub struct EmbedRequest {
    pub key: AssetKey,
    pub text: String,
}

impl EmbedRequest {
    /// Create embed request from asset metadata.
    pub fn from_metadata(key: AssetKey, metadata: &AssetMetadata) -> Self {
        // Combine name, description, tags for embedding
        let text = format!(
            "{} {} {}",
            metadata.name,
            metadata.description.as_deref().unwrap_or(""),
            metadata.tags.join(" ")
        );
        Self { key, text }
    }
}

/// Lazy-loaded embedding model.
///
/// The model is loaded on first use and then cached for subsequent calls.
/// Thread-safe via internal locking.
pub struct LazyEmbedding {
    model: RwLock<Option<CandleEmbedding>>,
    loading: Mutex<bool>,
}

impl LazyEmbedding {
    /// Create a new lazy embedding (model not loaded yet).
    pub fn new() -> Self {
        Self {
            model: RwLock::new(None),
            loading: Mutex::new(false),
        }
    }

    /// Get or load the embedding model.
    fn get_or_load(&self) -> Result<(), EmbedError> {
        // Check if already loaded
        if self.model.read().unwrap().is_some() {
            return Ok(());
        }

        // Try to acquire loading lock
        let mut loading = self.loading.lock().unwrap();
        if *loading {
            // Another thread is loading, wait for it
            drop(loading);
            while self.model.read().unwrap().is_none() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            return Ok(());
        }

        // Double-check after acquiring lock
        if self.model.read().unwrap().is_some() {
            return Ok(());
        }

        *loading = true;
        drop(loading);

        bevy::log::info!("Loading embedding model...");
        let model = CandleEmbedding::new()?;
        bevy::log::info!("Embedding model loaded successfully");

        *self.model.write().unwrap() = Some(model);
        *self.loading.lock().unwrap() = false;
        Ok(())
    }

    /// Embed text using the model.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        self.get_or_load()?;
        let guard = self.model.read().unwrap();
        guard.as_ref().unwrap().embed(text)
    }

    /// Get embedding dimension.
    pub fn dimension(&self) -> usize {
        384 // MiniLM-L6-v2
    }

    /// Check if model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.model.read().unwrap().is_some()
    }
}

impl Default for LazyEmbedding {
    fn default() -> Self {
        Self::new()
    }
}

/// Background service for generating and storing embeddings.
///
/// Runs embedding model in a separate thread to avoid blocking the main loop.
/// Assets are queued for embedding and processed asynchronously.
///
/// Also provides synchronous embedding for search queries via `embed_query()`.
#[derive(Resource, Clone)]
pub struct EmbeddingService {
    tx: Sender<EmbedRequest>,
    _handle: Arc<JoinHandle<()>>,
    /// Shared embedding model for synchronous query embedding
    model: Arc<LazyEmbedding>,
}

impl EmbeddingService {
    /// Create a new embedding service with background worker.
    ///
    /// The worker thread loads the model on first request (lazy loading).
    /// Accepts any `BlobStore` implementation that supports embeddings.
    pub fn new(store: Arc<dyn BlobStore>) -> Self {
        let (tx, rx) = mpsc::channel::<EmbedRequest>();
        let model = Arc::new(LazyEmbedding::new());
        let worker_model = Arc::clone(&model);

        let handle = thread::spawn(move || {
            Self::worker_loop(rx, store, worker_model);
        });

        Self {
            tx,
            _handle: Arc::new(handle),
            model,
        }
    }

    /// Queue an asset for embedding.
    ///
    /// Non-blocking. The embedding will be generated in the background.
    pub fn queue(&self, request: EmbedRequest) {
        if let Err(e) = self.tx.send(request) {
            bevy::log::error!("Failed to queue embedding request: {}", e);
        }
    }

    /// Queue embedding for an asset using its metadata.
    pub fn queue_asset(&self, key: AssetKey, metadata: &AssetMetadata) {
        self.queue(EmbedRequest::from_metadata(key, metadata));
    }

    /// Embed a search query synchronously.
    ///
    /// This may block on first call while loading the model.
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbedError> {
        self.model.embed(query)
    }

    /// Get embedding dimension.
    pub fn dimension(&self) -> usize {
        self.model.dimension()
    }

    /// Background worker loop.
    fn worker_loop(
        rx: Receiver<EmbedRequest>,
        store: Arc<dyn BlobStore>,
        model: Arc<LazyEmbedding>,
    ) {
        for request in rx {
            // Generate embedding
            match model.embed(&request.text) {
                Ok(vec) => {
                    // Store in database (via BlobStore trait)
                    if let Err(e) = store.set_embedding(&request.key, &vec) {
                        bevy::log::error!("Failed to store embedding for {}: {}", request.key, e);
                    } else {
                        bevy::log::debug!("Embedded asset: {}", request.key);
                    }
                }
                Err(e) => {
                    bevy::log::error!("Failed to embed {}: {}", request.key, e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download, run with: cargo test -p studio_core embedding -- --ignored
    fn test_candle_embedding_loads() {
        let embedding = CandleEmbedding::new().expect("Failed to load model");
        assert_eq!(embedding.dimension(), 384);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_text() {
        let embedding = CandleEmbedding::new().expect("Failed to load model");
        let vec = embedding.embed("Hello world").expect("Failed to embed");
        assert_eq!(vec.len(), 384);

        // Check it's normalized (L2 norm should be ~1.0)
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "Expected normalized vector, got norm={}",
            norm
        );
    }

    #[test]
    #[ignore] // Requires model download
    fn test_similar_texts_have_high_similarity() {
        let embedding = CandleEmbedding::new().expect("Failed to load model");

        let v1 = embedding
            .embed("glowing blue crystal")
            .expect("Failed to embed");
        let v2 = embedding
            .embed("shiny blue gemstone")
            .expect("Failed to embed");
        let v3 = embedding.embed("hot molten lava").expect("Failed to embed");

        // Cosine similarity (for normalized vectors, it's just dot product)
        let sim_12: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let sim_13: f32 = v1.iter().zip(v3.iter()).map(|(a, b)| a * b).sum();

        // Crystal and gemstone should be more similar than crystal and lava
        assert!(
            sim_12 > sim_13,
            "Expected crystal/gemstone ({}) > crystal/lava ({})",
            sim_12,
            sim_13
        );
        assert!(
            sim_12 > 0.5,
            "Expected high similarity for crystal/gemstone, got {}",
            sim_12
        );
    }
}
