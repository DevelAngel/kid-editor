//! Real [`Embedder`] backed by `sentence-transformers/all-MiniLM-L6-v2`,
//! loaded via `candle-transformers`' BERT implementation. Model and
//! tokenizer are fetched from the Hugging Face Hub on first use and
//! cached locally by `hf-hub`; no network access is needed on
//! subsequent runs once that cache is warm.

use std::fs;

use anyhow::{Context, Result};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::api::sync::Api;
use hf_hub::{Repo, RepoType};
use tokenizers::{PaddingParams, Tokenizer};

use super::Embedder;

const MODEL_ID: &str = "sentence-transformers/all-MiniLM-L6-v2";

pub(in crate::mcp::search) struct MiniLmEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl MiniLmEmbedder {
    /// Downloads (if not already cached) and loads the model and
    /// tokenizer. Requires network access on a cold cache.
    pub(in crate::mcp::search) fn load() -> Result<Self> {
        let repo = Api::new()
            .context("hf-hub api")?
            .repo(Repo::new(MODEL_ID.to_owned(), RepoType::Model));

        let config_path = repo.get("config.json").context("fetch config.json")?;
        let tokenizer_path = repo.get("tokenizer.json").context("fetch tokenizer.json")?;
        let weights_path = repo
            .get("model.safetensors")
            .context("fetch model.safetensors")?;

        let config: Config =
            serde_json::from_str(&fs::read_to_string(config_path).context("read config.json")?)
                .context("parse config.json")?;

        let mut tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|err| anyhow::anyhow!("load tokenizer: {err}"))?;
        tokenizer.with_padding(Some(PaddingParams::default()));

        let device = Device::Cpu;
        let tensors =
            candle_core::safetensors::load(&weights_path, &device).context("load model weights")?;
        let vb = VarBuilder::from_tensors(tensors, DTYPE, &device);
        let model = BertModel::load(vb, &config).context("build BertModel")?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn try_embed(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|err| anyhow::anyhow!("tokenize: {err}"))?;

        let ids = Tensor::new(encoding.get_ids(), &self.device)?.unsqueeze(0)?;
        let type_ids = Tensor::new(encoding.get_type_ids(), &self.device)?.unsqueeze(0)?;
        let attention_mask =
            Tensor::new(encoding.get_attention_mask(), &self.device)?.unsqueeze(0)?;

        let hidden_states = self.model.forward(&ids, &type_ids, Some(&attention_mask))?;

        // Mean-pool over tokens, weighted by the attention mask, then
        // L2-normalize — the standard sentence-transformers recipe, so
        // cosine similarity between pooled vectors is meaningful.
        let mask = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?;
        let masked = hidden_states.broadcast_mul(&mask)?;
        let summed = masked.sum(1)?;
        let counts = mask.sum(1)?;
        let pooled = summed.broadcast_div(&counts)?;
        let norm = pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = pooled.broadcast_div(&norm)?;

        Ok(normalized.squeeze(0)?.to_vec1::<f32>()?)
    }
}

impl Embedder for MiniLmEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.try_embed(text).unwrap_or_else(|err| {
            tracing::warn!("semantic embedding failed, treating as zero vector: {err}");
            Vec::new()
        })
    }
}

/// True once per process: `MiniLmEmbedder::load()` touches the network
/// on a cold `hf-hub` cache, which unit tests must not depend on. Run
/// manually with `cargo test -- --ignored` after warming the cache.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads a ~90MB model on a cold hf-hub cache"]
    fn embeds_similar_sentences_closer_than_unrelated_ones() {
        let embedder = MiniLmEmbedder::load().unwrap();
        let a = embedder.embed("The cat sits on the mat.");
        let b = embedder.embed("A feline rests on the rug.");
        let c = embedder.embed("Quarterly revenue exceeded expectations.");

        let sim_ab = innr::cosine(&a, &b);
        let sim_ac = innr::cosine(&a, &c);
        assert!(
            sim_ab > sim_ac,
            "expected related sentences to score higher: {sim_ab} vs {sim_ac}"
        );
    }
}
