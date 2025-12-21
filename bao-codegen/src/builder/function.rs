//! Language-agnostic function definitions.
//!
//! This module provides declarative specifications for functions, methods,
//! and their components that can be rendered to any target language.

use super::{
    expr::Value,
    types::{TypeRef, Visibility},
};

/// A declarative specification for a function or method.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSpec {
    /// Function name.
    pub name: String,
    /// Documentation comment.
    pub doc: Option<String>,
    /// Parameters.
    pub params: Vec<ParamSpec>,
    /// Return type (None for void/unit).
    pub return_type: Option<TypeRef>,
    /// Whether this function is async.
    pub is_async: bool,
    /// Function body as statements.
    pub body: Vec<Statement>,
    /// Visibility modifier.
    pub visibility: Visibility,
    /// Generic type parameters.
    pub generics: Vec<GenericParam>,
    /// Whether this is a method (has self/this).
    pub receiver: Option<Receiver>,
}

impl FunctionSpec {
    /// Create a new public function spec.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            params: Vec::new(),
            return_type: None,
            is_async: false,
            body: Vec::new(),
            visibility: Visibility::Public,
            generics: Vec::new(),
            receiver: None,
        }
    }

    /// Set documentation comment.
    pub fn doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Add a parameter.
    pub fn param(mut self, param: ParamSpec) -> Self {
        self.params.push(param);
        self
    }

    /// Add multiple parameters.
    pub fn params(mut self, params: impl IntoIterator<Item = ParamSpec>) -> Self {
        self.params.extend(params);
        self
    }

    /// Set return type.
    pub fn returns(mut self, ty: TypeRef) -> Self {
        self.return_type = Some(ty);
        self
    }

    /// Mark as async.
    pub fn async_(mut self) -> Self {
        self.is_async = true;
        self
    }

    /// Add a statement to the body.
    pub fn statement(mut self, stmt: Statement) -> Self {
        self.body.push(stmt);
        self
    }

    /// Add multiple statements to the body.
    pub fn statements(mut self, stmts: impl IntoIterator<Item = Statement>) -> Self {
        self.body.extend(stmts);
        self
    }

    /// Set visibility.
    pub fn visibility(mut self, vis: Visibility) -> Self {
        self.visibility = vis;
        self
    }

    /// Make this function private.
    pub fn private(mut self) -> Self {
        self.visibility = Visibility::Private;
        self
    }

    /// Add a generic type parameter.
    pub fn generic(mut self, param: GenericParam) -> Self {
        self.generics.push(param);
        self
    }

    /// Set the receiver (makes this a method).
    pub fn receiver(mut self, recv: Receiver) -> Self {
        self.receiver = Some(recv);
        self
    }

    /// Make this an immutable self method.
    pub fn method(self) -> Self {
        self.receiver(Receiver::Ref)
    }

    /// Make this a mutable self method.
    pub fn method_mut(self) -> Self {
        self.receiver(Receiver::RefMut)
    }

    /// Check if this function has a body.
    pub fn has_body(&self) -> bool {
        !self.body.is_empty()
    }
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: TypeRef,
    /// Default value (if optional).
    pub default: Option<Value>,
    /// Whether this parameter is variadic (...args in TS, ... in Rust).
    pub variadic: bool,
}

impl ParamSpec {
    /// Create a new required parameter.
    pub fn new(name: impl Into<String>, ty: TypeRef) -> Self {
        Self {
            name: name.into(),
            ty,
            default: None,
            variadic: false,
        }
    }

    /// Set a default value.
    pub fn default(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    /// Mark as variadic.
    pub fn variadic(mut self) -> Self {
        self.variadic = true;
        self
    }
}

/// Method receiver type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// Owned receiver: `self` in Rust, `this` in TS.
    Owned,
    /// Immutable reference: `&self` in Rust.
    Ref,
    /// Mutable reference: `&mut self` in Rust.
    RefMut,
}

/// A generic type parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    /// Parameter name (e.g., "T", "E").
    pub name: String,
    /// Trait bounds (Rust) or constraints (TS extends).
    pub bounds: Vec<String>,
    /// Default type.
    pub default: Option<TypeRef>,
}

