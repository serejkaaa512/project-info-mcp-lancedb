# piim-lance

MCP server for storing and searching project information in embedded [LanceDB](https://lancedb.com) with ANN vector + full-text search, REST API, and dashboard.

Part of the [PIIM (Project Info In MCP)](../../README.md) workspace. Both binaries are driven by **`piim-common`** (`crates/common`): `kameo` actors (`ProjectInfoActor` + `EmbeddingActor`), the MCP tool layer, and the `MemoryStore` backend trait. Embeddings come from any OpenAI-compatible API (e.g. Qwen3-embed, TEI, vLLM).

## Features

- **Multi-project memory** — one LanceDB database serves several projects: every record carries a `project` column (`id`, `project`, `content`, `vector`, `category`, `file_hash`, `timestamp`). The active project defaults to `PROJECT_NAME` (`default`) and can be overridden per tool call via the optional `project` argument (`"*"` searches/stats across all projects).
- **Persistent memory** — `project_memory` LanceDB table with the schema above (no migration: pre-`project`-column tables must be recreated — delete the table directory or use a fresh `LANCEDB_PATH`).
- **Hybrid search** — ANN vector search over embeddings + FTS index on `content`, with optional `category` filter, always scoped to the active project (or `"*"` for all).
- **Content-aware upsert** — SHA-256 `file_hash` check skips embedding inference when content is unchanged; otherwise delete + re-insert. Dedup key is `(id, project)`, so the same file path can exist in many projects.
- **File catalog** — `save_file_description` stores one `file`-category record per source file *per project*; the `(file path, project)` pair is the unique id.
- **Actor isolation** — `ProjectInfoActor` owns the LanceDB `Table`; `EmbeddingActor` owns the HTTP client for the embeddings API.
- **Zero-config defaults** — works out of the box against `http://localhost:8002/v1/embeddings`.

## Prerequisites

- Rust 1.85+ (edition 2024 workspace; `cargo build`)
- A running OpenAI-compatible embeddings endpoint, e.g.:
  ```bash
  curl -X POST http://localhost:8002/v1/embeddings \
    -H 'Content-Type: application/json' \
    -d '{"input":"hello","model":"qwen3-embed","encoding_format":"float"}'
  ```

## Build & Run

```bash
cargo build --release -p piim-lance   # LanceDB only
./target/release/piim-lance           # stdio MCP + HTTP dashboard
```

Run `piim-lance` with custom config:

```bash
LANCEDB_PATH=./.opencode_memory/lance_db \
EMBEDDINGS_URL=http://localhost:8002/v1/embeddings \
EMBEDDINGS_MODEL=qwen3-embed \
VECTOR_DIMENSION=1024 \
PROJECT_NAME=my-project \
./target/release/piim-lance
```

## Configuration

| Env var | Default | Description |
|---|---|---|
| `LANCEDB_PATH` | `./.opencode_memory/lance_db` | LanceDB storage directory (created on first run). |
| `EMBEDDINGS_URL` | `http://localhost:8002/v1/embeddings` | Embeddings HTTP endpoint (OpenAI-compatible). |
| `EMBEDDINGS_MODEL` | `qwen3-embed` | Model name sent as `model` in the embedding request. |
| `VECTOR_DIMENSION` | `1024` | Expected embedding size; upsert fails fast on mismatch. |
| `PROJECT_NAME` | `default` | Default project scope for all records; one shared DB can hold many projects. Overridable per tool call via the optional `project` argument. |
| `HTTP_PORT` | `6333` | Port for the REST API + dashboard (Qdrant-style). Set to `0` or empty to disable HTTP (stdio MCP only). |
## Dashboard & REST API (Qdrant-style)

Alongside MCP stdio, the server exposes an HTTP API + dashboard on `:HTTP_PORT` (default `6333`):

- Dashboard: `http://localhost:6333/dashboard` — collection overview, per-project/category stats, point browser (filter + pagination + delete), hybrid search, upsert form, snapshot manager. Page markup, stylesheet, and client script live in `src/http/static/` (`dashboard.html`, `style.css`, `app.js`), baked into the binary via `include_str!` and served at `/dashboard`, `/static/style.css`, `/static/app.js`.
- `GET /healthz`, `GET /readyz`, `GET /api/version`
- `GET /api/collections`, `GET /api/collections/stats?project=...`
- `GET /api/points?project=&category=&query=&limit=&offset=`
- `POST /api/points/upsert` `{"id","content","category","project?"}` (embeds like the MCP tool)
- `POST /api/points/search` `{"query","category?","limit?","project?"}`
- `DELETE /api/points/:id?project=...`
- `POST /api/optimize`
- Snapshots (`.tar.gz` of the whole `LANCEDB_PATH` dir):
  `GET /api/snapshots`, `POST /api/snapshots`, `POST /api/snapshots/:name/restore`, `DELETE /api/snapshots/:name`, `GET /api/snapshots/:name/download`.
  Restore wipes the DB dir, extracts the archive, and re-opens the actor's table handle.

```bash
curl localhost:6333/healthz
curl -X POST localhost:6333/api/snapshots
curl 'localhost:6333/api/collections/stats?project=*'
```

## Client setup (opencode)

Add to `opencode.json`:

```json
{
  "mcp": {
    "piim": {
      "type": "local",
      "command": ["/full/path/to/target/release/piim-lance"],
      "environment": {
        "LANCEDB_PATH": "./.opencode_memory/lance_db",
        "EMBEDDINGS_URL": "http://localhost:8002/v1/embeddings",
        "EMBEDDINGS_MODEL": "qwen3-embed",
        "VECTOR_DIMENSION": "1024",
        "PROJECT_NAME": "my-project"
      },
      "enabled": true
    }
  }
}
```

> Use an absolute `command` path and keep `VECTOR_DIMENSION` in sync with your embedding model, otherwise upserts are rejected.

| `SNAPSHOT_DIR` | `./.opencode_memory/snapshots` | Directory where `.tar.gz` DB snapshots are stored. |

On startup the server opens the `project_memory` table, or creates it with the Arrow schema from `src/helpers.rs` if missing. If an existing table lacks the `project` column (created by an older version), the server exits with an error — recreate the table (delete it or use a fresh `LANCEDB_PATH`) instead of migrating.


## Tools

| Tool | Arguments | What it does |
|---|---|---|
| `upsert_project_info` | `info_id`, `content`, `category`, `project?` (defaults to `PROJECT_NAME`) | Hashes `content` (SHA-256); skips inference if hash matches existing `(id, project)` row; otherwise embeds content via `EmbeddingActor` and adds an Arrow record with current unix timestamp. |
| `save_file_description` | `file_path`, `description`, `project?` (defaults to `PROJECT_NAME`) | Upserts a `file`-category record keyed by `(file path, project)`; unchanged descriptions skip embedding, re-saving overwrites. |
| `save_function_description` | `file_path`, `function_name`, `struct_name?`, `description`, `project?` | Upserts a `function`-category record; SHA-256 dedup skips embedding when unchanged, re-saving overwrites. |
| `hybrid_search_memory` | `query`, `limit`, `category?`, `project?` (defaults to `PROJECT_NAME`; `"*"` searches all projects) | Embeds `query`, ensures an FTS index on `content`, then runs a hybrid nearest-neighbor query scoped to `project` (unless `"*"`) plus optional `category` filter. Returns matches as `[project:category] id (distance)` plus `content`. |
| `optimize_database` | _(none — must be called with no arguments)_ | Runs LanceDB `optimize()` / compaction on the table. |
| `memory_stats` | `project?` (defaults to `PROJECT_NAME`; `"*"` aggregates all projects) | Reports total record count, per-category breakdown, and content size via a single column-projection scan; no embedding inference. |

## Structure

```
crates/lancedb/
├── src/
│   ├── main.rs          # Entry point, stdio wiring, table open/create, HTTP dashboard
│   ├── config.rs        # Configuration from environment variables
│   ├── helpers.rs       # table_schema() + build_arrow_record()
│   ├── store.rs         # LanceStore (MemoryStore impl): hybrid/vector queries, hash dedup, FTS index, stats
│   ├── http/            # REST routes, dashboard, snapshots (static assets baked in via include_str!)
├── Cargo.toml
└── README.md
```

The embedding actor and MCP handlers are in the shared `piim-common` crate.

### Module map

- `src/main.rs` — stdio wiring, table open/create (+ fail-fast check for the `project` column on old tables), actor spawn, MCP `Server::start()`, HTTP dashboard.
- `src/config.rs` — `Config::get_from_env()` with defaults above.
- `src/store.rs` — `LanceStore` (`MemoryStore` impl): hybrid/vector queries, hash-dedup precheck, FTS index creation, column-projection stats.
- `src/helpers.rs` — `table_schema()` + `build_arrow_record()` (validates `vector.len() == VECTOR_DIMENSION`).
- `src/http/` — REST routes, dashboard, snapshots (static assets baked in via `include_str!`).

## License

MIT
