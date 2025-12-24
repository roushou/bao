//! Analyze phase - computes shared data from IR.

use eyre::Result;

use crate::{
    pipeline::{CompilationContext, Phase},
    schema::ComputedData,
};

/// Phase that computes shared analysis data from the Application IR.
///
/// This phase must run after `LowerPhase` as it requires the IR to be populated.
/// It computes commonly-needed data like context fields, command paths, and
/// async requirements.
pub struct AnalyzePhase;

impl Phase for AnalyzePhase {
    fn name(&self) -> &'static str {
        "analyze"
    }

    fn description(&self) -> &'static str {
        "Compute shared data from IR"
    }

    fn run(&self, ctx: &mut CompilationContext) -> Result<()> {
        let ir = ctx
            .ir
            .as_ref()
            .ok_or_else(|| eyre::eyre!("IR not set - AnalyzePhase must run after LowerPhase"))?;

        ctx.computed = Some(ComputedData::from_ir(ir));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use baobao_ir::{AppIR, AppMeta, DatabaseResource, DatabaseType, PoolConfig, Resource};
    use baobao_manifest::Manifest;

    use super::*;

    fn parse_manifest(content: &str) -> Manifest {
        toml::from_str(content).expect("Failed to parse test manifest")
    }

    fn make_test_manifest() -> Manifest {
        parse_manifest(
            r#"
            [cli]
            name = "test"
            language = "rust"
        "#,
        )
    }

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
    fn test_analyze_phase() {
        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);
        ctx.ir = Some(make_test_ir());

        assert!(ctx.computed.is_none());

        AnalyzePhase.run(&mut ctx).expect("analyze should succeed");

        assert!(ctx.computed.is_some());

        let computed = ctx.computed.as_ref().unwrap();
        assert!(computed.is_async);
        assert!(computed.has_database);
    }

    #[test]
    fn test_analyze_phase_requires_ir() {
        let manifest = make_test_manifest();
        let mut ctx = CompilationContext::new(manifest);
        // Don't set IR

        let result = AnalyzePhase.run(&mut ctx);
        assert!(result.is_err());
    }
}
