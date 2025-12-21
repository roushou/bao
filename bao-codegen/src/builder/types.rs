//! Language-agnostic type system for code generation.
//!
//! This module provides abstractions for representing types in a way that
//! can be rendered to any target language via the [`TypeMapper`] trait.

/// A language-agnostic type reference.
///
/// Types are represented semantically and can be rendered differently
/// per target language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// A primitive type (string, int, bool, etc.).
    Primitive(PrimitiveType),
    /// An optional/nullable type.
    Optional(Box<TypeRef>),
    /// An array/list/vector type.
    Array(Box<TypeRef>),
    /// A named type (custom struct, class, interface, etc.).
    Named(String),
    /// A generic type with type arguments.
    Generic {
        /// Base type name (e.g., "HashMap", "Map", "Record").
        base: String,
        /// Type arguments (e.g., [String, Int] for HashMap<String, i64>).
        args: Vec<TypeRef>,
    },
    /// A reference type (Rust: &T, others: usually no-op).
    Ref(Box<TypeRef>),
    /// A mutable reference type (Rust: &mut T).
    RefMut(Box<TypeRef>),
    /// Unit/void type.
    Unit,
    /// Result type with Ok and Err types.
    Result {
        /// The success type.
        ok: Box<TypeRef>,
        /// The error type.
        err: Box<TypeRef>,
    },
}

impl TypeRef {
    /// Create a primitive type reference.
    pub fn primitive(ty: PrimitiveType) -> Self {
        Self::Primitive(ty)
    }

    /// Create an optional type reference.
    pub fn optional(inner: TypeRef) -> Self {
        Self::Optional(Box::new(inner))
    }

    /// Create an array type reference.
    pub fn array(inner: TypeRef) -> Self {
        Self::Array(Box::new(inner))
    }

    /// Create a named type reference.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }

    /// Create a generic type reference.
    pub fn generic(base: impl Into<String>, args: Vec<TypeRef>) -> Self {
        Self::Generic {
            base: base.into(),
            args,
        }
    }

    /// Create a reference type.
    pub fn ref_(inner: TypeRef) -> Self {
        Self::Ref(Box::new(inner))
    }

    /// Create a mutable reference type.
    pub fn ref_mut(inner: TypeRef) -> Self {
        Self::RefMut(Box::new(inner))
    }

    /// Create a unit type.
    pub fn unit() -> Self {
        Self::Unit
    }

    /// Create a Result type.
    pub fn result(ok: TypeRef, err: TypeRef) -> Self {
        Self::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        }
    }

    /// Convenience: String type.
    pub fn string() -> Self {
        Self::Primitive(PrimitiveType::String)
    }

    /// Convenience: Int type.
    pub fn int() -> Self {
        Self::Primitive(PrimitiveType::Int)
    }

    /// Convenience: Float type.
    pub fn float() -> Self {
        Self::Primitive(PrimitiveType::Float)
    }

    /// Convenience: Bool type.
    pub fn bool() -> Self {
        Self::Primitive(PrimitiveType::Bool)
    }

    /// Convenience: Path type.
    pub fn path() -> Self {
        Self::Primitive(PrimitiveType::Path)
    }

    /// Convenience: Duration type.
    pub fn duration() -> Self {
        Self::Primitive(PrimitiveType::Duration)
    }

    /// Check if this type is optional.
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional(_))
    }

    /// Get the inner type for wrapper types (Optional, Array, Ref, RefMut).
    ///
    /// Returns `None` for non-wrapper types.
    pub fn inner_type(&self) -> Option<&TypeRef> {
        match self {
            Self::Optional(inner) | Self::Array(inner) | Self::Ref(inner) | Self::RefMut(inner) => {
                Some(inner)
            }
            _ => None,
        }
    }
}

/// Primitive types supported across languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    /// String type (Rust: String, TS: string).
    String,
    /// Integer type (Rust: i64, TS: number, Go: int64).
    Int,
    /// Unsigned integer (Rust: u64, TS: number, Go: uint64).
    UInt,
    /// Float type (Rust: f64, TS: number, Go: float64).
    Float,
    /// Boolean type (Rust: bool, TS: boolean, Go: bool).
    Bool,
    /// Path type (Rust: PathBuf, TS: string, Go: string).
    Path,
    /// Duration type (Rust: Duration, TS: number, Go: time.Duration).
    Duration,
    /// Character type (Rust: char, TS: string).
    Char,
    /// Byte type (Rust: u8, TS: number, Go: byte).
    Byte,
}

