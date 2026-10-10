//! Arrow schema and record-batch helpers for the LanceDB table.

use std::sync::Arc;

use arrow_array::builder::{FixedSizeListBuilder, Float32Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema};

/// Escapes a string literal for use inside a LanceDB/SQL `only_if` predicate.
///
/// Wraps the value in single quotes and doubles any embedded single quotes
/// (standard SQL escaping), so project ids/categories containing `'` can't
/// break out of the literal.
pub fn escape_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Builds the Arrow table schema
/// (`id`, `project`, `content`, `vector`, `category`, `file_hash`, `timestamp`)
/// with a fixed-size float vector of the given embedding dimension.
pub fn table_schema(dimension: usize) -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("project", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                dimension as i32,
            ),
            false,
        ),
        Field::new("category", DataType::Utf8, false),
        Field::new("file_hash", DataType::Utf8, false),
        Field::new("timestamp", DataType::Int64, false),
    ]))
}

/// Builds a single Arrow `RecordBatch` containing one
/// project info record with its embedding vector.
///
/// # Errors
///
/// Returns an error when `vector` length does not match
/// `dimension`, or when the
/// resulting `RecordBatch` fails schema validation.
#[allow(clippy::too_many_arguments)]
pub fn build_arrow_record(
    id: &str,
    project: &str,
    content: &str,
    category: &str,
    hash: &str,
    ts: i64,
    vector: Vec<f32>,
    dimension: usize,
) -> Result<RecordBatch, String> {
    let schema = table_schema(dimension);

    let mut id_builder = StringBuilder::with_capacity(1, id.len());
    let mut project_builder = StringBuilder::with_capacity(1, project.len());
    let mut content_builder = StringBuilder::with_capacity(1, content.len());
    let mut category_builder = StringBuilder::with_capacity(1, category.len());
    let mut hash_builder = StringBuilder::with_capacity(1, hash.len());
    let mut ts_builder = Int64Builder::with_capacity(1);

    let values_builder = Float32Builder::with_capacity(dimension);
    let mut vector_builder =
        FixedSizeListBuilder::new(values_builder, i32::try_from(dimension).unwrap_or(1024));

    id_builder.append_value(id);
    project_builder.append_value(project);
    content_builder.append_value(content);
    category_builder.append_value(category);
    hash_builder.append_value(hash);
    ts_builder.append_value(ts);

    if vector.len() != dimension {
        return Err(format!(
            "Critical error: Length of vector from Model ({}) does not match VECTOR_DIMENSION ({})",
            vector.len(),
            dimension
        ));
    }

    let values_item_builder = vector_builder.values();
    for &val in &vector {
        values_item_builder.append_value(val);
    }
    vector_builder.append(true);

    let id_array: ArrayRef = Arc::new(id_builder.finish());
    let project_array: ArrayRef = Arc::new(project_builder.finish());
    let content_array: ArrayRef = Arc::new(content_builder.finish());
    let vector_array: ArrayRef = Arc::new(vector_builder.finish());
    let category_array: ArrayRef = Arc::new(category_builder.finish());
    let hash_array: ArrayRef = Arc::new(hash_builder.finish());
    let ts_array: ArrayRef = Arc::new(ts_builder.finish());

    let columns = vec![
        id_array,
        project_array,
        content_array,
        vector_array,
        category_array,
        hash_array,
        ts_array,
    ];

    RecordBatch::try_new(schema, columns)
        .map_err(|e| format!("Critical error building Arrow RecordBatch: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_project_column() {
        let schema = table_schema(4);
        assert!(schema.field_with_name("project").is_ok());
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(
            names,
            vec![
                "id",
                "project",
                "content",
                "vector",
                "category",
                "file_hash",
                "timestamp"
            ]
        );
    }

    #[test]
    fn escapes_single_quotes_in_literals() {
        assert_eq!(escape_literal("my'project"), "'my''project'");
        assert_eq!(escape_literal("plain"), "'plain'");
    }

    #[test]
    fn builds_record_with_project() {
        let batch =
            build_arrow_record("id1", "proj-a", "hello", "todo", "hash", 1, vec![0.0; 4], 4)
                .expect("valid batch");
        assert_eq!(batch.num_rows(), 1);
        let idx = batch.schema().index_of("project").expect("project column");
        let values = arrow_array::cast::AsArray::as_string::<i32>(batch.column(idx));
        assert_eq!(values.value(0), "proj-a");
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let err = build_arrow_record("id1", "p", "hello", "todo", "hash", 1, vec![0.0; 2], 4)
            .unwrap_err();
        assert!(err.contains("does not match VECTOR_DIMENSION"));
    }
}
