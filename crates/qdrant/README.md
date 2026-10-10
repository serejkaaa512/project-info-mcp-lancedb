# piim-qdrant

MCP server for storing and searching project information in Qdrant with vector search.

## Prerequisites

- **Rust** (compiler)
- **Qdrant** — vector database server (local or remote)
- **Embeddings** — either an external API endpoint or local inference via [fastembed-rs](https://github.com/anush008/fastembed-rs)

## Quick Start

### 1. Run Qdrant

Locally via Docker:

```bash
docker run -p 6333:6333 -p 6334:6334 \
  -v $(pwd)/qdrant_storage:/qdrant/storage:z \
  qdrant/qdrant
```

Or connect to an existing remote server.

### 2. Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `QDRANT_URL` | Yes | `http://localhost:6334` | Qdrant server URL. |
| `QDRANT_API_KEY` | No | — | API key for authentication. |
| `QDRANT_COLLECTION` | No | `project_memory` | Name of the Qdrant collection. |
| `EMBEDDINGS_BACKEND` | No | `http` | Embedding backend: `http` (external API) or `fastembed` (local inference). |
| `EMBEDDINGS_URL` | Yes (http) | `http://localhost:8002/v1/embeddings` | Embeddings API URL (OpenAI-compatible). Ignored when `local`. |
| `EMBEDDINGS_MODEL` | No | `qwen3-embed` | Embedding model name. |
| `VECTOR_DIMENSION` | No | `1024` | Expected embedding vector size (auto-detected for `local`). |
| `PROJECT_NAME` | No | `default` | Default project scope. Overridable per tool call. |
| `FASTEMBED_CACHE_DIR` | No | `~/.cache/fastembed` | Custom directory for fastembed model cache. Only used when `EMBEDDINGS_BACKEND=fastembed`. |

### 3. Local Embeddings (fastembed)

Set `EMBEDDINGS_BACKEND=fastembed` to run embeddings locally without an external API. Models are downloaded once and cached.

**Supported models:**

| Model Name | Dimensions | Params | Notes |
|---|---|---|---|
| `bge-small-en` / `bge-small-en-v1.5` | 384 | 130M | Default ONNX model |
| `bge-base-en` / `bge-base-en-v1.5` | 768 | 110M | |
| `bge-large-en` / `bge-large-en-v1.5` | 1024 | 335M | |
| `nomic-embed-text` / `nomic-embed-text-v1` | 768 | 27M | |
| `nomic-embed-text-v1.5` | 768 | 27M | |
| `all-MiniLM-L6-v2` / `minilm-l6` | 384 | 22M | |
| `qwen3-embed` / `qwen3-embedding-0.6b` | 1024 | 0.6B | Requires `qwen3` feature |
| `qwen3-embedding-4b` | 4096 | 4B | Requires `qwen3` feature |
| `qwen3-embedding-8b` | 4096 | 8B | Requires `qwen3` feature |

**Example with local BGE model:**

```bash
export QDRANT_URL="http://localhost:6334"
export EMBEDDINGS_BACKEND="fastembed"
export EMBEDDINGS_MODEL="bge-small-en"
export PROJECT_NAME="my-project"
```

**Example with local Qwen3 model:**

```bash
export QDRANT_URL="http://localhost:6334"
export EMBEDDINGS_BACKEND="fastembed"
export EMBEDDINGS_MODEL="qwen3-embed"
export PROJECT_NAME="my-project"
```

> **Note:** The first run downloads the model (~100MB–8GB depending on model). Subsequent runs use the cached model with no network needed.

> **Custom cache directory:** By default, fastembed downloads models to a platform-specific cache directory (e.g. `~/.cache/fastembed`). Override this with the `FASTEMBED_CACHE_DIR` environment variable:
>
> ```bash
> export FASTEMBED_CACHE_DIR="/path/to/custom/cache"
> ```

Example (HTTP backend):

```bash
export QDRANT_URL="http://localhost:6334"
export EMBEDDINGS_URL="http://localhost:8002/v1/embeddings"
export EMBEDDINGS_MODEL="qwen3-embed"
export VECTOR_DIMENSION=1024
export PROJECT_NAME="my-project"
```

### 4. Build

```bash
cargo build --release -p piim-qdrant
```

### 4. Run

```bash
cargo run -p piim-qdrant
```

The server connects to Qdrant, creates the collection if it doesn't exist, and starts listening via **stdio**.

## Using as an MCP Server

Start it through an MCP client — the server communicates over stdio using the MCP protocol.

## Client setup (opencode)

Add to `opencode.json`:

```json
{
  "mcp": {
    "piim": {
      "type": "local",
      "command": ["/full/path/to/target/release/piim-qdrant"],
      "environment": {
        "QDRANT_URL": "http://localhost:6334",
        "EMBEDDINGS_BACKEND": "fastembed",
        "EMBEDDINGS_MODEL": "bge-small-en",
        "PROJECT_NAME": "my-project"
      },
      "enabled": true
    }
  }
}
```

For HTTP backend (external embeddings API):

```json
{
  "mcp": {
    "piim": {
      "type": "local",
      "command": ["/full/path/to/target/release/piim-qdrant"],
      "environment": {
        "QDRANT_URL": "http://localhost:6334",
        "EMBEDDINGS_BACKEND": "http",
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

> Use an absolute `command` path. Ensure Qdrant is running at `QDRANT_URL` and `VECTOR_DIMENSION` matches your embedding model. Optional: set `QDRANT_API_KEY` and `QDRANT_COLLECTION` if using authentication or a custom collection name.

## Client setup (Zed)

Add to `~/.config/zed/settings.json`:

```json
{
  "mcp": {
    "piim": {
      "command": "/full/path/to/target/release/piim-qdrant",
      "args": [],
      "env": {
        "QDRANT_URL": "http://localhost:6334",
        "EMBEDDINGS_BACKEND": "fastembed",
        "EMBEDDINGS_MODEL": "bge-small-en",
        "PROJECT_NAME": "my-project"
      }
    }
  }
}
```

Or via `~/.config/zed/mcp.json`:

```json
{
  "mcp": {
    "piim": {
      "command": "/full/path/to/target/release/piim-qdrant",
      "args": [],
      "env": {
        "QDRANT_URL": "http://localhost:6334",
        "EMBEDDINGS_BACKEND": "fastembed",
        "EMBEDDINGS_MODEL": "bge-small-en",
        "PROJECT_NAME": "my-project"
      }
    }
  }
}
```

> Use an absolute `command` path. Ensure Qdrant is running at `QDRANT_URL` and `VECTOR_DIMENSION` matches your embedding model. Optional: set `QDRANT_API_KEY` and `QDRANT_COLLECTION` if using authentication or a custom collection name.

## Structure

```
crates/qdrant/
├── src/
│   ├── main.rs          # Entry point, server initialization
│   ├── config.rs        # Configuration from environment variables
│   ├── helpers.rs       # Helper functions (collection creation)
│   ├── store.rs         # Qdrant storage backend implementation
├── Cargo.toml
└── README.md
```

The embedding actor and MCP handlers are in the shared `piim-common` crate.

## Architecture

```
┌──────────────┐     stdio (MCP)     ┌─────────────────┐
│  MCP Client  │ ◄─────────────────► │  piim-qdrant    │
│              │                     │  (MCP Server)    │
└──────────────┘                     └────────┬────────┘
                                              │
                    ┌─────────────────────────┼─────────────────────────┐
                    │                         │                         │
                    ▼                         ▼                         ▼
            ┌──────────────┐       ┌─────────────────┐       ┌──────────────┐
            │ Embedding    │       │ ProjectInfo     │       │   Qdrant     │
            │ Actor        │──────►│ Actor           │──────►│ (vector DB)  │
            │ (HTTP/Local) │       │ (memory actor)  │       │              │
            └──────────────┘       └─────────────────┘       └──────────────┘
```

The Embedding Actor supports two backends:
- **HTTP** (`EMBEDDINGS_BACKEND=http`): Calls an external OpenAI-compatible embeddings API
- **Fastembed** (`EMBEDDINGS_BACKEND=fastembed`): Runs inference locally via [fastembed-rs](https://github.com/anush008/fastembed-rs) (ONNX/Candle)

## License

MIT
