use std::{convert::TryFrom, time::Duration};

use anyhow::{anyhow, Context, Result};
use qdrant_client::{
    qdrant::{
        point_id, CreateCollection, Distance, Filter, PointId, PointStruct, ScoredPoint,
        SearchPoints, UpsertPoints, VectorParams, VectorsConfig,
    },
    Payload, Qdrant,
};
use serde_json::Value;

use crate::config::QdrantConfig;

#[derive(Clone)]
pub struct QdrantManager {
    client: Qdrant,
    collection: String,
    vector_size: u64,
}

impl QdrantManager {
    pub fn is_enabled(config: &QdrantConfig) -> bool {
        config.enabled
    }

    pub async fn new(config: &QdrantConfig) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }

        let mut builder = Qdrant::from_url(&config.uri)
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .skip_compatibility_check();

        if let Some(api_key) = &config.api_key {
            builder = builder.api_key(api_key.clone());
        }

        let client = builder.build().context("failed to create Qdrant client")?;

        let manager = Self {
            client,
            collection: config.collection.clone(),
            vector_size: config.vector_size,
        };

        manager.ensure_collection().await?;

        Ok(Some(manager))
    }

    async fn ensure_collection(&self) -> Result<()> {
        let exists = self
            .client
            .collection_exists(&self.collection)
            .await
            .context("failed to check Qdrant collection")?;

        if exists {
            return Ok(());
        }

        let vectors_config = Some(VectorsConfig {
            config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                VectorParams {
                    size: self.vector_size,
                    distance: Distance::Cosine.into(),
                    ..Default::default()
                },
            )),
            ..Default::default()
        });

        let request = CreateCollection {
            collection_name: self.collection.clone(),
            vectors_config,
            ..Default::default()
        };

        self.client
            .create_collection(request)
            .await
            .context("failed to create Qdrant collection")?;

        Ok(())
    }

    pub async fn upsert_article_vector(
        &self,
        article_id: i64,
        vector: Vec<f32>,
        payload: Option<Value>,
    ) -> Result<()> {
        if vector.len() as u64 != self.vector_size {
            return Err(anyhow!(
                "vector size mismatch; expected {}, got {}",
                self.vector_size,
                vector.len()
            ));
        }

        let payload = match payload {
            Some(raw) => Payload::try_from(raw).context("invalid payload for Qdrant point")?,
            None => Payload::default(),
        };

        let point_id = u64::try_from(article_id)
            .context("article id must be non-negative when stored in Qdrant")?;
        let point = PointStruct::new(point_id, vector, payload);

        let request = UpsertPoints {
            collection_name: self.collection.clone(),
            wait: Some(true),
            points: vec![point],
            ordering: None,
            shard_key_selector: None,
        };

        self.client
            .upsert_points(request)
            .await
            .context("failed to upsert vector into Qdrant")?;

        Ok(())
    }

    pub async fn search_similar(
        &self,
        vector: Vec<f32>,
        limit: u64,
        filter: Option<Filter>,
    ) -> Result<Vec<ScoredPoint>> {
        if vector.len() as u64 != self.vector_size {
            return Err(anyhow!(
                "vector size mismatch; expected {}, got {}",
                self.vector_size,
                vector.len()
            ));
        }

        let request = SearchPoints {
            collection_name: self.collection.clone(),
            vector,
            filter,
            limit,
            with_payload: None,
            params: None,
            score_threshold: None,
            offset: None,
            vector_name: None,
            with_vectors: None,
            read_consistency: None,
            timeout: None,
            shard_key_selector: None,
            sparse_indices: None,
        };

        let response = self
            .client
            .search_points(request)
            .await
            .context("failed to search Qdrant")?;

        Ok(response.result)
    }
}

pub fn point_id_to_i64(id: &PointId) -> Option<i64> {
    match id.point_id_options.as_ref()? {
        point_id::PointIdOptions::Num(value) => i64::try_from(*value).ok(),
        _ => None,
    }
}