impl PrimitiveType {
    /// Get the canonical name of this primitive.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Int => "int",
            Self::UInt => "uint",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Path => "path",
            Self::Duration => "duration",
            Self::Char => "char",
            Self::Byte => "byte",
        }
    }
}

/// Visibility/access level for types and members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// Public (Rust: pub, TS: export, Go: capitalized).
    #[default]
    Public,
    /// Private (Rust: no modifier, TS: no export, Go: lowercase).
    Private,
    /// Crate-level visibility (Rust: pub(crate)).
    Crate,
    /// Module-level visibility (Rust: pub(super)).
    Super,
}

impl Visibility {
    /// Check if this is a public visibility.
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }

    /// Check if this is a private visibility.
    pub fn is_private(&self) -> bool {
        matches!(self, Self::Private)
    }
}

/// Trait for mapping types to language-specific representations.
///
/// Implement this trait to support a new target language's type system.
pub trait TypeMapper {
    /// Map a primitive type to the target language.
    fn map_primitive(&self, ty: PrimitiveType) -> String;

    /// Map an optional type (e.g., `Option<T>`, `T | undefined`).
    fn map_optional(&self, inner: &str) -> String;

    /// Map an array type (e.g., `Vec<T>`, `T[]`).
    fn map_array(&self, inner: &str) -> String;

    /// Map a reference type (Rust: `&T`, others usually no-op).
    fn map_ref(&self, inner: &str) -> String {
        inner.to_string()
    }

    /// Map a mutable reference type (Rust: `&mut T`).
    fn map_ref_mut(&self, inner: &str) -> String {
        inner.to_string()
    }

    /// Map a generic type with arguments.
    fn map_generic(&self, base: &str, args: &[String]) -> String {
        if args.is_empty() {
            base.to_string()
        } else {
            format!("{}<{}>", base, args.join(", "))
        }
    }

    /// Map a Result type (Rust: `Result<T, E>`, TS: `T`, Go: `(T, error)`).
    fn map_result(&self, ok: &str, err: &str) -> String;

    /// Map the unit type (Rust: `()`, TS: `void`, Go: empty).
    fn map_unit(&self) -> String;

    /// Render a complete TypeRef to a string.
    fn render_type(&self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Primitive(p) => self.map_primitive(*p),
            TypeRef::Optional(inner) => {
                let inner_str = self.render_type(inner);
                self.map_optional(&inner_str)
            }
            TypeRef::Array(inner) => {
                let inner_str = self.render_type(inner);
                self.map_array(&inner_str)
            }
            TypeRef::Named(name) => name.clone(),
            TypeRef::Generic { base, args } => {
                let arg_strs: Vec<_> = args.iter().map(|a| self.render_type(a)).collect();
                self.map_generic(base, &arg_strs)
            }
            TypeRef::Ref(inner) => {
                let inner_str = self.render_type(inner);
                self.map_ref(&inner_str)
            }
            TypeRef::RefMut(inner) => {
                let inner_str = self.render_type(inner);
                self.map_ref_mut(&inner_str)
            }
            TypeRef::Unit => self.map_unit(),
            TypeRef::Result { ok, err } => {
                let ok_str = self.render_type(ok);
                let err_str = self.render_type(err);
                self.map_result(&ok_str, &err_str)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_type_as_str() {
        assert_eq!(PrimitiveType::String.as_str(), "string");
        assert_eq!(PrimitiveType::Int.as_str(), "int");
        assert_eq!(PrimitiveType::Bool.as_str(), "bool");
    }

    #[test]
    fn test_type_ref_constructors() {
        let string = TypeRef::string();
        assert_eq!(string, TypeRef::Primitive(PrimitiveType::String));

        let opt_string = TypeRef::optional(TypeRef::string());
        assert!(matches!(opt_string, TypeRef::Optional(_)));

        let arr_int = TypeRef::array(TypeRef::int());
        assert!(matches!(arr_int, TypeRef::Array(_)));

        let named = TypeRef::named("Context");
        assert_eq!(named, TypeRef::Named("Context".into()));
    }

    #[test]
    fn test_visibility() {
        assert!(Visibility::Public.is_public());
        assert!(!Visibility::Public.is_private());
        assert!(Visibility::Private.is_private());
        assert!(!Visibility::Private.is_public());
    }

    #[test]
    fn test_generic_type() {
        let map = TypeRef::generic("HashMap", vec![TypeRef::string(), TypeRef::int()]);
        assert!(
            matches!(map, TypeRef::Generic { base, args } if base == "HashMap" && args.len() == 2)
        );
    }
}
