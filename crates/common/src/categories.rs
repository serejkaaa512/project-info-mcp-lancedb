//! Canonical memory categories with use-cases for smart hybrid search.

/// Metadata describing one memory category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryInfo {
    /// Canonical lowercase category name stored in the backend.
    pub name: &'static str,
    /// One-line description of what belongs in this category.
    pub description: &'static str,
    /// Typical use-cases: when to save here / what questions it answers.
    pub usecases: &'static [&'static str],
    /// Keyword hints embedded into content trailers and queries.
    pub keywords: &'static [&'static str],
}

/// Canonical category catalog.
pub const CATEGORIES: &[CategoryInfo] = &[
    CategoryInfo {
        name: "architecture",
        description: "Database schemas, API contracts, module dependencies, ADRs.",
        usecases: &[
            "recall module boundaries before refactoring",
            "check API contracts before changing endpoints",
            "onboard onto system design and data flow",
        ],
        keywords: &[
            "design",
            "adr",
            "schema",
            "dependency",
            "contract",
            "module",
        ],
    },
    CategoryInfo {
        name: "codestyle",
        description: "Code style rules and conventions, one focused rule per record.",
        usecases: &[
            "recall naming, formatting and lint rules before writing code",
            "check error-handling and import-grouping conventions",
            "enforce mandatory QA workflow (fmt, clippy, test)",
        ],
        keywords: &[
            "naming",
            "clippy",
            "rustfmt",
            "error handling",
            "qa workflow",
        ],
    },
    CategoryInfo {
        name: "file",
        description: "One record per project file: purpose and key symbols it defines.",
        usecases: &[
            "find which file implements a feature",
            "map repository layout during onboarding",
            "locate candidate files for a symbol usage",
        ],
        keywords: &["file", "module", "layout", "which file"],
    },
    CategoryInfo {
        name: "function",
        description: "One record per function/method: behavior, params, side effects.",
        usecases: &[
            "find who implements or calls a function",
            "recall params, return values and side effects",
            "trace call chains without re-reading sources",
        ],
        keywords: &["function", "method", "params", "calls", "usage"],
    },
    CategoryInfo {
        name: "code_contract",
        description: "Interfaces, core structs, data types and exchange protocols.",
        usecases: &[
            "recall struct fields and protocol shapes before integration",
            "check interface guarantees and invariants",
            "find type definitions backing an API",
        ],
        keywords: &["interface", "struct", "type", "protocol", "contract"],
    },
    CategoryInfo {
        name: "todo",
        description: "Technical debt, planned features and discovered bugs.",
        usecases: &[
            "plan next work and track tech debt",
            "recall known bugs before fixing",
            "prioritize upcoming features",
        ],
        keywords: &["todo", "debt", "bug", "plan", "feature"],
    },
    CategoryInfo {
        name: "changelog",
        description: "Historical log of completed tasks: what changed, how, why.",
        usecases: &[
            "recall what changed and why",
            "audit past fixes before reverting",
            "summarize recent work for release notes",
        ],
        keywords: &["changed", "history", "release", "fixed"],
    },
];
/// Finds a category by name (case-insensitive, trimmed).
pub fn find(name: &str) -> Option<&'static CategoryInfo> {
    let needle = name.trim().to_lowercase();
    CATEGORIES.iter().find(|c| c.name == needle)
}

/// Normalizes a user category to its canonical name (maps aliases).
pub fn normalize(name: &str) -> String {
    let needle = name.trim().to_lowercase();
    match needle.as_str() {
        "api" | "code-contract" | "contract" => "code_contract".to_string(),
        "style" | "code-style" => "codestyle".to_string(),
        "files" => "file".to_string(),
        "functions" | "func" | "fn" => "function".to_string(),
        "todos" => "todo".to_string(),
        "architectures" => "architecture".to_string(),
        "changelogs" => "changelog".to_string(),
        _ => needle,
    }
}

/// Appends a Use-cases trailer so vector and FTS share retrieval cues.
pub fn enrich_content(category: &str, content: &str) -> String {
    let canonical = normalize(category);
    let Some(info) = find(&canonical) else {
        return content.to_string();
    };
    if content.contains("Use-cases:") {
        return content.to_string();
    }
    format!(
        "{}\n\n[category:{} - {} Use-cases: {}. Keywords: {}.]",
        content.trim_end(),
        info.name,
        info.description,
        info.usecases.join("; "),
        info.keywords.join(", ")
    )
}

/// Expands a search query with category keywords for smart search.
pub fn enrich_query(query: &str, category: Option<&str>) -> String {
    let q = query.trim();
    let norm = category.map(normalize);
    let Some(cat) = norm.as_deref().and_then(find) else {
        return q.to_string();
    };
    if q.contains("Use-cases:") {
        return q.to_string();
    }
    format!(
        "{q} (category:{} - {} Keywords: {})",
        cat.name,
        cat.description,
        cat.keywords.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_aliases() {
        assert_eq!(normalize("API"), "code_contract");
        assert_eq!(normalize(" style "), "codestyle");
        assert_eq!(normalize("Functions"), "function");
        assert_eq!(normalize("custom-cat"), "custom-cat");
    }
    #[test]
    fn enriches_known_category_content() {
        let out = enrich_content("codestyle", "Use snake_case.");
        assert!(out.contains("Use snake_case."));
        assert!(out.contains("[category:codestyle"));
        assert!(out.contains("Use-cases:"));
    }
    #[test]
    fn leaves_unknown_category_untouched() {
        assert_eq!(enrich_content("my-cat", "hello"), "hello");
        assert_eq!(enrich_query("hello", None), "hello");
    }
    #[test]
    fn expands_query_with_category_cues() {
        let out = enrich_query("naming", Some("codestyle"));
        assert!(out.contains("naming"));
        assert!(out.contains("category:codestyle"));
    }
}
