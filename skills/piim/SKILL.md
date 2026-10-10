---
name: piim
description: This skill provides the AI agent with a high-performance, persistent long-term memory system optimized for local code repositories, enabling lightning-fast hybrid retrieval while conserving context window tokens and local compute resources
---

# Project Info in MCP 

## Available Tools

### 1. `upsert_project_info`
*   **Purpose:** Securely inserts or updates an atomic piece of project context, documentation, or code contracts.
*   **Parameters:**
    *   `info_id` (string): Unique identifier — snake_case for concepts (e.g., `jwt_validation_logic`). Relative file paths are reserved for `save_file_description`, because ids are unique per project (see `project` below).
    *   `content` (string): The distilled code snippet, architectural summary, or text block to remember.
    *   `category` (string): Strict isolation scope with use-case. Must be one of: `architecture` (recall design/ADRs before refactoring), `codestyle` (recall rules before writing code), `file` (find which file implements a feature), `function` (find callers/params), `code_contract` (recall interfaces/types before integration), `todo` (plan work/track bugs), `changelog` (recall what changed). Aliases like `api` normalize to `code_contract`. Saved content is auto-enriched with a Use-cases trailer for smart hybrid search.
    *   `project` (string, optional): Project scope. Defaults to the server's `PROJECT_NAME` when omitted. The dedup key is `(info_id, project)`, so the same id can exist in many projects without collision.
*   **Optimization Note:** The underlying Rust actor computes a SHA-256 hash before executing. If the data is identical, the GPU text embedding inference is skipped automatically.

### 2. `delete_project_info`
*   **Purpose:** Deletes a single project info record by `(id, project)`.
*   **Parameters:**
    *   `info_id` (string): Unique record key to delete.
    *   `project` (string, optional): Project scope; defaults to the server's `PROJECT_NAME`.
*   **Reply:** `✅ deleted` when a record existed, `ℹ️ not found` otherwise.

### 3. `save_file_description`
*   **Purpose:** Stores a short description of a file's content in memory; the `(file path, project)` pair is the unique id (stored under the `file` category).
*   **Parameters:**
    *   `file_path` (string): Unique key within the project — relative path to the file (e.g., `src/auth.rs`). Re-saving the same path in the same project overwrites the previous description.
    *   `description` (string): 1–3 sentences: file purpose, key functions/types it defines, how it is used.
    *   `project` (string, optional): Project scope; defaults to the server's `PROJECT_NAME`. Always pass the current project name when working outside the default project.
*   **Optimization Note:** Same SHA-256 dedup as `upsert_project_info` — unchanged descriptions skip embedding.

### 4. `save_function_description`
*   **Purpose:** Stores a short description of a single function/method in memory; the unique id is `<file_path>::<function_name>` for free functions, or `<file_path>::<struct_name>::<function_name>` for struct/impl-associated functions (stored under the `function` category, scoped per project).
*   **Parameters:**
    *   `file_path` (string): Relative path to the file containing the function (e.g., `src/auth.rs`).
    *   `function_name` (string): Function or method name (e.g., `validate_jwt`).
    *   `struct_name` (string, optional): Struct/impl name for associated functions/methods (e.g., `AuthService`); omit for free functions.
    *   `description` (string): 1–3 sentences: what the function does, parameters/return value, side effects, how it is used.
    *   `project` (string, optional): Project scope; defaults to the server's `PROJECT_NAME`. Re-saving the same `(file_path, struct_name, function_name)` in the same project overwrites the previous description.
*   **Optimization Note:** Same SHA-256 dedup as `upsert_project_info` — unchanged descriptions skip embedding.

