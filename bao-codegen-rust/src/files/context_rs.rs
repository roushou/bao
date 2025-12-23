use std::path::{Path, PathBuf};

use baobao_codegen::{
    adapters::{DatabaseAdapter, PoolInitInfo},
    builder::{FieldSpec, RenderOptions, StructSpec, StructureRenderer, TypeRef},
    schema::ContextFieldInfo,
};
use baobao_core::{FileRules, GeneratedFile};
use baobao_ir::{ContextFieldType, DatabaseType};

use super::GENERATED_HEADER;
use crate::{
    Fn, Impl, RawCode, RustFile, RustRenderer, RustStructureRenderer, Use, adapters::SqlxAdapter,
};

/// The context.rs file containing shared application state.
pub struct ContextRs {
    pub fields: Vec<ContextFieldInfo>,
}

impl ContextRs {
    pub fn new(fields: Vec<ContextFieldInfo>) -> Self {
        Self { fields }
    }

    fn build_struct(&self) -> String {
        let renderer = RustStructureRenderer::new();

        let mut spec = StructSpec::new("Context")
            .doc("Application context shared across all command handlers.");

        for field in &self.fields {
            let type_ref = Self::map_context_type_ref(&field.field_type);
            spec = spec.field(FieldSpec::new(&field.name, type_ref));
        }

        renderer.render_struct(&spec)
    }

    /// Map ContextFieldType to TypeRef.
    fn map_context_type_ref(field_type: &ContextFieldType) -> TypeRef {
        match field_type {
            ContextFieldType::Database(DatabaseType::Postgres) => TypeRef::named("sqlx::PgPool"),
            ContextFieldType::Database(DatabaseType::Mysql) => TypeRef::named("sqlx::MySqlPool"),
            ContextFieldType::Database(DatabaseType::Sqlite) => TypeRef::named("sqlx::SqlitePool"),
            ContextFieldType::Http => TypeRef::named("reqwest::Client"),
        }
    }

    fn build_impl(&self) -> Impl {
        let has_async = self.fields.iter().any(|f| f.is_async);
        let adapter = SqlxAdapter::new();
        let renderer = RustRenderer::new();

        let body = if self.fields.is_empty() {
            "Ok(Self {})".to_string()
        } else {
            let field_inits = self
                .fields
                .iter()
                .map(|f| {
                    let init_expr = self.generate_field_init(f, &adapter, &renderer);
                    format!("{}: {},", f.name, init_expr)
                })
                .collect::<Vec<_>>()
                .join("\n    ");
            format!("Ok(Self {{\n    {}\n}})", field_inits)
        };

        let new_fn = Fn::new("new")
            .returns("eyre::Result<Self>")
            .body(body)
            .async_if(has_async);

        Impl::new("Context").method(new_fn)
    }

    /// Generate initialization expression for a context field.
    fn generate_field_init(
        &self,
        field: &ContextFieldInfo,
        adapter: &SqlxAdapter,
        renderer: &RustRenderer,
    ) -> String {
        match field.field_type {
            ContextFieldType::Database(db_type) => {
                let info = PoolInitInfo {
                    field_name: field.name.clone(),
                    db_type,
                    env_var: field.env_var.clone(),
                    pool_config: field.pool.clone(),
                    sqlite_config: field.sqlite.clone(),
                };
                let value = adapter.pool_init(&info);
                value.render_with(renderer, &RenderOptions::default().with_indent(2))
            }
            ContextFieldType::Http => "reqwest::Client::new()".to_string(),
        }
    }
}

impl GeneratedFile for ContextRs {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("src").join("context.rs")
    }

    fn rules(&self) -> FileRules {
        FileRules::always_overwrite().with_header(GENERATED_HEADER)
    }

    fn render(&self) -> String {
        // Check if we need FromStr import (for SqliteConnectOptions::from_str)
        let needs_from_str = self.fields.iter().any(|f| {
            matches!(
                f.field_type,
                ContextFieldType::Database(DatabaseType::Sqlite)
            ) && (f.sqlite.as_ref().is_some_and(|s| s.has_config()) || f.pool.has_config())
        });

        let mut file = RustFile::new();

        if needs_from_str {
            file = file.use_stmt(Use::new("std::str").symbol("FromStr"));
        }

        file.add(RawCode::new(self.build_struct()))
            .add(self.build_impl())
            .render_with_header(GENERATED_HEADER)
    }
}
