//! Lower typed IR to a stack-machine module. Function ids stay in HIR order.
//! The last function is `$entry`: it loads the argument array, constructs the
//! entry class when `main` is an instance method, and returns an int.

use terra_bytecode::{
    ClassInfo, Constant, EnumInfo, Function, ITable, InterfaceInfo, Module, Op, SourceLoc, StaticInit,
};
use terra_diagnostics::{Lang, SourceFile, Span};
use terra_sema::{
    BinaryOp, Builtin, CallKind, ConstValue, Expr, ExprKind, Pat, Place, Program, Stmt, StmtKind, Ty,
    UnaryOp,
};

struct Loop {
    cont_ip: Option<u32>,
    cont_holes: Vec<usize>,
    break_holes: Vec<usize>,
}

struct Gen<'a> {
    program: &'a Program,
    sources: &'a [SourceFile],
    constants: Vec<Constant>,
    code: Vec<Op>,
    locs: Vec<SourceLoc>,
    local_count: u16,
    local_names: Vec<String>,
    loops: Vec<Loop>,
}

pub fn emit(program: &Program, sources: &[SourceFile], lang: Lang) -> Module {
    let mut g = Gen {
        program,
        sources,
        constants: Vec::new(),
        code: Vec::new(),
        locs: Vec::new(),
        local_count: 0,
        local_names: Vec::new(),
        loops: Vec::new(),
    };
    let mut functions = Vec::new();
    for f in &program.functions {
        functions.push(g.function(f));
    }
    functions.push(g.entry_thunk());
    let entry = (functions.len() - 1) as u32;
    Module {
        files: sources.iter().map(|s| s.name.clone()).collect(),
        constants: g.constants,
        functions,
        classes: program
            .classes
            .iter()
            .map(|c| ClassInfo {
                name: c.name.clone(),
                package: c.package.clone(),
                super_id: c.super_id,
                field_names: c.fields.iter().map(|f| f.name.clone()).collect(),
                vtable: c.vtable.clone(),
                itables: c
                    .itables
                    .iter()
                    .map(|(iface, methods)| ITable {
                        iface: *iface,
                        methods: methods.clone(),
                    })
                    .collect(),
                ctor: c.ctor,
                is_exception: c.is_exception,
            })
            .collect(),
        interfaces: program
            .interfaces
            .iter()
            .map(|i| InterfaceInfo {
                name: i.name.clone(),
                package: i.package.clone(),
                methods: i.methods.clone(),
            })
            .collect(),
        enums: program
            .enums
            .iter()
            .map(|e| EnumInfo {
                name: e.name.clone(),
                package: e.package.clone(),
                variants: e.variants.clone(),
            })
            .collect(),
        static_inits: program
            .static_inits
            .iter()
            .map(|s| StaticInit {
                class: s.class,
                field: s.field,
                value: const_to_bc(&s.value),
            })
            .collect(),
        entry,
        global_count: 1,
        lang: match lang {
            Lang::Ru => "ru".into(),
            Lang::En => "en".into(),
        },
    }
}

fn const_to_bc(v: &ConstValue) -> Constant {
    match v {
        ConstValue::Int(n) => Constant::Int(*n),
        ConstValue::Float(n) => Constant::Float(*n),
        ConstValue::Double(n) => Constant::Double(*n),
        ConstValue::Bool(b) => Constant::Bool(*b),
        ConstValue::Str(s) => Constant::Str(s.clone()),
        ConstValue::Null => Constant::Null,
        ConstValue::Enum { id, variant } => Constant::Enum {
            id: *id,
            variant: *variant,
        },
    }
}

