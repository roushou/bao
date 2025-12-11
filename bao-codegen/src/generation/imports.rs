//! Import and dependency collection utilities.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;

/// Tracks imports and deduplicates them.
///
/// Maintains insertion order for deterministic output.
///
/// # Example
///
/// ```
/// use baobao_codegen::generation::ImportCollector;
///
/// let mut imports = ImportCollector::new();
/// imports.add("std::collections", "HashMap");
/// imports.add("std::collections", "HashSet");
/// imports.add("std::io", "Read");
///
/// // Render for Rust
/// for (module, symbols) in imports.iter() {
///     let symbols: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
///     println!("use {}::{{{}}};", module, symbols.join(", "));
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct ImportCollector {
    /// Module path -> set of symbols (sorted for deterministic output)
    imports: IndexMap<String, BTreeSet<String>>,
}

impl ImportCollector {
    /// Create a new empty import collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a symbol import from a module.
    pub fn add(&mut self, module: &str, symbol: &str) {
        self.imports
            .entry(module.to_string())
            .or_default()
            .insert(symbol.to_string());
    }

    /// Add a module import without specific symbols (e.g., `import * as foo`).
    pub fn add_module(&mut self, module: &str) {
        self.imports.entry(module.to_string()).or_default();
    }

    /// Merge another collector into this one.
    pub fn merge(&mut self, other: &ImportCollector) {
        for (module, symbols) in &other.imports {
            let entry = self.imports.entry(module.clone()).or_default();
            entry.extend(symbols.iter().cloned());
        }
    }

    /// Check if a module is already imported.
    pub fn has_module(&self, module: &str) -> bool {
        self.imports.contains_key(module)
    }

    /// Check if a specific symbol is imported from a module.
    pub fn has_symbol(&self, module: &str, symbol: &str) -> bool {
        self.imports
            .get(module)
            .is_some_and(|symbols| symbols.contains(symbol))
    }

    /// Iterate over all imports in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &BTreeSet<String>)> {
        self.imports.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Check if the collector is empty.
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
    }

    /// Get the number of modules.
    pub fn len(&self) -> usize {
        self.imports.len()
    }
}

/// Specification for a package dependency.
#[derive(Debug, Clone)]
pub struct DependencySpec {
    /// Version requirement (e.g., "1.0", "^2.0", ">=1.5")
    pub version: String,
    /// Optional features to enable
    pub features: Vec<String>,
    /// Whether this is an optional dependency
    pub optional: bool,
}

impl DependencySpec {
    /// Create a new dependency with just a version.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            features: Vec::new(),
            optional: false,
        }
    }

    /// Add features to enable.
    pub fn with_features(mut self, features: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    /// Mark as optional.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// Tracks package dependencies with versions and features.
///
/// # Example
///
/// ```
/// use baobao_codegen::generation::{DependencyCollector, DependencySpec};
///
/// let mut deps = DependencyCollector::new();
/// deps.add("serde", DependencySpec::new("1.0").with_features(["derive"]));
/// deps.add("tokio", DependencySpec::new("1").with_features(["rt-multi-thread", "macros"]));
///
/// for (name, spec) in deps.iter() {
///     println!("{} = \"{}\"", name, spec.version);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct DependencyCollector {
    deps: HashMap<String, DependencySpec>,
}

impl DependencyCollector {
    /// Create a new empty dependency collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dependency. If it already exists, this is a no-op.
    pub fn add(&mut self, name: impl Into<String>, spec: DependencySpec) {
        let name = name.into();
        self.deps.entry(name).or_insert(spec);
    }

    /// Add a simple dependency with just a version.
    pub fn add_simple(&mut self, name: impl Into<String>, version: impl Into<String>) {
        self.add(name, DependencySpec::new(version));
    }

    /// Check if a dependency exists.
    pub fn has(&self, name: &str) -> bool {
        self.deps.contains_key(name)
    }

    /// Get a dependency spec.
    pub fn get(&self, name: &str) -> Option<&DependencySpec> {
        self.deps.get(name)
    }

    /// Iterate over all dependencies.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DependencySpec)> {
        self.deps.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get dependencies sorted by name for deterministic output.
    pub fn sorted(&self) -> Vec<(&str, &DependencySpec)> {
        let mut deps: Vec<_> = self.iter().collect();
        deps.sort_by_key(|(name, _)| *name);
        deps
    }

    /// Check if the collector is empty.
    pub fn is_empty(&self) -> bool {
        self.deps.is_empty()
    }

    /// Get the number of dependencies.
    pub fn len(&self) -> usize {
        self.deps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_collector_basic() {
        let mut imports = ImportCollector::new();
        imports.add("std::io", "Read");
        imports.add("std::io", "Write");
        imports.add("std::collections", "HashMap");

        assert!(imports.has_module("std::io"));
        assert!(imports.has_symbol("std::io", "Read"));
        assert!(!imports.has_symbol("std::io", "Seek"));
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn test_import_collector_merge() {
        let mut a = ImportCollector::new();
        a.add("std::io", "Read");

        let mut b = ImportCollector::new();
        b.add("std::io", "Write");
        b.add("std::fs", "File");

        a.merge(&b);

        assert!(a.has_symbol("std::io", "Read"));
        assert!(a.has_symbol("std::io", "Write"));
        assert!(a.has_module("std::fs"));
    }

    #[test]
    fn test_dependency_collector() {
        let mut deps = DependencyCollector::new();
        deps.add_simple("serde", "1.0");
        deps.add(
            "tokio",
            DependencySpec::new("1").with_features(["rt-multi-thread"]),
        );

        assert!(deps.has("serde"));
        assert!(deps.has("tokio"));
        assert!(!deps.has("async-std"));

        let tokio = deps.get("tokio").unwrap();
        assert_eq!(tokio.features, vec!["rt-multi-thread"]);
    }
}
