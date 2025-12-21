//! context.ts generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_codegen::{
    builder::{FieldSpec, StructSpec, StructureRenderer, TypeRef},
    schema::ContextFieldInfo,
};
use baobao_core::{ContextFieldType, DatabaseType, FileRules, GeneratedFile};

use super::GENERATED_HEADER;
use crate::{
    TypeScriptStructureRenderer,
    ast::Import,
    code_file::{CodeFile, RawCode},
};

/// The context.ts file containing shared application state.
pub struct ContextTs {
    pub fields: Vec<ContextFieldInfo>,
}

impl ContextTs {
    pub fn new(fields: Vec<ContextFieldInfo>) -> Self {
        Self { fields }
    }

    fn needs_sqlite(&self) -> bool {
        self.fields.iter().any(|f| {
            matches!(
                f.field_type,
                ContextFieldType::Database(DatabaseType::Sqlite)
            )
        })
    }

    fn build_imports(&self) -> Vec<Import> {
        let mut imports = Vec::new();
        if self.needs_sqlite() {
            imports.push(Import::new("bun:sqlite").named("Database"));
        }
        imports
    }

    fn build_context_type(&self) -> String {
        let renderer = TypeScriptStructureRenderer::new();

        let mut spec = StructSpec::new("Context");

        for field in &self.fields {
            let type_ref = Self::map_context_type_ref(&field.field_type);
            spec = spec.field(FieldSpec::new(&field.name, type_ref));
        }

        renderer.render_struct(&spec)
    }

    /// Map ContextFieldType to TypeRef.
    fn map_context_type_ref(field_type: &ContextFieldType) -> TypeRef {
        match field_type {
            ContextFieldType::Database(DatabaseType::Sqlite) => TypeRef::named("Database"),
            ContextFieldType::Database(DatabaseType::Postgres) => TypeRef::named("unknown"),
            ContextFieldType::Database(DatabaseType::Mysql) => TypeRef::named("unknown"),
            ContextFieldType::Http => TypeRef::named("unknown"),
        }
    }
}

impl GeneratedFile for ContextTs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("context.ts")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        CodeFile::new()
            .add(RawCode::new(GENERATED_HEADER))
            .imports(self.build_imports())
            .add(RawCode::new(self.build_context_type()))
            .render()
    }
}