impl Gen<'_> {
    fn function(&mut self, f: &terra_sema::Function) -> Function {
        self.code.clear();
        self.locs.clear();
        self.loops.clear();
        self.local_count = f.locals.len() as u16;
        self.local_names = f.locals.iter().map(|l| l.name.clone()).collect();
        for stmt in &f.body {
            self.stmt(stmt);
        }
        if !matches!(self.code.last(), Some(Op::Return)) {
            if !f.is_ctor && !matches!(f.ret, Ty::Void) {
                let c = self.intern(Constant::Int(0));
                self.emit(Op::Const(c), f.span);
            }
            self.emit(Op::Return, f.span);
        }
        Function {
            name: f.name.clone(),
            arity: f.arity,
            locals: self.local_count,
            local_names: std::mem::take(&mut self.local_names),
            code: std::mem::take(&mut self.code),
            locs: std::mem::take(&mut self.locs),
            is_async: f.is_async,
            returns_void: f.is_ctor || matches!(f.ret, Ty::Void),
        }
    }

    fn entry_thunk(&mut self) -> Function {
        self.code.clear();
        self.locs.clear();
        if let Some(class) = self.program.entry_class {
            self.emit_hidden(Op::New(class));
            self.emit_hidden(Op::Dup);
            if let Some(ctor) = self.program.entry_ctor {
                self.emit_hidden(Op::Call(ctor));
            } else {
                self.emit_hidden(Op::Pop);
            }
            self.emit_hidden(Op::LoadGlobal(0));
            self.emit_hidden(Op::Call(self.program.entry_func));
        } else {
            self.emit_hidden(Op::LoadGlobal(0));
            self.emit_hidden(Op::Call(self.program.entry_func));
        }
        if !self.program.entry_returns_int {
            let c = self.intern(Constant::Int(0));
            self.emit_hidden(Op::Const(c));
        }
        self.emit_hidden(Op::Return);
        Function {
            name: "$entry".into(),
            arity: 0,
            locals: 0,
            local_names: Vec::new(),
            code: std::mem::take(&mut self.code),
            locs: std::mem::take(&mut self.locs),
            is_async: false,
            returns_void: false,
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        let span = stmt.span;
        match &stmt.kind {
            StmtKind::Local { slot, init } => {
                if let Some(init) = init {
                    self.expr(init);
                    self.emit(Op::Store(*slot), span);
                }
            }
            StmtKind::Assign { place, value } => self.assign(place, value, span),
            StmtKind::Expr(e) => {
                let void = matches!(e.ty, Ty::Void);
                self.expr(e);
                if !void {
                    self.emit(Op::Pop, span);
                }
            }
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.expr(cond);
                let else_j = self.hole(Op::JumpIfFalse(0), span);
                for s in then_body {
                    self.stmt(s);
                }
                if let Some(else_body) = else_body {
                    let end_j = self.hole(Op::Jump(0), span);
                    self.patch(else_j, self.here());
                    for s in else_body {
                        self.stmt(s);
                    }
                    self.patch(end_j, self.here());
                } else {
                    self.patch(else_j, self.here());
                }
            }
            StmtKind::While { cond, body } => {
                let start = self.here();
                self.expr(cond);
                let fail = self.hole(Op::JumpIfFalse(0), span);
                self.push_loop(Some(start));
                for s in body {
                    self.stmt(s);
                }
                self.emit(Op::Jump(start), span);
                let end = self.here();
                self.patch(fail, end);
                self.finish_loop(end);
            }
            StmtKind::Loop { body } => {
                let start = self.here();
                self.push_loop(Some(start));
                for s in body {
                    self.stmt(s);
                }
                self.emit(Op::Jump(start), span);
                self.finish_loop(self.here());
            }
            StmtKind::ForC {
                init,
                cond,
                step,
                body,
            } => {
                if let Some(init) = init {
                    self.stmt(init);
                }
                let start = self.here();
                let fail = if let Some(cond) = cond {
                    self.expr(cond);
                    Some(self.hole(Op::JumpIfFalse(0), span))
                } else {
                    None
                };
                self.push_loop(None);
                for s in body {
                    self.stmt(s);
                }
                let cont = self.here();
                self.patch_conts(cont);
                if let Some(step) = step {
                    self.stmt(step);
                }
                self.emit(Op::Jump(start), span);
                let end = self.here();
                if let Some(fail) = fail {
                    self.patch(fail, end);
                }
                self.finish_loop(end);
            }
            StmtKind::ForIn { slot, iter, body } => self.for_in(*slot, iter, body, span),
            StmtKind::DoWhile { body, cond } => {
                let start = self.here();
                self.push_loop(None);
                for s in body {
                    self.stmt(s);
                }
                self.patch_conts(self.here());
                self.expr(cond);
                let fail = self.hole(Op::JumpIfFalse(0), span);
                self.emit(Op::Jump(start), span);
                let end = self.here();
                self.patch(fail, end);
                self.finish_loop(end);
            }
            StmtKind::Match { scrut, arms } => {
                self.expr(scrut);
                let mut ends = Vec::new();
                for arm in arms {
                    match &arm.pat {
                        Pat::Wildcard => {
                            self.emit(Op::Pop, arm.span);
                            for s in &arm.body {
                                self.stmt(s);
                            }
                            ends.push(self.hole(Op::Jump(0), arm.span));
                        }
                        Pat::Value(v) => {
                            self.emit(Op::Dup, arm.span);
                            let c = self.intern(const_to_bc(v));
                            self.emit(Op::Const(c), arm.span);
                            self.emit(Op::Eq, arm.span);
                            let next = self.hole(Op::JumpIfFalse(0), arm.span);
                            self.emit(Op::Pop, arm.span);
                            for s in &arm.body {
                                self.stmt(s);
                            }
                            ends.push(self.hole(Op::Jump(0), arm.span));
                            self.patch(next, self.here());
                        }
                    }
                }
                self.emit(Op::Pop, span);
                let end = self.here();
                for h in ends {
                    self.patch(h, end);
                }
            }
            StmtKind::Try { body, catches } => {
                let handler = self.hole(Op::PushHandler(0), span);
                for s in body {
                    self.stmt(s);
                }
                self.emit(Op::PopHandler, span);
                let end_j = self.hole(Op::Jump(0), span);
                self.patch(handler, self.here());
                let mut nexts = Vec::new();
                for catch in catches {
                    self.emit(Op::Dup, span);
                    self.emit(Op::IsType(catch.class), span);
                    let miss = self.hole(Op::JumpIfFalse(0), span);
                    self.emit(Op::Store(catch.slot), span);
                    for s in &catch.body {
                        self.stmt(s);
                    }
                    nexts.push(self.hole(Op::Jump(0), span));
                    self.patch(miss, self.here());
                }
                self.emit(Op::Throw, span);
                let end = self.here();
                self.patch(end_j, end);
                for h in nexts {
                    self.patch(h, end);
                }
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    self.expr(value);
                }
                self.emit(Op::Return, span);
            }
            StmtKind::Break => {
                let h = self.hole(Op::Jump(0), span);
                if let Some(lp) = self.loops.last_mut() {
                    lp.break_holes.push(h);
                }
            }
            StmtKind::Continue => {
                let h = self.hole(Op::Jump(0), span);
                let known = self.loops.last().and_then(|lp| lp.cont_ip);
                if let Some(ip) = known {
                    self.patch(h, ip);
                } else if let Some(lp) = self.loops.last_mut() {
                    lp.cont_holes.push(h);
                }
            }
            StmtKind::Throw { value } => {
                self.expr(value);
                self.emit(Op::Throw, span);
            }
            StmtKind::Delete { value } => {
                self.expr(value);
                self.emit(Op::Delete, span);
            }
            StmtKind::Super { ctor, args } => {
                self.emit(Op::Load(0), span);
                for a in args {
                    self.expr(a);
                }
                self.emit(Op::Call(*ctor), span);
            }
        }
    }

    fn for_in(&mut self, slot: u16, iter: &Expr, body: &[Stmt], span: Span) {
        match &iter.ty {
            Ty::Range => {
                self.expr(iter);
                let end = self.temp();
                self.emit(Op::Dup, span);
                self.emit(Op::RangeEnd, span);
                self.emit(Op::Store(end), span);
                self.emit(Op::RangeStart, span);
                self.emit(Op::Store(slot), span);
                let start = self.here();
                self.emit(Op::Load(slot), span);
                self.emit(Op::Load(end), span);
                self.emit(Op::Lt, span);
                let fail = self.hole(Op::JumpIfFalse(0), span);
                self.push_loop(None);
                for s in body {
                    self.stmt(s);
                }
                let cont = self.here();
                self.patch_conts(cont);
                self.emit(Op::Load(slot), span);
                let one = self.intern(Constant::Int(1));
                self.emit(Op::Const(one), span);
                self.emit(Op::Add, span);
                self.emit(Op::Store(slot), span);
                self.emit(Op::Jump(start), span);
                let done = self.here();
                self.patch(fail, done);
                self.finish_loop(done);
            }
            _ => {
                self.expr(iter);
                let arr = self.temp();
                let idx = self.temp();
                self.emit(Op::Store(arr), span);
                let zero = self.intern(Constant::Int(0));
                self.emit(Op::Const(zero), span);
                self.emit(Op::Store(idx), span);
                let start = self.here();
                self.emit(Op::Load(idx), span);
                self.emit(Op::Load(arr), span);
                self.emit(Op::Len, span);
                self.emit(Op::Lt, span);
                let fail = self.hole(Op::JumpIfFalse(0), span);
                self.emit(Op::Load(arr), span);
                self.emit(Op::Load(idx), span);
                self.emit(Op::GetIndex, span);
                self.emit(Op::Store(slot), span);
                self.push_loop(None);
                for s in body {
                    self.stmt(s);
                }
                let cont = self.here();
                self.patch_conts(cont);
                self.emit(Op::Load(idx), span);
                let one = self.intern(Constant::Int(1));
                self.emit(Op::Const(one), span);
                self.emit(Op::Add, span);
                self.emit(Op::Store(idx), span);
                self.emit(Op::Jump(start), span);
                let done = self.here();
                self.patch(fail, done);
                self.finish_loop(done);
            }
        }
    }

    fn assign(&mut self, place: &Place, value: &Expr, span: Span) {
        match place {
            Place::Local(slot) => {
                self.expr(value);
                self.emit(Op::Store(*slot), span);
            }
            Place::Field { recv, index } => {
                self.expr(recv);
                self.expr(value);
                self.emit(Op::StoreField(*index), span);
            }
            Place::Index { recv, index } => {
                self.expr(recv);
                self.expr(index);
                self.expr(value);
                self.emit(Op::SetIndex, span);
            }
            Place::Static { class, index } => {
                self.expr(value);
                self.emit(Op::StoreStatic(*class, *index), span);
            }
        }
    }

    fn expr(&mut self, e: &Expr) {
        let span = e.span;
        match &e.kind {
            ExprKind::Int(n) => {
                let c = self.intern(Constant::Int(*n));
                self.emit(Op::Const(c), span);
            }
            ExprKind::Float(n) => {
                let c = self.intern(Constant::Float(*n));
                self.emit(Op::Const(c), span);
            }
            ExprKind::Double(n) => {
                let c = self.intern(Constant::Double(*n));
                self.emit(Op::Const(c), span);
            }
            ExprKind::Bool(b) => {
                let c = self.intern(Constant::Bool(*b));
                self.emit(Op::Const(c), span);
            }
            ExprKind::Str(s) => {
                let c = self.intern(Constant::Str(s.clone()));
                self.emit(Op::Const(c), span);
            }
            ExprKind::Null => {
                let c = self.intern(Constant::Null);
                self.emit(Op::Const(c), span);
            }
            ExprKind::Local(slot) => self.emit(Op::Load(*slot), span),
            ExprKind::Field { recv, index } => {
                self.expr(recv);
                self.emit(Op::LoadField(*index), span);
            }
            ExprKind::StaticField { class, index } => {
                self.emit(Op::LoadStatic(*class, *index), span);
            }
            ExprKind::Enum { enum_id, variant } => {
                let c = self.intern(Constant::Enum {
                    id: *enum_id,
                    variant: *variant,
                });
                self.emit(Op::Const(c), span);
            }
            ExprKind::Unary { op, expr } => {
                self.expr(expr);
                self.emit(
                    match op {
                        UnaryOp::Not => Op::Not,
                        UnaryOp::Neg => Op::Neg,
                    },
                    span,
                );
            }
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::And => {
                    self.expr(left);
                    self.emit(Op::Dup, span);
                    let fail = self.hole(Op::JumpIfFalse(0), span);
                    self.emit(Op::Pop, span);
                    self.expr(right);
                    self.patch(fail, self.here());
                }
                BinaryOp::Or => {
                    self.expr(left);
                    self.emit(Op::Dup, span);
                    self.emit(Op::Not, span);
                    let done = self.hole(Op::JumpIfFalse(0), span);
                    self.emit(Op::Pop, span);
                    self.expr(right);
                    self.patch(done, self.here());
                }
                other => {
                    self.expr(left);
                    self.expr(right);
                    self.emit(bin_op(*other), span);
                }
            },
            ExprKind::Call { kind, args } => {
                for a in args {
                    self.expr(a);
                }
                match kind {
                    CallKind::Direct { func, is_async } => {
                        if *is_async {
                            self.emit(Op::MakeTask(*func), span);
                        } else {
                            self.emit(Op::Call(*func), span);
                        }
                    }
                    CallKind::Virtual { slot, argc, is_async } => {
                        if *is_async {
                            self.emit(Op::MakeTaskVirtual(*slot, *argc), span);
                        } else {
                            self.emit(Op::CallVirtual(*slot, *argc), span);
                        }
                    }
                    CallKind::Interface {
                        iface,
                        slot,
                        argc,
                        is_async,
                    } => {
                        if *is_async {
                            self.emit(Op::MakeTaskInterface(*iface, *slot, *argc), span);
                        } else {
                            self.emit(Op::CallInterface(*iface, *slot, *argc), span);
                        }
                    }
                    CallKind::Builtin(b) => self.emit(builtin_op(*b), span),
                }
            }
            ExprKind::New { class, ctor, args } => {
                self.emit(Op::New(*class), span);
                self.emit(Op::Dup, span);
                for a in args {
                    self.expr(a);
                }
                self.emit(Op::Call(*ctor), span);
            }
            ExprKind::NewArray { len } => {
                self.expr(len);
                self.emit(Op::NewArray, span);
            }
            ExprKind::Array { elems } => {
                for el in elems {
                    self.expr(el);
                }
                self.emit(Op::ArrayLit(elems.len() as u16), span);
            }
            ExprKind::Index { recv, index } => {
                self.expr(recv);
                self.expr(index);
                self.emit(Op::GetIndex, span);
            }
            ExprKind::Range { lo, hi } => {
                self.expr(lo);
                self.expr(hi);
                self.emit(Op::Range, span);
            }
            ExprKind::Await { expr } => {
                self.expr(expr);
                self.emit(Op::Await, span);
            }
        }
    }

    fn push_loop(&mut self, cont_ip: Option<u32>) {
        self.loops.push(Loop {
            cont_ip,
            cont_holes: Vec::new(),
            break_holes: Vec::new(),
        });
    }

    fn finish_loop(&mut self, end: u32) {
        if let Some(lp) = self.loops.pop() {
            for h in lp.break_holes {
                self.patch(h, end);
            }
        }
    }

    fn patch_conts(&mut self, ip: u32) {
        if let Some(lp) = self.loops.last_mut() {
            let holes = std::mem::take(&mut lp.cont_holes);
            for h in holes {
                self.patch(h, ip);
            }
        }
    }

    fn temp(&mut self) -> u16 {
        let slot = self.local_count;
        self.local_count += 1;
        self.local_names.push(String::new());
        slot
    }

    fn intern(&mut self, c: Constant) -> u32 {
        if let Some(i) = self.constants.iter().position(|e| const_eq(e, &c)) {
            return i as u32;
        }
        let id = self.constants.len() as u32;
        self.constants.push(c);
        id
    }

    fn emit_hidden(&mut self, op: Op) {
        self.code.push(op);
        self.locs.push(SourceLoc::unknown());
    }

    fn emit(&mut self, op: Op, span: Span) {
        self.code.push(op);
        self.locs.push(self.loc(span));
    }

    fn hole(&mut self, op: Op, span: Span) -> usize {
        let i = self.code.len();
        self.emit(op, span);
        i
    }

    fn here(&self) -> u32 {
        self.code.len() as u32
    }

    fn patch(&mut self, at: usize, target: u32) {
        match &mut self.code[at] {
            Op::Jump(t) | Op::JumpIfFalse(t) | Op::PushHandler(t) => *t = target,
            _ => {}
        }
    }

    fn loc(&self, span: Span) -> SourceLoc {
        let Some(file) = self.sources.get(span.file as usize) else {
            return SourceLoc::unknown();
        };
        let (line, col) = file.line_col(span.start);
        SourceLoc {
            file: span.file,
            line,
            col,
        }
    }
}