### 5. `hybrid_search_memory`
*   **Purpose:** Executes an ultra-fast hybrid search combining dense semantic vectors and exact keyword matches (BM25) over the stored project repository knowledge. Search is always scoped to one project (or all with `"*"`).
*   **Parameters:**
    *   `query` (string): Natural language query or exact function/variable name.
    *   `category` (string, optional): Filters the search strictly to a specific metadata category to narrow scope. Pick by use-case: `architecture` for design, `codestyle` for rules before writing code, `file`/`function` for symbol lookup, `code_contract` for interfaces, `todo`/`changelog` for plans/history. The query is auto-expanded with category keywords for smart search.
    *   `limit` (integer): Maximum number of records to return (e.g., 10).
    *   `project` (string, optional): Project scope; defaults to the server's `PROJECT_NAME`. Pass `"*"` to search across all projects.
*   **Output:** matches are listed as `[project:category] id (distance)` followed by the stored content.

### 6. `optimize_database`
*   **Purpose:** Triggers file compaction, merges small Arrow record batches, and garbage-collects historical timeline versions within the DB table to optimize disk I/O and maintain low-latency lookups.

### 7. `memory_stats`
*   **Purpose:** Reports memory usage statistics — total record count, per-category breakdown, and content size (total/avg chars) — so you can watch how full the memory is without reading every record.
*   **Parameters:**
    *   `project` (string, optional): Project scope; defaults to the server's `PROJECT_NAME`. Pass `"*"` to aggregate all projects (reply includes a per-project breakdown).
*   **When to use:** at session start to gauge memory size, before a big file-catalog walk to see what's already stored, or when deciding whether to compact (`optimize_database`) or prune stale records.

## Multi-Project Scoping

*   One database serves several projects: every record carries a `project` column.
*   **Default:** when you omit `project`, the server uses its `PROJECT_NAME` env value (`default` if unset). Reads and writes never leak across projects unless you explicitly pass `"*"` (search/stats only).
*   **Rule:** always pass the current project name explicitly in `upsert_project_info`, `delete_project_info`, `save_file_description`, `save_function_description`, `hybrid_search_memory`, and `memory_stats` when the session's project differs from the server default. Never invent project names — use the repository/project name you are working in.
*   **Cross-project lookup:** pass `project: "*"` to `hybrid_search_memory` / `memory_stats` when the user asks to search everywhere; the reply shows which project each hit belongs to (`[project:category]`).

## Operational Rules & Behavioral Guidelines

### 1. Token Economy (Proactive Context Offloading)
*   **Do Not Feed Entire Files Repeatedly:** Instead of keeping large tracking files, markdown schemas, or structural indices constantly inside your active system prompt, offload them using `upsert_project_info`.
*   **On-Demand Retrieval:** When starting a task in an area of the codebase not currently visible in your workspace context, call `hybrid_search_memory` first to fetch only relevant definitions.

### 2. Categorization Protocol
*   Categorize data with precision to maintain efficient SQL metadata filtering on the DB engine:
    *   Use `file` for per-file content descriptions written via `save_file_description` (id = relative file path).
    *   Use `function` for per-function descriptions written via `save_function_description` (id = `<file_path>::<function_name>` or `<file_path>::<struct_name>::<function_name>`).
    *   Use `architecture` for configuration formats, core dependencies, API endpoint signatures, and ADRs.
    *   Use `codestyle` for code style rules and conventions — one focused rule per record via `upsert_project_info` (e.g., `rust_naming_conventions`, `rust_error_handling`, `rust_import_grouping`, `rust_qa_workflow`). Content must state the rule, what to avoid, and a minimal good/bad example, and include the keywords future queries will use (naming, clippy/rustfmt, `?`/`if let`/iterator chains, `thiserror`/`anyhow`, import grouping, `///` docs, `cargo fmt`/`cargo clippy`/`cargo test --all`). Search it with `hybrid_search_memory` + `category: "codestyle"` before writing or editing code.
    *   Use `code_contract` for internal types, interfaces, traits, and shared state structures.
    *   Use `todo` to capture structural bugs, tech debt, and immediate feature requirements.
    *   Use `changelog` to summarize your own work at the end of a session (files modified, logic added, and architectural impacts).

