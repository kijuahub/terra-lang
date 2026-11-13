use terra_diagnostics::Span;

#[derive(Clone, Debug)]
pub struct File {
    pub package: Option<PathName>,
    pub uses: Vec<PathName>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct PathName {
    pub parts: Vec<String>,
    pub span: Span,
}

impl PathName {
    pub fn join(&self) -> String {
        self.parts.join(".")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vis {
    Default,
    Pub,
    Priv,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub is_static: bool,
    pub is_final: bool,
    pub is_async: bool,
    pub is_override: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mutability {
    Const,
    Var,
    Mut,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub vis: Vis,
    pub mods: Mods,
    pub kind: ItemKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ItemKind {
    Class(Class),
    Interface(Interface),
    Enum(EnumDecl),
    Function(Function),
    Field(Field),
    Exception { name: String },
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub extends: Option<TypeRef>,
    pub implements: Vec<TypeRef>,
    pub members: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub name: String,
    pub extends: Vec<TypeRef>,
    pub methods: Vec<Function>,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<(String, Span)>,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeRef>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub mutable: Mutability,
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeRef {
    pub path: Vec<String>,
    pub dims: u8,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Empty,
    Local {
        mutable: Mutability,
        name: String,
        ty: Option<TypeRef>,
        init: Option<Expr>,
        span: Span,
    },
    If {
        cond: Expr,
        then_branch: Block,
        else_branch: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    Loop {
        body: Block,
        span: Span,
    },
    ForC {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Block,
        span: Span,
    },
    ForIn {
        name: String,
        iter: Expr,
        body: Block,
        span: Span,
    },
    DoWhile {
        body: Block,
        cond: Expr,
        span: Span,
    },
    Match {
        scrut: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    Try {
        body: Block,
        catches: Vec<Catch>,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Throw {
        value: Expr,
        span: Span,
    },
    Delete {
        value: Expr,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
    Block(Block),
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Pattern {
    Wildcard { span: Span },
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Catch {
    pub name: String,
    pub ty: TypeRef,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Null,
    Name(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Assign {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Field {
        recv: Box<Expr>,
        name: String,
    },
    Index {
        recv: Box<Expr>,
        index: Box<Expr>,
    },
    New {
        ty: TypeRef,
        args: Vec<Expr>,
    },
    NewArray {
        ty: TypeRef,
        len: Box<Expr>,
    },
    Array {
        elems: Vec<Expr>,
    },
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
    },
    Await {
        expr: Box<Expr>,
    },
    SelfRef,
    SuperCall {
        args: Vec<Expr>,
    },
    SuperField {
        name: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
