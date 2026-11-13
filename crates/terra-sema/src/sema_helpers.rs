struct BindingRef {
    slot: u16,
    ty: Ty,
    mutable: bool,
    const_value: Option<ConstValue>,
}

fn primitive(name: &str) -> Option<Ty> {
    Some(match name {
        "void" => Ty::Void,
        "int" => Ty::Int,
        "float" => Ty::Float,
        "double" => Ty::Double,
        "bool" => Ty::Bool,
        "string" => Ty::String,
        _ => return None,
    })
}

fn is_entry_params(params: &[(String, Ty)]) -> bool {
    matches!(params, [(_, Ty::Array(t))] if matches!(t.as_ref(), Ty::String))
}

fn is_super_stmt(stmt: &ast::Stmt) -> bool {
    matches!(stmt, ast::Stmt::Expr { expr, .. } if matches!(expr.kind, ast::ExprKind::SuperCall { .. }))
}

fn stmt_span(stmt: &ast::Stmt) -> Span {
    match stmt {
        ast::Stmt::Empty => Span::new(0, 0, 0),
        ast::Stmt::Local { span, .. }
        | ast::Stmt::If { span, .. }
        | ast::Stmt::While { span, .. }
        | ast::Stmt::Loop { span, .. }
        | ast::Stmt::ForC { span, .. }
        | ast::Stmt::ForIn { span, .. }
        | ast::Stmt::DoWhile { span, .. }
        | ast::Stmt::Match { span, .. }
        | ast::Stmt::Try { span, .. }
        | ast::Stmt::Return { span, .. }
        | ast::Stmt::Break { span }
        | ast::Stmt::Continue { span }
        | ast::Stmt::Throw { span, .. }
        | ast::Stmt::Delete { span, .. }
        | ast::Stmt::Expr { span, .. } => *span,
        ast::Stmt::Block(b) => b.span,
    }
}

fn diverges(stmt: &Stmt) -> bool {
    match &stmt.kind {
        StmtKind::Return { .. } | StmtKind::Throw { .. } => true,
        StmtKind::If { then_body, else_body: Some(e), .. } => block_diverges(then_body) && block_diverges(e),
        StmtKind::Try { body, catches } => block_diverges(body) && catches.iter().all(|c| block_diverges(&c.body)),
        _ => false,
    }
}

fn block_diverges(stmts: &[Stmt]) -> bool {
    stmts.iter().any(diverges)
}

fn map_bin(op: AstBin) -> BinaryOp {
    match op {
        AstBin::Add => BinaryOp::Add,
        AstBin::Sub => BinaryOp::Sub,
        AstBin::Mul => BinaryOp::Mul,
        AstBin::Div => BinaryOp::Div,
        AstBin::Mod => BinaryOp::Mod,
        AstBin::Eq => BinaryOp::Eq,
        AstBin::Ne => BinaryOp::Ne,
        AstBin::Lt => BinaryOp::Lt,
        AstBin::Le => BinaryOp::Le,
        AstBin::Gt => BinaryOp::Gt,
        AstBin::Ge => BinaryOp::Ge,
        AstBin::And => BinaryOp::And,
        AstBin::Or => BinaryOp::Or,
    }
}

fn const_ty(v: &ConstValue) -> Ty {
    match v {
        ConstValue::Int(_) => Ty::Int,
        ConstValue::Float(_) => Ty::Float,
        ConstValue::Double(_) => Ty::Double,
        ConstValue::Bool(_) => Ty::Bool,
        ConstValue::Str(_) => Ty::String,
        ConstValue::Null => Ty::Null,
        ConstValue::Enum { id, .. } => Ty::Enum(*id),
    }
}

fn default_const(ty: &Ty) -> ConstValue {
    match ty {
        Ty::Int => ConstValue::Int(0),
        Ty::Float => ConstValue::Float(0.0),
        Ty::Double => ConstValue::Double(0.0),
        Ty::Bool => ConstValue::Bool(false),
        Ty::String => ConstValue::Str(String::new()),
        Ty::Enum(_) => ConstValue::Null,
        _ => ConstValue::Null,
    }
}

fn eval_bin(op: AstBin, l: ConstValue, r: ConstValue) -> Option<ConstValue> {
    use ConstValue::*;
    match (op, l, r) {
        (AstBin::Add, Int(a), Int(b)) => Some(Int(a.wrapping_add(b))),
        (AstBin::Sub, Int(a), Int(b)) => Some(Int(a.wrapping_sub(b))),
        (AstBin::Mul, Int(a), Int(b)) => Some(Int(a.wrapping_mul(b))),
        (AstBin::Div, Int(a), Int(b)) if b != 0 => Some(Int(a / b)),
        (AstBin::Mod, Int(a), Int(b)) if b != 0 => Some(Int(a % b)),
        (AstBin::Add, Str(a), Str(b)) => Some(Str(a + &b)),
        (AstBin::Eq, Int(a), Int(b)) => Some(Bool(a == b)),
        (AstBin::Ne, Int(a), Int(b)) => Some(Bool(a != b)),
        (AstBin::Lt, Int(a), Int(b)) => Some(Bool(a < b)),
        (AstBin::Le, Int(a), Int(b)) => Some(Bool(a <= b)),
        (AstBin::Gt, Int(a), Int(b)) => Some(Bool(a > b)),
        (AstBin::Ge, Int(a), Int(b)) => Some(Bool(a >= b)),
        (AstBin::And, Bool(a), Bool(b)) => Some(Bool(a && b)),
        (AstBin::Or, Bool(a), Bool(b)) => Some(Bool(a || b)),
        _ => None,
    }
}

fn flatten(expr: &ast::Expr) -> Option<Vec<String>> {
    match &expr.kind {
        ast::ExprKind::Name(n) => Some(vec![n.clone()]),
        ast::ExprKind::Field { recv, name } => {
            let mut p = flatten(recv)?;
            p.push(name.clone());
            Some(p)
        }
        _ => None,
    }
}

fn host_accepts(expect: terra_bytecode::HostTy, got: &Ty) -> bool {
    if got.is_error() || matches!(got, Ty::Any) {
        return true;
    }
    match expect {
        terra_bytecode::HostTy::Any => !matches!(got, Ty::Void),
        terra_bytecode::HostTy::Void => false,
        terra_bytecode::HostTy::Int => matches!(got, Ty::Int),
        terra_bytecode::HostTy::Float => matches!(got, Ty::Float),
        terra_bytecode::HostTy::Double => matches!(got, Ty::Double),
        terra_bytecode::HostTy::Bool => matches!(got, Ty::Bool),
        terra_bytecode::HostTy::String => matches!(got, Ty::String),
    }
}

fn builtin_name(b: Builtin) -> String {
    match b {
        Builtin::Print => "print".into(),
        Builtin::PrintLn => "println".into(),
        Builtin::ToString => "to_string".into(),
        Builtin::ToInt => "to_int".into(),
        Builtin::ToFloat => "to_float".into(),
        Builtin::ToDouble => "to_double".into(),
        Builtin::Len => "len".into(),
        Builtin::Host(id) => terra_bytecode::host(id)
            .map(|f| f.name.to_string())
            .unwrap_or_else(|| "host".into()),
    }
}

trait WithTy {
    fn with_ty(self, ty: Ty) -> Self;
}

impl WithTy for Expr {
    fn with_ty(mut self, ty: Ty) -> Self {
        self.ty = ty;
        self
    }
}


