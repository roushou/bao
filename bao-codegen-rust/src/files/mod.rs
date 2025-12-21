// Re-export from bao-core for backwards compatibility
pub use baobao_core::GENERATED_HEADER;

use crate::Use;

/// Common use statement helpers for generated files.
pub mod uses {
    use super::Use;

    /// `use crate::context::Context;`
    pub fn context() -> Use {
        Use::new("crate::context").symbol("Context")
    }

    /// `use clap::Parser;`
    pub fn clap_parser() -> Use {
        Use::new("clap").symbol("Parser")
    }

    /// `use clap::{Parser, Subcommand};`
    pub fn clap_parser_subcommand() -> Use {
        Use::new("clap").symbols(["Parser", "Subcommand"])
    }
}

mod app_rs;
mod cargo_toml;
mod cli_rs;
mod command_rs;
mod commands_mod;
mod context_rs;
mod generated_mod;
mod gitignore;
mod handler_stub;
mod handlers_mod;
mod main_rs;

pub use app_rs::AppRs;
pub use baobao_codegen::generation::BaoToml;
pub use cargo_toml::CargoToml;
pub use cli_rs::CliRs;
pub use command_rs::CommandRs;
pub use commands_mod::CommandsMod;
pub use context_rs::ContextRs;
pub use generated_mod::GeneratedMod;
pub use gitignore::GitIgnore;
pub use handler_stub::{HandlerStub, STUB_MARKER};
pub use handlers_mod::HandlersMod;
pub use main_rs::MainRs;
