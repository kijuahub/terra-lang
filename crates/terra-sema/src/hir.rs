use crate::ty::Ty;
use terra_diagnostics::Span;

#[derive(Clone, Debug)]
pub struct Program {
    pub functions: Vec<Function>,
    pub classes: Vec<Class>,
    pub interfaces: Vec<Interface>,
    pub enums: Vec<Enum>,
    pub static_inits: Vec<StaticInit>,
    pub entry_func: u32,
    pub entry_class: Option<u32>,
    pub entry_ctor: Option<u32>,
    pub entry_returns_int: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub arity: u16,
    pub locals: Vec<Local>,
    pub ret: Ty,
    pub is_async: bool,
    pub is_ctor: bool,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub package: String,
    pub super_id: Option<u32>,
    pub fields: Vec<Field>,
    pub vtable: Vec<u32>,
    pub itables: Vec<(u32, Vec<u32>)>,
    pub ctor: Option<u32>,
    pub is_exception: bool,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub name: String,
    pub package: String,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Enum {
    pub name: String,
    pub package: String,
    pub variants: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StaticInit {
    pub class: u32,
    pub field: u16,
    pub value: ConstValue,
}

#[derive(Clone, Debug)]
pub enum ConstValue {
    Int(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Null,
    Enum { id: u32, variant: u16 },
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub span: Span,
    pub kind: StmtKind,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Local {
        slot: u16,
        init: Option<Expr>,
    },
    Assign {
        place: Place,
        value: Expr,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    Loop {
        body: Vec<Stmt>,
    },
    ForC {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Vec<Stmt>,
    },
    ForIn {
        slot: u16,
        iter: Expr,
        body: Vec<Stmt>,
    },
    DoWhile {
        body: Vec<Stmt>,
        cond: Expr,
    },
    Match {
        scrut: Expr,
        arms: Vec<Arm>,
    },
    Try {
        body: Vec<Stmt>,
        catches: Vec<Catch>,
    },
    Return {
        value: Option<Expr>,
    },
    Break,
    Continue,
    Throw {
        value: Expr,
    },
    Delete {
        value: Expr,
    },
    Super {
        ctor: u32,
        args: Vec<Expr>,
    },
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub pat: Pat,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Pat {
    Wildcard,
    Value(ConstValue),
}

#[derive(Clone, Debug)]
pub struct Catch {
    pub slot: u16,
    pub class: u32,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub enum Place {
    Local(u16),
    Field { recv: Expr, index: u16 },
    Index { recv: Expr, index: Expr },
    Static { class: u32, index: u16 },
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub ty: Ty,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Null,
    Local(u16),
    Field {
        recv: Box<Expr>,
        index: u16,
    },
    StaticField {
        class: u32,
        index: u16,
    },
    Enum {
        enum_id: u32,
        variant: u16,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        kind: CallKind,
        args: Vec<Expr>,
    },
    New {
        class: u32,
        ctor: u32,
        args: Vec<Expr>,
    },
    NewArray {
        len: Box<Expr>,
    },
    Array {
        elems: Vec<Expr>,
    },
    Index {
        recv: Box<Expr>,
        index: Box<Expr>,
    },
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
    },
    Await {
        expr: Box<Expr>,
    },
}

#[derive(Clone, Debug)]
pub enum CallKind {
    Direct { func: u32, is_async: bool },
    Virtual { slot: u16, argc: u8, is_async: bool },
    Interface { iface: u32, slot: u16, argc: u8, is_async: bool },
    Builtin(Builtin),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Print,
    PrintLn,
    ToString,
    ToInt,
    ToFloat,
    ToDouble,
    Len,
    Host(u16),
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
