//! Qdrant helpers for building points and managing collections.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, CreateVectorNameRequestBuilder,
    DenseVectorCreationConfigBuilder, Distance, FieldType, PointStruct, VectorParamsBuilder,
    VectorsConfigBuilder,
};
use qdrant_client::{Payload, Qdrant};
use serde_json::json;

/// Default Qdrant collection name.
pub const DEFAULT_COLLECTION: &str = "project_memory";

/// Builds a Qdrant `PointStruct` from a project info
/// record with a named embedding vector.
#[allow(clippy::too_many_arguments)]
pub fn build_point(
    id: &str,
    project: &str,
    content: &str,
    category: &str,
    hash: &str,
    ts: i64,
    vector_name: &str,
    vector: Vec<f32>,
) -> PointStruct {
    let mut vectors = HashMap::new();
    vectors.insert(vector_name.to_string(), vector);
    PointStruct::new(
        uuid::Uuid::new_v4().to_string(),
        vectors,
        Payload::try_from(json!({
            "id": id,
            "project": project,
            "content": content,
            "category": category,
            "file_hash": hash,
            "timestamp": ts,
        }))
        .unwrap_or_default(),
    )
}

/// Builds the payload filter for project/category filtering.
///
/// Returns `None` when no filtering is needed (project == "*").
pub fn build_filter(
    project: &str,
    category: Option<&str>,
) -> Option<qdrant_client::qdrant::Filter> {
    use qdrant_client::qdrant::Condition;

    if project == "*" {
        return None;
    }

    let mut conditions = vec![Condition::matches("project", project.to_owned())];

    if let Some(cat) = category {
        conditions.push(Condition::matches("category", cat.to_owned()));
    }

    Some(qdrant_client::qdrant::Filter::must(conditions))
}

/// Creates a Qdrant collection with named vector
/// configuration (one vector per embedding model).
/// If the collection exists but the vector name is missing, adds it.
pub async fn ensure_collection(
    client: &Qdrant,
    collection_name: &str,
    vector_name: &str,
    dimension: usize,
) -> anyhow::Result<()> {
    // Check if collection already exists
    let collection_exists = client.collection_exists(collection_name).await?;

    if !collection_exists {
        let mut vectors_config = VectorsConfigBuilder::default();
        vectors_config.add_named_vector_params(
            vector_name,
            VectorParamsBuilder::new(dimension as u64, Distance::Cosine),
        );
        client
            .create_collection(
                CreateCollectionBuilder::new(collection_name).vectors_config(vectors_config),
            )
            .await?;

        // Create payload indexes to speed up filtering
        for field in ["id", "project", "category"] {
            client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    collection_name,
                    field,
                    FieldType::Keyword,
                ))
                .await?;
        }
    } else {
        // Collection exists — check if the required vector name is present
        let info = client.collection_info(collection_name).await?;
        let has_vector = info
            .result
            .as_ref()
            .and_then(|c| c.config.as_ref())
            .and_then(|c| c.params.as_ref())
            .and_then(|p| p.vectors_config.as_ref())
            .and_then(|v| v.config.as_ref())
            .map(|config| match config {
                qdrant_client::qdrant::vectors_config::Config::ParamsMap(map) => {
                    map.map.contains_key(vector_name)
                }
                qdrant_client::qdrant::vectors_config::Config::Params(_params) => {
                    // Single unnamed vector — check if it matches
                    false
                }
            })
            .unwrap_or(false);

        if !has_vector {
            client
                .create_vector_name(CreateVectorNameRequestBuilder::new(
                    collection_name,
                    vector_name,
                    DenseVectorCreationConfigBuilder::new(dimension as u64, Distance::Cosine),
                ))
                .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_filter_with_project_only() {
        let filter = build_filter("my-proj", None).expect("filter");
        assert_eq!(filter.must.len(), 1);
    }

    #[test]
    fn builds_filter_with_project_and_category() {
        let filter = build_filter("my-proj", Some("file")).expect("filter");
        assert_eq!(filter.must.len(), 2);
    }

    #[test]
    fn returns_none_for_wildcard_project() {
        let filter = build_filter("*", None);
        assert!(filter.is_none());
    }

    #[test]
    fn builds_point_with_all_fields() {
        let point = build_point(
            "src/main.rs",
            "my-proj",
            "main entry point",
            "file",
            "abc123",
            1234567890,
            "qwen3-embed",
            vec![0.1; 4],
        );
        let has_id = point
            .id
            .as_ref()
            .and_then(|id| match &id.point_id_options {
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(_)) => Some(true),
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(_)) => Some(true),
                None => None,
            })
            .unwrap_or(false);
        assert!(has_id);
        let payload = &point.payload;
        assert!(payload.contains_key("project"));
        assert!(payload.contains_key("content"));
        assert!(payload.contains_key("category"));
        assert!(payload.contains_key("file_hash"));
        assert!(payload.contains_key("timestamp"));
        assert!(payload.contains_key("id"));
    }
}
