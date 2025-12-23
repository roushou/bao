//! Computed data from IR analysis.
//!
//! This module provides [`ComputedData`], a struct that holds pre-computed
//! analysis results from the Application IR. This avoids repeated computation
//! across different parts of the code generation pipeline.

use std::collections::HashSet;

use baobao_ir::{AppIR, Operation, Resource};

use super::{
    ContextFieldInfo, collect_command_paths_from_ir, collect_commands_from_ir,
    collect_context_fields_from_ir,
};

/// Pre-computed data from IR analysis.
///
/// This struct aggregates commonly-needed analysis results that would otherwise
/// be computed multiple times by different generators and phases.
#[derive(Debug, Default, Clone)]
pub struct ComputedData {
    /// Context fields (database pools, HTTP clients, etc.)
    pub context_fields: Vec<ContextFieldInfo>,
    /// All command handler paths (for orphan detection)
    pub command_paths: HashSet<String>,
    /// Whether any resources require async initialization
    pub is_async: bool,
    /// Whether the application has database resources
    pub has_database: bool,
    /// Whether the application has HTTP client resources
    pub has_http: bool,
    /// Number of top-level commands
    pub command_count: usize,
    /// Total number of leaf commands (handlers)
    pub handler_count: usize,
}

impl ComputedData {
    /// Compute all data from an Application IR.
    pub fn from_ir(ir: &AppIR) -> Self {
        let context_fields = collect_context_fields_from_ir(ir);
        let command_paths: HashSet<String> =
            collect_command_paths_from_ir(ir).into_iter().collect();

        let is_async = ir
            .resources
            .iter()
            .any(|r| matches!(r, Resource::Database(_)));

        let has_database = ir
            .resources
            .iter()
            .any(|r| matches!(r, Resource::Database(_)));

        let has_http = ir
            .resources
            .iter()
            .any(|r| matches!(r, Resource::HttpClient(_)));

        let command_count = ir
            .operations
            .iter()
            .filter(|op| matches!(op, Operation::Command(_)))
            .count();

        let handler_count = command_paths.len();

        Self {
            context_fields,
            command_paths,
            is_async,
            has_database,
            has_http,
            command_count,
            handler_count,
        }
    }

    /// Get command info for all top-level commands.
    pub fn commands(&self, ir: &AppIR) -> Vec<super::CommandInfo> {
        collect_commands_from_ir(ir)
    }
}

#[cfg(test)]
mod tests {
    use baobao_ir::{AppMeta, DatabaseResource, DatabaseType, PoolConfig};

    use super::*;

    fn make_test_ir() -> AppIR {
        AppIR {
            meta: AppMeta {
                name: "test".into(),
                version: "1.0.0".into(),
                description: None,
                author: None,
            },
            resources: vec![Resource::Database(DatabaseResource {
                name: "db".into(),
                db_type: DatabaseType::Postgres,
                env_var: "DATABASE_URL".into(),
                pool: PoolConfig::default(),
                sqlite: None,
            })],
            operations: vec![],
        }
    }

    #[test]
    fn test_computed_data_from_ir() {
        let ir = make_test_ir();
        let computed = ComputedData::from_ir(&ir);

        assert!(computed.is_async);
        assert!(computed.has_database);
        assert!(!computed.has_http);
        assert_eq!(computed.context_fields.len(), 1);
    }

    #[test]
    fn test_computed_data_default() {
        let computed = ComputedData::default();

        assert!(!computed.is_async);
        assert!(!computed.has_database);
        assert!(!computed.has_http);
        assert!(computed.context_fields.is_empty());
    }
}
