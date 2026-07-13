use jisp_core::Span;

#[derive(Clone, Debug)]
pub struct Module {
    pub imports: Vec<Import>,
    pub types: Vec<TypeDecl>,
    pub definitions: Vec<Definition>,
    pub exports: Vec<String>,
    /// Entry points for a host-managed, reducer-driven UI application.
    pub ui_app: Option<UiApp>,
}

impl Module {
    pub fn empty() -> Self {
        Self {
            imports: vec![],
            types: vec![],
            definitions: vec![],
            exports: vec![],
            ui_app: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UiApp {
    pub init: String,
    pub reduce: String,
    pub view: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Import {
    pub alias: String,
    pub path: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub name: String,
    pub public: bool,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeDecl {
    pub name: String,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct VariantDecl {
    pub name: String,
    pub field_types: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub const fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn null(span: Span) -> Self {
        Self::new(ExprKind::Literal(Literal::Null), span)
    }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(Literal),
    Name(String),
    Lambda {
        params: Vec<String>,
        rest: Option<String>,
        body: Box<Expr>,
    },
    Let {
        bindings: Vec<(String, Expr)>,
        body: Box<Expr>,
    },
    Do(Vec<Expr>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    List(Vec<Expr>),
    Object(Vec<(Expr, Expr)>),
    Field {
        object: Box<Expr>,
        key: Box<Expr>,
    },
    StringTemplate {
        lines: bool,
        parts: Vec<StringPart>,
    },
    Case {
        subject: Box<Expr>,
        branches: Vec<CaseBranch>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug)]
pub enum StringPart {
    Literal(String),
    Expr(Expr),
    Splice(Expr),
}

#[derive(Clone, Debug)]
pub struct CaseBranch {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Wildcard,
    Bind(String),
    Alias {
        pattern: Box<Pattern>,
        name: String,
    },
    Or(Vec<Pattern>),
    Literal(Literal),
    Variant {
        tag: String,
        fields: Vec<Pattern>,
    },
    List {
        prefix: Vec<Pattern>,
        rest: Option<String>,
    },
    Object(Vec<(String, Pattern)>),
}
