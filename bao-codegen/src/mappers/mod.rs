//! Language-specific type mappers.

mod rust;
mod typescript;

pub use rust::RustTypeMapper;
pub use typescript::TypeScriptTypeMapper;