### 3. How to Save All Files Info (`save_file_description`)
*   **When (if needed):** on first onboarding to an unfamiliar project, or after a refactor changes a file's purpose or public symbols. Skip it when the files are already inside your context window — store only what you would otherwise have to re-read later.
*   **How:** walk the project's source tree and call `save_file_description` once per source file:
    *   `file_path` — relative path from the project root; it is the unique id within the project (e.g., `src/auth.rs`). Re-saving the same path in the same project overwrites its previous description.
    *   `description` — 1–3 sentences: what the file does, which key functions/structs/traits it defines, and what it depends on.
    *   `project` (optional) — project scope; defaults to the server's `PROJECT_NAME`. Pass the current project name explicitly when working outside the default project.
*   **Scope:** only real project sources; skip generated and vendored trees (`target/`, `node_modules/`, `dist/`, lock files). Keep descriptions concise — every changed description costs one embedding call (unchanged ones are skipped via SHA-256 dedup).

### 4. How to Save Function Info (`save_function_description`)
*   **When:** after onboarding file descriptions, for key public functions/methods whose behavior you would otherwise have to re-read; or whenever a function's signature or behavior changes.
*   **How:** call `save_function_description` once per function:
    *   `file_path` — relative path from the project root (e.g., `src/auth.rs`).
    *   `function_name` — function/method name (e.g., `validate_jwt`).
    *   `struct_name` (optional) — struct/impl name for associated functions/methods (e.g., `AuthService`); omit for free functions.
    *   `description` — 1–3 sentences: what the function does, its parameters/return value, side effects, and how it is used.
    *   `project` (optional) — project scope; defaults to the server's `PROJECT_NAME`.
*   **Unique id:** the tool builds `<file_path>::<function_name>` (e.g., `src/auth.rs::validate_jwt`) or `<file_path>::<struct_name>::<function_name>` (e.g., `src/auth.rs::AuthService::validate`) under the `function` category, scoped per project. Re-saving the same triple in the same project overwrites the previous description (SHA-256 dedup skips embedding when unchanged).

### 5. How to Search Function/Type Usage in the Project
*   Call `hybrid_search_memory` with:
    *   `query` — the exact symbol name (e.g., `validate_jwt`), optionally with context ("who calls validate_jwt").
    *   `category` — `"file"` for file descriptions, `"function"` for function descriptions (omit to search both plus everything else).
    *   `limit` — how many candidate files to inspect (e.g., `10`).
    *   `project` (optional) — project scope; defaults to the server's `PROJECT_NAME`. Pass `"*"` only on explicit user request to search all projects.
*   The reply lists matches as `[project:category] id (distance)` followed by the stored description: file hits show the file path, function hits show `<file_path>::<function>` ids. Treat the top hits as candidates: open those files and grep for the symbol to confirm exact usages.
*   If you need types and contracts instead of files, repeat with `category` `code_contract` or `architecture`; if the file/function catalog is empty, fall back to ripgrep over the repository.

### 6. Precision Token Matching
*   When a user asks about specific system internals (e.g., *"Where do we validate JWT tokens?"*), do not guess. Invoke `hybrid_search_memory` with the method name or keyword. The hybrid FTS (Full-Text Search) engine will locate exact lexical matches, while the vector engine fetches surrounding semantic contexts.
*   To find usages of a function, type, or constant across the project, call `hybrid_search_memory` with the symbol name as `query`: first with `category: "function"` for exact function descriptions, then with `category: "file"` for candidate files — open the top hits and grep for the symbol to confirm exact usages.

### 7. How to Watch Memory Usage (`memory_stats`)
*   Call `memory_stats` with optional `project` (defaults to the server's `PROJECT_NAME`; `"*"` aggregates all projects with a per-project breakdown) to see record count, per-category breakdown, and content size (total/avg chars) — no embedding inference, so it is cheap.
*   **When:** at session start to gauge what's already stored, before a file-catalog walk to avoid re-saving, or when deciding whether to run `optimize_database` / prune stale records.
