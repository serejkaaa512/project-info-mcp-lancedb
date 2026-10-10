# PIIM (Project Info In MCP)

A persistent project-memory MCP server in Rust. Stores discrete facts (architecture notes, TODOs, API contracts, changelogs) in a vector database with hybrid search, exposed to AI agents via the [Model Context Protocol](https://modelcontextprotocol.io) over stdio.

This repository is a Cargo workspace with two storage backends sharing one core:

- **`piim-lance`** — embedded [LanceDB](https://lancedb.com) storage ([`crates/lancedb`](crates/lancedb), see its [README](crates/lancedb/README.md)) with ANN vector + full-text search, REST API, and dashboard.
- **`piim-qdrant`** — remote [Qdrant](https://qdrant.tech) storage ([`crates/qdrant`](crates/qdrant), see its [README](crates/qdrant/README.md)).

Both binaries are driven by **`piim-common`** ([`crates/common`](crates/common)): `kameo` actors (`ProjectInfoActor` + `EmbeddingActor`), the MCP tool layer, and the `MemoryStore` backend trait. Embeddings come from any OpenAI-compatible API (e.g. Qwen3-embed, TEI, vLLM).

## Features

- **Multi-project memory** — one database serves several projects: every record carries a `project` key (`id`, `project`, `content`, `vector`, `category`, `file_hash`, `timestamp`). The active project defaults to `PROJECT_NAME` (`default`) and can be overridden per tool call via the optional `project` argument (`"*"` searches/stats across all projects).
- **Persistent memory** — `project_memory` table/collection with the schema above (backend-specific creation details live in each crate README).
- **Hybrid search** — vector search over embeddings + full-text search on `content`, with optional `category` filter, always scoped to the active project (or `"*"` for all).
- **Content-aware upsert** — SHA-256 `file_hash` check skips embedding inference when content is unchanged; otherwise delete + re-insert. Dedup key is `(id, project)`, so the same file path can exist in many projects.
- **File catalog** — `save_file_description` stores one `file`-category record per source file *per project*; the `(file path, project)` pair is the unique id.
- **Pluggable storage backend** — the MCP surface is backed by the `MemoryStore` trait (`piim-common::store`); `crates/lancedb` provides `LanceStore` and `crates/qdrant` provides `QdrantStore`. Query construction, error messages, and maintenance ops (`optimize`, `reopen`) live in each implementation; the actor and MCP layers stay backend-agnostic.
- **Actor isolation** — `ProjectInfoActor` owns the storage backend; `EmbeddingActor` owns the HTTP client for the embeddings API.
- **Zero-config defaults** — works out of the box against `http://localhost:8002/v1/embeddings`.

## Prerequisites

- Rust 1.85+ (edition 2024 workspace; `cargo build`)

## Build & Run

```bash
cargo build --release                # whole workspace (piim-lance + piim-qdrant)
./target/release/piim-lance          # LanceDB backend (stdio MCP + HTTP dashboard)
./target/release/piim-qdrant         # Qdrant backend (stdio MCP)
```

Build a single binary:

```bash
cargo build --release -p piim-lance      # LanceDB only
cargo build --release -p piim-qdrant     # Qdrant only
```

## Configuration

Shared env vars (used by both backends): `EMBEDDINGS_URL` (`http://localhost:8002/v1/embeddings`), `EMBEDDINGS_MODEL` (`qwen3-embed`), `VECTOR_DIMENSION` (`1024`), `PROJECT_NAME` (`default`). Backend-specific storage settings:

- `piim-lance`: `LANCEDB_PATH`, `HTTP_PORT`, `SNAPSHOT_DIR` — see [crates/lancedb/README.md](crates/lancedb/README.md).
- `piim-qdrant`: `QDRANT_URL`, `QDRANT_API_KEY`, `QDRANT_COLLECTION` — see [crates/qdrant/README.md](crates/qdrant/README.md).

## Client setup (opencode)

See the backend READMEs for copy-paste configs: [piim-lance](crates/lancedb/README.md#client-setup-opencode) and [piim-qdrant](crates/qdrant/README.md).

## Tools

Both backends expose the same 6 MCP tools (`upsert_project_info`, `save_file_description`, `save_function_description`, `hybrid_search_memory`, `optimize_database`, `memory_stats`). See [crates/lancedb/README.md](crates/lancedb/README.md#tools) for the argument table.

## LanceDB vs Qdrant — the databases compared

This repo pins `lancedb` **0.40.0** (`piim-lance`) and `qdrant-client` **1.19** (`piim-qdrant`), but the choice between backends is really a choice between two different database philosophies. Both are Apache-2.0 open source and written in Rust.

| | LanceDB | Qdrant |
|---|---|---|
| What it is | Embedded multimodal lakehouse built on the Lance columnar format — a library that lives inside your process, no server to run. | Purpose-built vector-search server — a standalone service (Rust) you run separately (self-hosted, Docker, or Qdrant Cloud) and talk to over gRPC/REST. |
| Architecture / storage | Data = versioned Lance datasets on local disk or object storage (S3/GCS/Azure). Vectors, raw content, and metadata live in one Arrow-native table. Compute-storage separation keeps cost near object-storage rates (~$0.02/GB/month). | Data = collections of points (vector + JSON payload) held in a dedicated engine: HNSW graph index for vectors plus RocksDB-backed payload storage. RAM-heavy by design — shards and payload indexes want memory, so cost tracks compute/RAM. |
| Scaling model | Vertical + object storage: one table can grow very large (20 PB-scale in LanceDB Cloud claims) without sharding decisions; scaling reads is mostly stateless. No built-in horizontal sharding in OSS embedded mode. | Horizontal by design: collections shard and replicate across nodes, with dynamic rebalancing for billion-scale datasets and multi-writer setups. Better fit for distributed, high-throughput production clusters. |
| Vector search | ANN search (IVF-PQ / HNSW depending on version and config) with query-time precision tuning; indexing is largely automatic, little parameter tuning. | Highly tunable HNSW (`m`, `ef_construct`, `ef` at query time) plus quantization (scalar/binary/product) and segment optimizer settings. More knobs = higher peak single-query performance, but requires tuning. |
| Beyond vectors | Native: vector + full-text (FTS) + SQL filtering in one query; schema evolution is cheap (e.g. add a column for a new embedding model). | Vector-first: rich payload filtering (`must`/`should`/`must_not`, keyword/full-text-match indexes, geo, datetime) but no native FTS — text search needs externally produced sparse vectors. Multi-vector per point (named vectors) is first-class. |
| Filtering / metadata | Arrow-native columnar scan: `WHERE` predicates are fast and analytics-friendly; time-travel/versioning comes from the Lance format. | Payload indexes (keyword, integer, text, geo) that should be declared up front; filtering at scale is excellent but indexes must fit operational planning (RAM, segment optimization). |
| Operations | Zero-ops for a single node: no shards, no segments to rebalance, no server to upgrade — back up by copying files/snapshots. | Operated service: segment merging, shard allocation, replication, rolling upgrades, auth/API keys, snapshots via its API. More machinery, but built for teams that need HA and centralized management. |
| Best for | Local-first / single-machine apps (like this MCP server's default): zero extra services, cheap storage, hybrid vector+FTS out of the box, file-level backup. | Shared/remote deployments, many clients or writers, billion-scale or latency-critical search, existing platform teams that already run stateful services. |

## LanceDB vs Qdrant — which backend to pick?

Both binaries expose the same 6 MCP tools and share the `piim-common` core (actors, `MemoryStore` trait, HTTP/fastembed embedding backends, `PROJECT_NAME` scoping). They differ only in where and how vectors are stored.

| | `piim-lance` (embedded LanceDB) | `piim-qdrant` (remote Qdrant) |
|---|---|---|
| Crate / version used | `lancedb` **0.40.0** (`crates/lancedb/Cargo.toml`), binary `piim-lance` 0.1.0 | `qdrant-client` **1.19** (`crates/qdrant/Cargo.toml`), binary `piim-qdrant` 0.3.1 |
| Deployment | Embedded — no extra service. Data lives in a local directory (`LANCEDB_PATH`, default `./.opencode_memory/lance_db`). | Remote server — requires a running Qdrant (`QDRANT_URL`, default `http://localhost:6333`, e.g. `docker run -p 6333:6333 qdrant/qdrant`). Optional `QDRANT_API_KEY` auth. |
| Storage unit | One Arrow table `project_memory` (`id`, `project`, `content`, `vector`, `category`, `file_hash`, `timestamp`). | One collection (`QDRANT_COLLECTION`, default `project_memory`); each record is a point with a UUID point-id and the same fields as JSON payload. |
| Search | True hybrid: ANN vector search + FTS index on `content` (index auto-created on first search), with `project`/`category` SQL predicates. | Filtered vector search (Cosine distance) via `SearchPointsBuilder` with payload filters on `project`/`category`. No FTS index — keyword matching relies on embeddings. |
| Multi-model support | One table = one `VECTOR_DIMENSION`; changing the embedding model requires recreating the table (fail-fast if the `project` column is missing on old tables). | Named vectors per embedding model (`vector_name` = `EMBEDDINGS_MODEL`); one collection can hold vectors from several models side by side, plus keyword payload indexes on `id`/`project`/`category`. |
| Upsert | Hash pre-check then `delete` + `add` (LanceDB has no in-place point update). | Hash pre-check via `scroll`, then `upsert_points(wait=true)` with a fresh UUID point. |
| `optimize_database` | Real work: runs LanceDB `optimize()` / compaction on the table. | No-op by design: Qdrant manages storage itself, returns "optimized" immediately. |
| Extras | Built-in REST API + dashboard (`HTTP_PORT`, default `6333`, `0` to disable) and `.tar.gz` snapshots (`SNAPSHOT_DIR`). Stdio MCP runs alongside. | No dashboard/snapshot layer in this repo — use the Qdrant server's own UI/API. Stdio MCP only. |
| Best for | Single-machine / local-first use, zero-ops setup, file-based backup (copy the directory or snapshot). | Shared or remote deployments, larger scale, multiple writers/readers, existing Qdrant infrastructure. |

## Architecture

```text
┌───────────────────────  piim-common (crates/common)  ────────────────────────┐
│ stdin (JSON-RPC) → StdioTransport → MemoryToolHandler  (6 MCP tools)         │
│                                        │ tools/call                          │
│                                        ▼                                     │
│                          ProjectInfoActor (kameo)                            │
│                            ├─ generic over the MemoryStore trait             │
│                            └─ asks EmbeddingActor ──POST EMBEDDINGS_URL──▶   │
│                                                             embeddings API   │
└────────────────────────────────┬─────────────────────────────────────────────┘
                                 │ MemoryStore (crates/common/src/store.rs)
                 ┌───────────────┴───────────────┐
                 ▼                               ▼
   crates/lancedb  (piim-lance)         crates/qdrant  (piim-qdrant)
   LanceStore: table open/create,     QdrantStore: client, points & filters,
   Arrow records, hybrid query,       hybrid/structured search, counts,
   hash dedup, FTS index, optimize,   payload stats, optimize, collection
   snapshots, REST API + dashboard    creation
```

| Crate | Package | Role |
|---|---|---|
| [`crates/common`](crates/common) | `piim-common` | Shared core: `MemoryStore` trait, `CommonConfig` (shared env parsing), `ProjectInfoActor` + all message handlers, `EmbeddingActor`, MCP server + tool definitions, search/stats text formatting. |
| [`crates/lancedb`](crates/lancedb) | `piim-lance` | LanceDB backend binary (see its [README](crates/lancedb/README.md)). |
| [`crates/qdrant`](crates/qdrant) | `piim-qdrant` | Qdrant backend binary. |

Backend-specific behavior — query construction, error messages, `optimize`/`reopen` wording, the empty-stats noun ("table" vs "collection") — lives entirely inside each `MemoryStore` implementation, so adding a backend means implementing the trait plus a thin `main.rs`/`config.rs`.

## Development

```bash
cargo fmt --all             # format all workspace members
cargo clippy --all-targets  # lint (must be clean)
cargo test --all            # tests across the workspace
```