impl GenericParam {
    /// Create a new generic parameter.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bounds: Vec::new(),
            default: None,
        }
    }

    /// Add a trait bound.
    pub fn bound(mut self, bound: impl Into<String>) -> Self {
        self.bounds.push(bound.into());
        self
    }

    /// Set a default type.
    pub fn default(mut self, ty: TypeRef) -> Self {
        self.default = Some(ty);
        self
    }
}

/// A statement in a function body.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Variable declaration: `let x = ...` or `const x = ...`.
    Let {
        /// Variable name.
        name: String,
        /// Optional explicit type annotation.
        ty: Option<TypeRef>,
        /// Initial value.
        value: Value,
        /// Whether the binding is mutable.
        mutable: bool,
    },
    /// Return statement.
    Return(Option<Value>),
    /// Expression statement.
    Expr(Value),
    /// If statement.
    If {
        /// Condition expression.
        condition: Value,
        /// Then branch.
        then_branch: Vec<Statement>,
        /// Else branch.
        else_branch: Option<Vec<Statement>>,
    },
    /// Match/switch statement.
    Match {
        /// Expression to match.
        expr: Value,
        /// Match arms.
        arms: Vec<MatchArm>,
    },
    /// For loop.
    For {
        /// Loop variable name.
        var: String,
        /// Iterator/iterable expression.
        iter: Value,
        /// Loop body.
        body: Vec<Statement>,
    },
    /// While loop.
    While {
        /// Condition.
        condition: Value,
        /// Loop body.
        body: Vec<Statement>,
    },
    /// Raw code (escape hatch for language-specific code).
    Raw(String),
    /// Block of statements.
    Block(Vec<Statement>),
}

impl Statement {
    /// Create a let binding.
    pub fn let_(name: impl Into<String>, value: Value) -> Self {
        Self::Let {
            name: name.into(),
            ty: None,
            value,
            mutable: false,
        }
    }

    /// Create a mutable let binding.
    pub fn let_mut(name: impl Into<String>, value: Value) -> Self {
        Self::Let {
            name: name.into(),
            ty: None,
            value,
            mutable: true,
        }
    }

    /// Create a typed let binding.
    pub fn let_typed(name: impl Into<String>, ty: TypeRef, value: Value) -> Self {
        Self::Let {
            name: name.into(),
            ty: Some(ty),
            value,
            mutable: false,
        }
    }

    /// Create a return statement.
    pub fn return_(value: Value) -> Self {
        Self::Return(Some(value))
    }

    /// Create an empty return statement.
    pub fn return_void() -> Self {
        Self::Return(None)
    }

    /// Create an expression statement.
    pub fn expr(value: Value) -> Self {
        Self::Expr(value)
    }

    /// Create a raw code statement.
    pub fn raw(code: impl Into<String>) -> Self {
        Self::Raw(code.into())
    }

    /// Create an if statement.
    pub fn if_(condition: Value, then_branch: Vec<Statement>) -> Self {
        Self::If {
            condition,
            then_branch,
            else_branch: None,
        }
    }

    /// Create an if-else statement.
    pub fn if_else(
        condition: Value,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    ) -> Self {
        Self::If {
            condition,
            then_branch,
            else_branch: Some(else_branch),
        }
    }

    /// Create a match statement.
    pub fn match_(expr: Value, arms: Vec<MatchArm>) -> Self {
        Self::Match { expr, arms }
    }

    /// Create a for loop.
    pub fn for_(var: impl Into<String>, iter: Value, body: Vec<Statement>) -> Self {
        Self::For {
            var: var.into(),
            iter,
            body,
        }
    }

    /// Create a while loop.
    pub fn while_(condition: Value, body: Vec<Statement>) -> Self {
        Self::While { condition, body }
    }

    /// Create a block of statements.
    pub fn block(stmts: Vec<Statement>) -> Self {
        Self::Block(stmts)
    }
}

/// A match arm (case in switch).
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Pattern to match.
    pub pattern: Pattern,
    /// Optional guard condition.
    pub guard: Option<Value>,
    /// Arm body.
    pub body: Vec<Statement>,
}

impl MatchArm {
    /// Create a new match arm.
    pub fn new(pattern: Pattern, body: Vec<Statement>) -> Self {
        Self {
            pattern,
            guard: None,
            body,
        }
    }

    /// Add a guard condition.
    pub fn guard(mut self, condition: Value) -> Self {
        self.guard = Some(condition);
        self
    }
}

