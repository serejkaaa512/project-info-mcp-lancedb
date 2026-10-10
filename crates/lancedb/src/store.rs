//! LanceDB implementation of the shared [`MemoryStore`] trait.

use std::collections::BTreeMap;

use arrow_array::cast::AsArray;
use arrow_array::{Array, Float32Array, Float64Array, RecordBatch};
use async_trait::async_trait;
use lancedb::index::{Index, scalar::FtsIndexBuilder};
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::table::Table;
use lancedb::{Connection, connect};
use piim_common::actors::{PointRecord, SearchHit, StatsData};
use piim_common::store::{MemoryStore, Record};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::helpers::{build_arrow_record, escape_literal, table_schema};

/// Storage backed by a LanceDB table (`project_memory` inside `db_dir`).
pub struct LanceStore {
    table: Mutex<Table>,
    db_dir: String,
    vector_dimension: usize,
}

impl LanceStore {
    /// Connects to `db_dir` and opens (or creates) the `project_memory` table.
    ///
    /// # Errors
    ///
    /// Returns the raw LanceDB error text when connecting
    /// or table creation fails.
    pub async fn new(db_dir: String, vector_dimension: usize) -> Result<Self, String> {
        let db: Connection = connect(&db_dir)
            .execute()
            .await
            .map_err(|e| e.to_string())?;
        let table = match db.open_table("project_memory").execute().await {
            Ok(t) => t,
            Err(_) => db
                .create_table(
                    "project_memory",
                    RecordBatch::new_empty(table_schema(vector_dimension)),
                )
                .execute()
                .await
                .map_err(|e| e.to_string())?,
        };
        Ok(Self {
            table: Mutex::new(table),
            db_dir,
            vector_dimension,
        })
    }
}

#[async_trait]
impl MemoryStore for LanceStore {
    async fn prepare_upsert(&self, id: &str, project: &str, file_hash: &str) -> bool {
        let predicate = format!(
            "id = {} AND project = {}",
            escape_literal(id),
            escape_literal(project)
        );
        let table = self.table.lock().await;
        if let Ok(mut stream) = table.query().only_if(&predicate).limit(1).execute().await
            && let Some(Ok(batch)) = stream.next().await
            && batch.num_rows() > 0
        {
            if let Ok(hash_col_idx) = batch.schema().index_of("file_hash") {
                let hash_array = batch.column(hash_col_idx).as_string::<i32>();
                let old_hash = hash_array.value(0);
                if old_hash == file_hash {
                    return false;
                }
            }

            let _ = table.delete(&predicate).await;
        }
        true
    }

    async fn write_record(&self, record: Record<'_>, vector: Vec<f32>) -> Result<(), String> {
        let record_batch = build_arrow_record(
            record.id,
            record.project,
            record.content,
            record.category,
            record.file_hash,
            record.timestamp,
            vector,
            self.vector_dimension,
        )?;

        let table = self.table.lock().await;
        table
            .add(record_batch)
            .execute()
            .await
            .map_err(|e| format!("Error writing to LanceDB: {e}"))?;
        Ok(())
    }