fn bin_op(op: BinaryOp) -> Op {
    match op {
        BinaryOp::Add => Op::Add,
        BinaryOp::Sub => Op::Sub,
        BinaryOp::Mul => Op::Mul,
        BinaryOp::Div => Op::Div,
        BinaryOp::Mod => Op::Mod,
        BinaryOp::Eq => Op::Eq,
        BinaryOp::Ne => Op::Ne,
        BinaryOp::Lt => Op::Lt,
        BinaryOp::Le => Op::Le,
        BinaryOp::Gt => Op::Gt,
        BinaryOp::Ge => Op::Ge,
        BinaryOp::And | BinaryOp::Or => unreachable!("and/or are compiled as jumps"),
    }
}

fn builtin_op(b: Builtin) -> Op {
    match b {
        Builtin::Print => Op::Print,
        Builtin::PrintLn => Op::PrintLn,
        Builtin::ToString => Op::ToString,
        Builtin::ToInt => Op::ToInt,
        Builtin::ToFloat => Op::ToFloat,
        Builtin::ToDouble => Op::ToDouble,
        Builtin::Len => Op::Len,
        Builtin::Host(id) => Op::Host(id),
    }
}

fn const_eq(a: &Constant, b: &Constant) -> bool {
    match (a, b) {
        (Constant::Int(x), Constant::Int(y)) => x == y,
        (Constant::Float(x), Constant::Float(y)) => x.to_bits() == y.to_bits(),
        (Constant::Double(x), Constant::Double(y)) => x.to_bits() == y.to_bits(),
        (Constant::Bool(x), Constant::Bool(y)) => x == y,
        (Constant::Str(x), Constant::Str(y)) => x == y,
        (Constant::Null, Constant::Null) => true,
        (
            Constant::Enum { id: a, variant: av },
            Constant::Enum { id: b, variant: bv },
        ) => a == b && av == bv,
        _ => false,
    }
}