/// A pattern for matching.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Wildcard pattern: `_` or `default`.
    Wildcard,
    /// Literal pattern.
    Literal(Value),
    /// Variable binding pattern.
    Binding(String),
    /// Enum variant pattern.
    Variant {
        /// Variant path (e.g., "Option::Some").
        path: String,
        /// Bound fields/values.
        fields: Vec<Pattern>,
    },
    /// Tuple pattern.
    Tuple(Vec<Pattern>),
    /// Struct pattern.
    Struct {
        /// Struct name.
        name: String,
        /// Field patterns.
        fields: Vec<(String, Pattern)>,
    },
}

impl Pattern {
    /// Create a wildcard pattern.
    pub fn wildcard() -> Self {
        Self::Wildcard
    }

    /// Create a literal pattern.
    pub fn literal(value: Value) -> Self {
        Self::Literal(value)
    }

    /// Create a binding pattern.
    pub fn binding(name: impl Into<String>) -> Self {
        Self::Binding(name.into())
    }

    /// Create a variant pattern.
    pub fn variant(path: impl Into<String>, fields: Vec<Pattern>) -> Self {
        Self::Variant {
            path: path.into(),
            fields,
        }
    }

    /// Create a tuple pattern.
    pub fn tuple(patterns: Vec<Pattern>) -> Self {
        Self::Tuple(patterns)
    }
}

/// Trait for rendering function specs to language-specific code.
///
/// Implement this trait to support rendering functions and methods
/// in a new target language.
pub trait FunctionRenderer {
    /// Render a function specification to code.
    fn render_function(&self, spec: &FunctionSpec) -> String;

    /// Render a parameter specification to code.
    fn render_param(&self, spec: &ParamSpec) -> String;

    /// Render a statement to code.
    fn render_statement(&self, stmt: &Statement, indent: usize) -> String;

    /// Render a match arm to code.
    fn render_match_arm(&self, arm: &MatchArm, indent: usize) -> String;

    /// Render a pattern to code.
    fn render_pattern(&self, pattern: &Pattern) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_spec() {
        let spec = FunctionSpec::new("process")
            .doc("Process the input")
            .async_()
            .param(ParamSpec::new("input", TypeRef::string()))
            .returns(TypeRef::result(TypeRef::unit(), TypeRef::named("Error")))
            .statement(Statement::return_(Value::ident("Ok(())")));

        assert_eq!(spec.name, "process");
        assert!(spec.is_async);
        assert_eq!(spec.params.len(), 1);
        assert!(spec.return_type.is_some());
        assert!(spec.has_body());
    }

    #[test]
    fn test_method_spec() {
        let spec = FunctionSpec::new("get_name")
            .method()
            .returns(TypeRef::string());

        assert!(matches!(spec.receiver, Some(Receiver::Ref)));
    }

    #[test]
    fn test_param_spec() {
        let required = ParamSpec::new("name", TypeRef::string());
        assert!(required.default.is_none());

        let optional = ParamSpec::new("count", TypeRef::int()).default(Value::int(10));
        assert!(optional.default.is_some());
    }

    #[test]
    fn test_statements() {
        let let_stmt = Statement::let_("x", Value::int(42));
        assert!(matches!(let_stmt, Statement::Let { mutable: false, .. }));

        let let_mut = Statement::let_mut("y", Value::int(0));
        assert!(matches!(let_mut, Statement::Let { mutable: true, .. }));

        let ret = Statement::return_(Value::ident("result"));
        assert!(matches!(ret, Statement::Return(Some(_))));
    }

    #[test]
    fn test_match_arm() {
        let arm = MatchArm::new(
            Pattern::variant("Some", vec![Pattern::binding("value")]),
            vec![Statement::return_(Value::ident("value"))],
        )
        .guard(Value::ident("value > 0"));

        assert!(arm.guard.is_some());
        assert!(matches!(arm.pattern, Pattern::Variant { .. }));
    }

    #[test]
    fn test_generic_param() {
        let generic = GenericParam::new("T")
            .bound("Clone")
            .bound("Send")
            .default(TypeRef::string());

        assert_eq!(generic.name, "T");
        assert_eq!(generic.bounds.len(), 2);
        assert!(generic.default.is_some());
    }
}