    async fn search_text(
        &self,
        query_vector: Vec<f32>,
        project: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>, String> {
        let table = self.table.lock().await;
        let _ = table
            .create_index(&["content"], Index::FTS(FtsIndexBuilder::default()))
            .execute()
            .await;

        let mut query_builder = table
            .query()
            .nearest_to(query_vector)
            .map_err(|e| e.to_string())?
            .limit(limit);
        let mut filters = Vec::new();
        if project != "*" {
            filters.push(format!("project = {}", escape_literal(project)));
        }
        if let Some(cat) = category {
            filters.push(format!("category = {}", escape_literal(cat)));
        }
        if !filters.is_empty() {
            query_builder = query_builder.only_if(filters.join(" AND "));
        }

        let mut stream = query_builder
            .execute()
            .await
            .map_err(|e| format!("Error during hybrid search: {e}"))?;

        let mut found = Vec::new();
        while let Some(batch) = stream.next().await {
            let batch = batch.map_err(|e| e.to_string())?;
            collect_matches(&batch, &mut found);
        }
        Ok(found)
    }

    async fn search_structured(
        &self,
        query_vector: Vec<f32>,
        project: &str,
        category: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>, String> {
        let table = self.table.lock().await;
        let _ = table
            .create_index(&["content"], Index::FTS(FtsIndexBuilder::default()))
            .execute()
            .await;

        let mut qb = table
            .query()
            .nearest_to(query_vector)
            .map_err(|e| e.to_string())?
            .limit(limit.max(1));
        let mut filters = Vec::new();
        if project != "*" {
            filters.push(format!("project = {}", escape_literal(project)));
        }
        if let Some(cat) = category.filter(|c| !c.trim().is_empty()) {
            filters.push(format!("category = {}", escape_literal(cat)));
        }
        if !filters.is_empty() {
            qb = qb.only_if(filters.join(" AND "));
        }

        let mut stream = qb
            .execute()
            .await
            .map_err(|e| format!("Error during hybrid search: {e}"))?;
        let mut hits = Vec::new();
        while let Some(batch) = stream.next().await {
            let batch = batch.map_err(|e| e.to_string())?;
            collect_hits(&batch, &mut hits);
        }
        Ok(hits)
    }

    async fn count(&self, project: &str, category: Option<&str>) -> Result<usize, String> {
        let count_filter = list_predicate(project, category);
        let table = self.table.lock().await;
        let total = table
            .count_rows(count_filter)
            .await
            .map_err(|e| format!("Error counting rows: {e}"))?;
        Ok(total as usize)
    }

    async fn list(
        &self,
        project: &str,
        category: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<PointRecord>, String> {
        let table = self.table.lock().await;
        let mut query = table.query().select(Select::Columns(vec![
            "id".to_string(),
            "project".to_string(),
            "content".to_string(),
            "category".to_string(),
            "file_hash".to_string(),
            "timestamp".to_string(),
        ]));
        if let Some(filter) = list_predicate(project, category) {
            query = query.only_if(filter);
        }
        let mut stream = query
            .execute()
            .await
            .map_err(|e| format!("Error scanning table: {e}"))?;

        let mut all = Vec::new();
        while let Some(batch) = stream.next().await {
            let batch = batch.map_err(|e| e.to_string())?;
            collect_points(&batch, &mut all);
        }
        Ok(all)
    }

    async fn stats(&self, project: &str) -> Result<StatsData, String> {
        let count_filter =
            (project != "*").then(|| format!("project = {}", escape_literal(project)));
        let table = self.table.lock().await;
        let total = table
            .count_rows(count_filter)
            .await
            .map_err(|e| format!("Error counting rows: {e}"))? as usize;

        if total == 0 {
            return Ok(StatsData::default());
        }

        let mut query = table.query().select(Select::Columns(vec![
            "project".to_string(),
            "category".to_string(),
            "content".to_string(),
        ]));
        if project != "*" {
            query = query.only_if(format!("project = {}", escape_literal(project)));
        }
        let mut stream = query
            .execute()
            .await
            .map_err(|e| format!("Error scanning table for stats: {e}"))?;

        let mut per_category: BTreeMap<String, usize> = BTreeMap::new();
        let mut content_chars: usize = 0;
        let mut scanned: usize = 0;
        let all_projects = project == "*";
        let mut per_project: BTreeMap<String, usize> = BTreeMap::new();
        while let Some(batch) = stream.next().await {
            let batch = batch.map_err(|e| e.to_string())?;
            scanned += batch.num_rows();
            accumulate(
                &batch,
                &mut per_category,
                &mut content_chars,
                all_projects.then_some(&mut per_project),
            );
        }

        Ok(StatsData {
            total,
            scanned,
            content_chars,
            avg_chars: content_chars / scanned.max(1),
            per_category,
            per_project,
        })
    }

    async fn delete(&self, id: &str, project: &str) -> Result<bool, String> {
        let predicate = format!(
            "id = {} AND project = {}",
            escape_literal(id),
            escape_literal(project)
        );
        let table = self.table.lock().await;
        let existing = table
            .count_rows(Some(predicate.clone()))
            .await
            .map_err(|e| format!("Error checking point: {e}"))?;
        if existing == 0 {
            return Ok(false);
        }
        table
            .delete(&predicate)
            .await
            .map_err(|e| format!("Error deleting point: {e}"))?;
        Ok(true)
    }

    async fn optimize(&self) -> Result<String, String> {
        let table = self.table.lock().await;
        let _ = table.optimize(Default::default()).await;

        Ok("🔍 Kameo subtask completed. Database optimized".to_string())
    }

    async fn reopen(&self) -> Result<String, String> {
        let db: Connection = connect(&self.db_dir)
            .execute()
            .await
            .map_err(|e| format!("Error reconnecting to LanceDB: {e}"))?;
        let table = match db.open_table("project_memory").execute().await {
            Ok(t) => t,
            Err(_) => db
                .create_table(
                    "project_memory",
                    RecordBatch::new_empty(table_schema(self.vector_dimension)),
                )
                .execute()
                .await
                .map_err(|e| format!("Error recreating table: {e}"))?,
        };
        *self.table.lock().await = table;
        Ok("Table reopened".to_string())
    }

    fn stats_empty_noun(&self) -> &'static str {
        "table"
    }
}

/// Builds an optional SQL predicate for project/category scoping.
fn list_predicate(project: &str, category: Option<&str>) -> Option<String> {
    let mut filters = Vec::new();
    if project != "*" {
        filters.push(format!("project = {}", escape_literal(project)));
    }
    if let Some(cat) = category.filter(|c| !c.trim().is_empty()) {
        filters.push(format!("category = {}", escape_literal(cat)));
    }
    if filters.is_empty() {
        None
    } else {
        Some(filters.join(" AND "))
    }
}

/// Copies readable columns (`id`, `project`, `category`, `content`,
/// `_distance`) out of a text-search batch (missing `_distance` yields `None`).
fn collect_matches(batch: &RecordBatch, found: &mut Vec<SearchHit>) {
    for row in 0..batch.num_rows() {
        found.push(SearchHit {
            id: text_string_value(batch, "id", row).unwrap_or_default(),
            project: text_string_value(batch, "project", row).unwrap_or_default(),
            category: text_string_value(batch, "category", row).unwrap_or_default(),
            content: text_string_value(batch, "content", row).unwrap_or_default(),
            distance: text_distance_value(batch, row),
        });
    }
}

/// Copies readable columns out of a structured-search batch.
fn collect_hits(batch: &RecordBatch, out: &mut Vec<SearchHit>) {
    for row in 0..batch.num_rows() {
        out.push(SearchHit {
            id: string_value(batch, "id", row),
            project: string_value(batch, "project", row),
            category: string_value(batch, "category", row),
            content: string_value(batch, "content", row),
            distance: distance_value(batch, row),
        });
    }
}

/// Copies readable columns out of a list batch.
fn collect_points(batch: &RecordBatch, out: &mut Vec<PointRecord>) {
    for row in 0..batch.num_rows() {
        out.push(PointRecord {
            id: string_value(batch, "id", row),
            project: string_value(batch, "project", row),
            content: string_value(batch, "content", row),
            category: string_value(batch, "category", row),
            file_hash: string_value(batch, "file_hash", row),
            timestamp: int_value(batch, "timestamp", row),
        });
    }
}

/// Accumulates per-category counts and content length from one batch.
fn accumulate(
    batch: &RecordBatch,
    per_category: &mut BTreeMap<String, usize>,
    content_chars: &mut usize,
    mut per_project: Option<&mut BTreeMap<String, usize>>,
) {
    let schema = batch.schema();
    let category_idx = schema.index_of("category").ok();
    let content_idx = schema.index_of("content").ok();
    let project_idx = schema.index_of("project").ok();
    for row in 0..batch.num_rows() {
        if let Some(idx) = category_idx {
            let values = batch.column(idx).as_string::<i32>();
            if row < values.len() {
                *per_category
                    .entry(values.value(row).to_string())
                    .or_insert(0) += 1;
            }
        }
        if let Some(idx) = content_idx {
            let values = batch.column(idx).as_string::<i32>();
            if row < values.len() {
                *content_chars += values.value(row).len();
            }
        }
        if let (Some(idx), Some(per_project)) = (project_idx, per_project.as_mut()) {
            let values = batch.column(idx).as_string::<i32>();
            if row < values.len() {
                *per_project
                    .entry(values.value(row).to_string())
                    .or_insert(0) += 1;
            }
        }
    }
}

fn text_string_value(batch: &RecordBatch, column: &str, row: usize) -> Option<String> {
    let idx = batch.schema().index_of(column).ok()?;
    let values = batch.column(idx).as_string::<i32>();
    if row >= values.len() {
        return None;
    }
    Some(values.value(row).to_string())
}

fn string_value(batch: &RecordBatch, column: &str, row: usize) -> String {
    batch
        .schema()
        .index_of(column)
        .ok()
        .and_then(|idx| {
            let values = batch.column(idx).as_string::<i32>();
            (row < values.len()).then(|| values.value(row).to_string())
        })
        .unwrap_or_default()
}

fn int_value(batch: &RecordBatch, column: &str, row: usize) -> i64 {
    batch.schema().index_of(column).ok().map_or(0, |idx| {
        if let Some(arr) = batch
            .column(idx)
            .as_any()
            .downcast_ref::<arrow_array::Int64Array>()
            && row < arr.len()
        {
            return arr.value(row);
        }
        0
    })
}

fn text_distance_value(batch: &RecordBatch, row: usize) -> Option<f32> {
    let idx = batch.schema().index_of("_distance").ok()?;
    let column = batch.column(idx);
    if let Some(values) = column.as_any().downcast_ref::<Float32Array>()
        && row < values.len()
    {
        return Some(values.value(row));
    }
    if let Some(values) = column.as_any().downcast_ref::<Float64Array>()
        && row < values.len()
    {
        return Some(values.value(row) as f32);
    }
    None
}

fn distance_value(batch: &RecordBatch, row: usize) -> Option<f32> {
    let idx = batch.schema().index_of("_distance").ok()?;
    let col = batch.column(idx);
    if let Some(arr) = col.as_any().downcast_ref::<Float32Array>() {
        return (row < arr.len() && arr.is_valid(row)).then(|| arr.value(row));
    }
    if let Some(arr) = col.as_any().downcast_ref::<Float64Array>() {
        return (row < arr.len() && arr.is_valid(row)).then(|| arr.value(row) as f32);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{Float32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};

    use super::*;

    fn file_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("project", DataType::Utf8, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("_distance", DataType::Float32, true),
        ]));
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["src/auth.rs", "src/main.rs"])),
                Arc::new(StringArray::from(vec!["my-proj", "my-proj"])),
                Arc::new(StringArray::from(vec!["file", "file"])),
                Arc::new(StringArray::from(vec![
                    "JWT validation helpers",
                    "stdio wiring and actor spawn",
                ])),
                Arc::new(Float32Array::from(vec![0.125, 0.5])),
            ],
        )
        .expect("valid batch")
    }

    #[test]
    fn collects_rows_with_distance() {
        let mut found = Vec::new();
        collect_matches(&file_batch(), &mut found);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "src/auth.rs");
        assert_eq!(found[0].project, "my-proj");
        assert_eq!(found[0].category, "file");
        assert_eq!(found[0].content, "JWT validation helpers");
        assert_eq!(found[0].distance, Some(0.125));
        assert_eq!(found[1].id, "src/main.rs");
    }

    #[test]
    fn tolerates_missing_distance_column() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("project", DataType::Utf8, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["jwt_validation_logic"])),
                Arc::new(StringArray::from(vec!["my-proj"])),
                Arc::new(StringArray::from(vec!["todo"])),
                Arc::new(StringArray::from(vec!["Fix flaky expiry test"])),
            ],
        )
        .expect("valid batch");

        let mut found = Vec::new();
        collect_matches(&batch, &mut found);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "jwt_validation_logic");
        assert_eq!(found[0].project, "my-proj");
        assert_eq!(found[0].distance, None);
    }

    fn stats_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("project", DataType::Utf8, false),
            Field::new("category", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
        ]));
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec!["proj-a", "proj-a", "proj-b"])),
                Arc::new(StringArray::from(vec!["file", "todo", "file"])),
                Arc::new(StringArray::from(vec!["aaa", "bb", "c"])),
            ],
        )
        .expect("valid batch")
    }

    #[test]
    fn accumulates_category_counts_and_chars() {
        let mut per_category = BTreeMap::new();
        let mut per_project = BTreeMap::new();
        let mut content_chars = 0;
        accumulate(
            &stats_batch(),
            &mut per_category,
            &mut content_chars,
            Some(&mut per_project),
        );

        assert_eq!(per_category.get("file"), Some(&2));
        assert_eq!(per_category.get("todo"), Some(&1));
        assert_eq!(per_project.get("proj-a"), Some(&2));
        assert_eq!(per_project.get("proj-b"), Some(&1));
        assert_eq!(content_chars, 6);
    }
}
