//! REST route handlers (Qdrant-style JSON API).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;

use crate::actors::{
    DeletePointMessage, ListPointsMessage, OptimizeMessage, ReopenMessage, SearchHit,
    StatsStructuredMessage, StructuredSearchMessage, UpsertMessage,
};
use crate::http::AppState;
use crate::http::dashboard::{APP_JS, DASHBOARD_HTML, STYLE_CSS};
use crate::http::snapshots::{self, SnapshotInfo};

/// Serves the dashboard SPA page.
pub async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

/// Serves the dashboard stylesheet.
pub async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE_CSS,
    )
}

/// Serves the dashboard client script.
pub async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
}

/// Liveness probe.
pub async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// Readiness probe (actor round-trip + stats query).
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    match state
        .actor
        .ask(StatsStructuredMessage {
            project: "*".to_string(),
        })
        .await
    {
        Ok(data) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ready", "points_count": data.total})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "not_ready", "error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Version info.
pub async fn version(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "app": "piim",
        "version": env!("CARGO_PKG_VERSION"),
        "model": state.model,
        "vector_dimension": state.vector_dimension,
        "default_project": state.default_project,
        "uptime_seconds": chrono::Utc::now().timestamp() - state.started_unix,
    }))
}

/// Single-collection view (the `project_memory` table
/// with per-project breakdown).
pub async fn collections(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let data = state
        .actor
        .ask(StatsStructuredMessage {
            project: "*".to_string(),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "collections": [{
            "name": "project_memory",
            "points_count": data.total,
            "vectors_count": data.total,
            "indexed_vectors_count": data.total,
            "segments_count": 1,
            "status": "green",
            "per_project": data.per_project,
            "per_category": data.per_category,
        }]
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct StatsQuery {
    pub project: Option<String>,
}

/// Collection stats for a project scope.
pub async fn collection_stats(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = q.project.unwrap_or_else(|| state.default_project.clone());
    let data = state
        .actor
        .ask(StatsStructuredMessage {
            project: project.clone(),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "project": project,
        "points_count": data.total,
        "scanned": data.scanned,
        "content_chars": data.content_chars,
        "avg_chars": data.avg_chars,
        "per_category": data.per_category,
        "per_project": data.per_project,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub project: Option<String>,
    pub category: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Paginated point listing with filters.
pub async fn list_points(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (points, total) = state
        .actor
        .ask(ListPointsMessage {
            project: q.project.unwrap_or_else(|| state.default_project.clone()),
            category: q.category,
            query: q.query,
            limit: q.limit.unwrap_or(50).clamp(1, 500),
            offset: q.offset.unwrap_or(0),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(serde_json::json!({"points": points, "total": total})))
}

#[derive(Debug, Deserialize)]
pub struct UpsertBody {
    pub id: String,
    pub content: String,
    pub category: String,
    pub project: Option<String>,
}

/// Upserts a point (goes through embedding, like the MCP tool).
pub async fn upsert_point(
    State(state): State<AppState>,
    Json(body): Json<UpsertBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.id.trim().is_empty() || body.content.trim().is_empty() {
        return Err(ApiError("id and content must not be empty".to_string()));
    }
    let text = state
        .actor
        .ask(UpsertMessage {
            id: body.id.clone(),
            content: body.content,
            category: body.category,
            project: body
                .project
                .unwrap_or_else(|| state.default_project.clone()),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(
        serde_json::json!({"status": "ok", "id": body.id, "message": text}),
    ))
}

#[derive(Debug, Deserialize)]
pub struct SearchBody {
    pub query: String,
    pub category: Option<String>,
    pub limit: Option<usize>,
    pub project: Option<String>,
}

/// Hybrid vector search returning structured hits.
pub async fn search_points(
    State(state): State<AppState>,
    Json(body): Json<SearchBody>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let hits = state
        .actor
        .ask(StructuredSearchMessage {
            query: body.query,
            category: body.category,
            limit: body.limit.unwrap_or(10).clamp(1, 100),
            project: body
                .project
                .unwrap_or_else(|| state.default_project.clone()),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(hits))
}

#[derive(Debug, Deserialize)]
pub struct DeleteQuery {
    pub project: Option<String>,
}

/// Deletes a point by `(id, project)`.
pub async fn delete_point(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<DeleteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = state
        .actor
        .ask(DeletePointMessage {
            id,
            project: q.project.unwrap_or_else(|| state.default_project.clone()),
        })
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    if deleted {
        Ok(Json(serde_json::json!({"status": "ok"})))
    } else {
        Err(ApiError::not_found("point not found"))
    }
}

/// Lists snapshots.
pub async fn list_snapshots(
    State(state): State<AppState>,
) -> Result<Json<Vec<SnapshotInfo>>, ApiError> {
    snapshots::list_snapshots(&state.snapshot_dir)
        .map(Json)
        .map_err(ApiError)
}

/// Creates a snapshot (blocking tar+gzip offloaded to a blocking thread).
pub async fn create_snapshot(
    State(state): State<AppState>,
) -> Result<Json<SnapshotInfo>, ApiError> {
    let db_dir = state.db_dir.clone();
    let snap_dir = state.snapshot_dir.clone();
    tokio::task::spawn_blocking(move || snapshots::create_snapshot(&db_dir, &snap_dir))
        .await
        .map_err(|e| ApiError(format!("Snapshot task failed: {e}")))?
        .map(Json)
        .map_err(ApiError)
}

#[derive(Debug, Deserialize, Default)]
pub struct RestoreQuery {
    /// When `true`, re-opens the actor's table handle after restore.
    pub reopen: Option<bool>,
}

/// Restores a snapshot (wipes + extracts DB dir, optionally reopens table).
pub async fn restore_snapshot(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RestoreQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db_dir = state.db_dir.clone();
    let snap_dir = state.snapshot_dir.clone();
    let owned_name = name.clone();
    tokio::task::spawn_blocking(move || {
        snapshots::restore_snapshot(&db_dir, &snap_dir, &owned_name)
    })
    .await
    .map_err(|e| ApiError(format!("Restore task failed: {e}")))?
    .map_err(ApiError)?;
    if q.reopen.unwrap_or(true) {
        state
            .actor
            .ask(ReopenMessage {})
            .await
            .map_err(|e| ApiError(e.to_string()))?;
    }
    Ok(Json(serde_json::json!({"status": "ok", "restored": name})))
}

/// Deletes a snapshot file.
pub async fn delete_snapshot(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    snapshots::delete_snapshot(&state.snapshot_dir, &name).map_err(ApiError)?;
    Ok(Json(serde_json::json!({"status": "ok", "deleted": name})))
}

/// Downloads a snapshot archive.
pub async fn download_snapshot(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(ApiError("Invalid snapshot name".to_string()));
    }
    let bytes = tokio::fs::read(std::path::Path::new(&state.snapshot_dir).join(&name))
        .await
        .map_err(|_| ApiError::not_found("snapshot not found"))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/gzip".parse().expect("static mime"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{name}\"")
            .parse()
            .map_err(|_| ApiError("Invalid snapshot name".to_string()))?,
    );
    Ok((headers, bytes).into_response())
}

/// Runs table optimization / compaction.
pub async fn optimize(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let text = state
        .actor
        .ask(OptimizeMessage {})
        .await
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(serde_json::json!({"status": "ok", "message": text})))
}

/// JSON error body with status code.
pub struct ApiError(String);

impl From<String> for ApiError {
    fn from(msg: String) -> Self {
        Self(msg)
    }
}

impl ApiError {
    fn not_found(msg: &str) -> Self {
        Self(format!("404:{msg}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (code, msg) = match self.0.strip_prefix("404:") {
            Some(rest) => (StatusCode::NOT_FOUND, rest.to_string()),
            None => (StatusCode::INTERNAL_SERVER_ERROR, self.0),
        };
        (code, Json(serde_json::json!({"error": msg}))).into_response()
    }
}
