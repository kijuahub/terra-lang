use std::collections::{HashMap, HashSet};

use crate::hir::*;
use crate::ty::Ty;
use terra_diagnostics::{DiagKind, Diagnostic, Span};
use terra_syntax::ast::{self, BinaryOp as AstBin, Mutability, UnaryOp as AstUn, Vis};

const PRELUDE: &str = "com.terra.lang";

pub fn analyze(
    files: &[ast::File],
    expected: &[Option<String>],
) -> Result<Program, Vec<Diagnostic>> {
    let mut ax = Analyzer::new();
    ax.prelude();
    for (i, file) in files.iter().enumerate() {
        ax.collect_file(i as u32, file, expected.get(i).and_then(|p| p.as_deref()));
    }
    ax.build_imports(files);
    ax.resolve_hierarchy();
    ax.resolve_signatures();
    ax.layout_all();
    ax.build_vtables();
    ax.flatten_interfaces();
    ax.build_itables();
    ax.eval_package_consts();
    ax.eval_static_fields();
    let ids: Vec<u32> = (0..ax.funcs.len() as u32).collect();
    for id in ids {
        ax.ensure_func(id);
    }
    ax.check_override_results();
    ax.pick_entry();
    if ax.diags.iter().any(|d| {
        matches!(d.severity, terra_diagnostics::Severity::Error)
    }) {
        return Err(ax.diags);
    }
    Ok(ax.finish())
}

struct Analyzer {
    diags: Vec<Diagnostic>,
    files: Vec<FileRec>,
    packages: HashMap<String, Package>,
    classes: Vec<ClassRec>,
    ifaces: Vec<IfaceRec>,
    enums: Vec<EnumRec>,
    funcs: Vec<FuncRec>,
    consts: Vec<ConstRec>,
    static_inits: Vec<StaticInit>,
    checking: Vec<u32>,
    env: Option<FuncEnv>,
    entry: Option<Entry>,
}

struct FileRec {
    package: String,
    imports: HashMap<String, Symbol>,
}

struct Package {
    symbols: HashMap<String, Symbol>,
}

#[derive(Clone)]
enum Symbol {
    Func(u32),
    Class(u32),
    Iface(u32),
    Enum(u32),
    Const(u32),
    Builtin(Builtin),
}

struct ClassRec {
    name: String,
    package: String,
    file: u32,
    span: Span,
    vis: Vis,
    is_final: bool,
    is_exception: bool,
    is_prelude: bool,
    super_id: Option<u32>,
    iface_ids: Vec<u32>,
    own_fields: Vec<OwnField>,
    statics: Vec<OwnField>,
    layout: Vec<LayField>,
    methods: Vec<u32>,
    ctor: Option<u32>,
    vtable: Vec<u32>,
    vtable_names: Vec<String>,
    itables: Vec<(u32, Vec<u32>)>,
    layout_done: bool,
    vtable_done: bool,
    hier_done: bool,
    unresolved_super: Option<ast::TypeRef>,
    unresolved_ifaces: Vec<ast::TypeRef>,
}

#[derive(Clone)]
struct OwnField {
    name: String,
    ty_ast: Option<ast::TypeRef>,
    ty: Ty,
    mutable: bool,
    is_const: bool,
    vis: Vis,
    span: Span,
    init: Option<ast::Expr>,
    index: u16,
}

#[derive(Clone)]
struct LayField {
    name: String,
    ty: Ty,
    mutable: bool,
    vis: Vis,
    owner: u32,
    index: u16,
}

struct IfaceRec {
    name: String,
    package: String,
    file: u32,
    span: Span,
    vis: Vis,
    parents: Vec<u32>,
    unresolved_parents: Vec<ast::TypeRef>,
    own: Vec<IfaceAst>,
    methods: Vec<IfaceMethod>,
    flat_done: bool,
    hier_done: bool,
}

#[derive(Clone)]
struct IfaceAst {
    name: String,
    params: Vec<ast::TypeRef>,
    ret: Option<ast::TypeRef>,
    is_async: bool,
    #[allow(dead_code)]
    span: Span,
}

#[derive(Clone)]
struct IfaceMethod {
    name: String,
    params: Vec<Ty>,
    ret: Ty,
    is_async: bool,
}

struct EnumRec {
    name: String,
    package: String,
    file: u32,
    #[allow(dead_code)]
    span: Span,
    vis: Vis,
    variants: Vec<String>,
}

struct FuncRec {
    simple: String,
    package: String,
    file: u32,
    span: Span,
    vis: Vis,
    is_static: bool,
    is_final: bool,
    is_async: bool,
    is_override: bool,
    is_ctor: bool,
    class: Option<u32>,
    params: Vec<(String, Ty)>,
    unresolved_params: Vec<(String, ast::TypeRef)>,
    ret: Ty,
    unresolved_ret: Option<ast::TypeRef>,
    infer: bool,
    body: BodyKind,
    slot: Option<u16>,
    done: bool,
    locals: Vec<Local>,
    hir: Vec<Stmt>,
}

#[derive(Clone)]
enum BodyKind {
    Empty,
    Ast(ast::Block),
    SetMessage,
    SuperMsg(String),
    DefaultCtor,
}

struct ConstRec {
    name: String,
    package: String,
    file: u32,
    span: Span,
    vis: Vis,
    ty_ast: Option<ast::TypeRef>,
    ty: Ty,
    init: ast::Expr,
    value: Option<ConstValue>,
    evaluating: bool,
}

struct FuncEnv {
    id: u32,
    class: Option<u32>,
    is_ctor: bool,
    is_static: bool,
    ret: Ty,
    infer: bool,
    has_value: bool,
    locals: Vec<Local>,
    scopes: Vec<HashMap<String, Binding>>,
    loop_depth: u32,
    saw_super: bool,
}

struct Binding {
    slot: u16,
    ty: Ty,
    mutable: bool,
    const_value: Option<ConstValue>,
}

enum Entry {
    Free(u32),
    Method { class: u32, func: u32 },
}

impl Analyzer {
    fn new() -> Self {
        Self {
            diags: Vec::new(),
            files: Vec::new(),
            packages: HashMap::new(),
            classes: Vec::new(),
            ifaces: Vec::new(),
            enums: Vec::new(),
            funcs: Vec::new(),
            consts: Vec::new(),
            static_inits: Vec::new(),
            checking: Vec::new(),
            env: None,
            entry: None,
        }
    }

    fn err(&mut self, span: Span, kind: DiagKind) {
        self.diags.push(Diagnostic::error(span, kind));
    }

    fn package_mut(&mut self, name: &str) -> &mut Package {
        self.packages.entry(name.to_string()).or_insert_with(|| Package {
            symbols: HashMap::new(),
        })
    }

    fn define(&mut self, package: &str, name: &str, sym: Symbol, span: Span) {
        let pkg = self.package_mut(package);
        if pkg.symbols.contains_key(name) {
            self.diags.push(Diagnostic::error(
                span,
                DiagKind::Duplicate { name: name.into() },
            ));
            return;
        }
        pkg.symbols.insert(name.to_string(), sym);
    }

    fn prelude(&mut self) {
        let object = self.prelude_class("Object", false, None);
        let exception = self.prelude_class("Exception", true, Some(object));
        self.classes[exception as usize].own_fields.push(OwnField {
            name: "message".into(),
            ty_ast: None,
            ty: Ty::String,
            mutable: false,
            is_const: false,
            vis: Vis::Pub,
            span: Span::new(0, 0, 0),
            init: None,
            index: 0,
        });
        self.prelude_ctor(object, Vec::new(), BodyKind::Empty);
        self.prelude_ctor(
            exception,
            vec![("message".into(), Ty::String)],
            BodyKind::SetMessage,
        );
        for (name, msg) in [
            ("DivZero", "division by zero"),
            ("NullRef", "null reference"),
            ("UseAfterFree", "use after free"),
            ("IndexError", "index out of bounds"),
            ("Overflow", "numeric overflow"),
            ("TypeError", "type error"),
            ("CastError", "conversion failed"),
            ("IoError", "io error"),
        ] {
            let id = self.prelude_class(name, true, Some(exception));
            self.prelude_ctor(id, Vec::new(), BodyKind::SuperMsg(msg.into()));
        }
        for (name, b) in [
            ("print", Builtin::Print),
            ("println", Builtin::PrintLn),
            ("to_string", Builtin::ToString),
            ("to_int", Builtin::ToInt),
            ("to_float", Builtin::ToFloat),
            ("to_double", Builtin::ToDouble),
            ("len", Builtin::Len),
        ] {
            self.define(PRELUDE, name, Symbol::Builtin(b), Span::new(0, 0, 0));
        }
        for (id, func) in terra_bytecode::HOSTS.iter().enumerate() {
            self.define(
                func.package,
                func.name,
                Symbol::Builtin(Builtin::Host(id as u16)),
                Span::new(0, 0, 0),
            );
        }
    }

    fn prelude_class(&mut self, name: &str, is_exception: bool, super_id: Option<u32>) -> u32 {
        let id = self.classes.len() as u32;
        self.classes.push(ClassRec {
            name: name.into(),
            package: PRELUDE.into(),
            file: u32::MAX,
            span: Span::new(0, 0, 0),
            vis: Vis::Pub,
            is_final: false,
            is_exception,
            is_prelude: true,
            super_id,
            iface_ids: Vec::new(),
            own_fields: Vec::new(),
            statics: Vec::new(),
            layout: Vec::new(),
            methods: Vec::new(),
            ctor: None,
            vtable: Vec::new(),
            vtable_names: Vec::new(),
            itables: Vec::new(),
            layout_done: false,
            vtable_done: false,
            hier_done: true,
            unresolved_super: None,
            unresolved_ifaces: Vec::new(),
        });
        self.define(PRELUDE, name, Symbol::Class(id), Span::new(0, 0, 0));
        id
    }

    fn prelude_ctor(&mut self, class: u32, params: Vec<(String, Ty)>, body: BodyKind) {
        let id = self.funcs.len() as u32;
        let name = self.classes[class as usize].name.clone();
        self.funcs.push(FuncRec {
            simple: name,
            package: PRELUDE.into(),
            file: u32::MAX,
            span: Span::new(0, 0, 0),
            vis: Vis::Pub,
            is_static: false,
            is_final: false,
            is_async: false,
            is_override: false,
            is_ctor: true,
            class: Some(class),
            params: params.clone(),
            unresolved_params: Vec::new(),
            ret: Ty::Void,
            unresolved_ret: None,
            infer: false,
            body,
            slot: None,
            done: false,
            locals: Vec::new(),
            hir: Vec::new(),
        });
        self.classes[class as usize].ctor = Some(id);
        self.classes[class as usize].methods.push(id);
    }

    fn collect_file(&mut self, file: u32, ast: &ast::File, expected: Option<&str>) {
        let declared = ast.package.as_ref().map(|p| p.join());
        let package = match (declared, expected) {
            (Some(d), Some(e)) if d != e => {
                self.err(
                    ast.package.as_ref().unwrap().span,
                    DiagKind::PackageMismatch {
                        declared: d,
                        expected: e.into(),
                    },
                );
                e.to_string()
            }
            (Some(d), _) => d,
            (None, Some(e)) => {
                self.err(Span::new(file, 0, 0), DiagKind::NoPackage);
                e.to_string()
            }
            (None, None) => "_main".into(),
        };
        self.files.push(FileRec {
            package: package.clone(),
            imports: HashMap::new(),
        });
        for item in &ast.items {
            self.collect_item(file, &package, item, None);
        }
    }

    fn collect_item(&mut self, file: u32, package: &str, item: &ast::Item, class: Option<u32>) {
        if let Some(class_id) = class {
            if !matches!(
                item.kind,
                ast::ItemKind::Function(_) | ast::ItemKind::Field(_)
            ) {
                self.err(item.span, DiagKind::NestedType);
                return;
            }
            let _ = class_id;
        }
        match &item.kind {
            ast::ItemKind::Class(c) => self.collect_class(file, package, item, c),
            ast::ItemKind::Interface(iface) => self.collect_iface(file, package, item, iface),
            ast::ItemKind::Enum(e) => self.collect_enum(file, package, item, e),
            ast::ItemKind::Exception { name } => {
                self.collect_exception(file, package, item, name)
            }
            ast::ItemKind::Function(f) => {
                self.collect_func(file, package, item, f, class);
            }
            ast::ItemKind::Field(field) => {
                if class.is_none() {
                    self.collect_const(file, package, item, field);
                } else {
                    self.collect_field(item, field, class.unwrap());
                }
            }
        }
    }

    fn bad_mods(&mut self, item: &ast::Item, allow: &str) {
        let m = item.mods;
        let mut bad = Vec::new();
        if !allow.contains("static") && m.is_static {
            bad.push("static");
        }
        if !allow.contains("final") && m.is_final {
            bad.push("final");
        }
        if !allow.contains("async") && m.is_async {
            bad.push("async");
        }
        if !allow.contains("override") && m.is_override {
            bad.push("override");
        }
        if !bad.is_empty() {
            self.err(
                item.span,
                DiagKind::BadModifier {
                    detail: format!("modifier {} is not allowed here", bad.join(", ")),
                },
            );
        }
    }

    fn collect_class(&mut self, file: u32, package: &str, item: &ast::Item, c: &ast::Class) {
        self.bad_mods(item, "final");
        let id = self.classes.len() as u32;
        self.define(package, &c.name, Symbol::Class(id), item.span);
        self.classes.push(ClassRec {
            name: c.name.clone(),
            package: package.into(),
            file,
            span: item.span,
            vis: item.vis,
            is_final: item.mods.is_final,
            is_exception: false,
            is_prelude: false,
            super_id: None,
            iface_ids: Vec::new(),
            own_fields: Vec::new(),
            statics: Vec::new(),
            layout: Vec::new(),
            methods: Vec::new(),
            ctor: None,
            vtable: Vec::new(),
            vtable_names: Vec::new(),
            itables: Vec::new(),
            layout_done: false,
            vtable_done: false,
            hier_done: false,
            unresolved_super: c.extends.clone(),
            unresolved_ifaces: c.implements.clone(),
        });
        let mut saw_ctor = false;
        for member in &c.members {
            match &member.kind {
                ast::ItemKind::Function(f) if f.name == c.name => {
                    saw_ctor = true;
                    self.collect_func(file, package, member, f, Some(id));
                }
                _ => self.collect_item(file, package, member, Some(id)),
            }
        }
        if !saw_ctor {
            self.prelude_like_default_ctor(id, file, package, item.span);
        }
    }

    fn prelude_like_default_ctor(&mut self, class: u32, file: u32, package: &str, span: Span) {
        let id = self.funcs.len() as u32;
        let name = self.classes[class as usize].name.clone();
        self.funcs.push(FuncRec {
            simple: name,
            package: package.into(),
            file,
            span,
            vis: Vis::Pub,
            is_static: false,
            is_final: false,
            is_async: false,
            is_override: false,
            is_ctor: true,
            class: Some(class),
            params: Vec::new(),
            unresolved_params: Vec::new(),
            ret: Ty::Void,
            unresolved_ret: None,
            infer: false,
            body: BodyKind::DefaultCtor,
            slot: None,
            done: false,
            locals: Vec::new(),
            hir: Vec::new(),
        });
        self.classes[class as usize].ctor = Some(id);
        self.classes[class as usize].methods.push(id);
    }

    fn collect_exception(&mut self, file: u32, package: &str, item: &ast::Item, name: &str) {
        self.bad_mods(item, "");
        let id = self.classes.len() as u32;
        self.define(package, name, Symbol::Class(id), item.span);
        self.classes.push(ClassRec {
            name: name.into(),
            package: package.into(),
            file,
            span: item.span,
            vis: item.vis,
            is_final: false,
            is_exception: true,
            is_prelude: false,
            super_id: None,
            iface_ids: Vec::new(),
            own_fields: Vec::new(),
            statics: Vec::new(),
            layout: Vec::new(),
            methods: Vec::new(),
            ctor: None,
            vtable: Vec::new(),
            vtable_names: Vec::new(),
            itables: Vec::new(),
            layout_done: false,
            vtable_done: false,
            hier_done: false,
            unresolved_super: None,
            unresolved_ifaces: Vec::new(),
        });
        let ctor = self.funcs.len() as u32;
        self.funcs.push(FuncRec {
            simple: name.into(),
            package: package.into(),
            file,
            span: item.span,
            vis: Vis::Pub,
            is_static: false,
            is_final: false,
            is_async: false,
            is_override: false,
            is_ctor: true,
            class: Some(id),
            params: Vec::new(),
            unresolved_params: Vec::new(),
            ret: Ty::Void,
            unresolved_ret: None,
            infer: false,
            body: BodyKind::SuperMsg(name.into()),
            slot: None,
            done: false,
            locals: Vec::new(),
            hir: Vec::new(),
        });
        self.classes[id as usize].ctor = Some(ctor);
        self.classes[id as usize].methods.push(ctor);
        // super is Exception, filled in resolve
        self.classes[id as usize].unresolved_super = Some(ast::TypeRef {
            path: vec!["Exception".into()],
            dims: 0,
            span: item.span,
        });
    }

    fn collect_iface(&mut self, file: u32, package: &str, item: &ast::Item, iface: &ast::Interface) {
        self.bad_mods(item, "");
        let id = self.ifaces.len() as u32;
        self.define(package, &iface.name, Symbol::Iface(id), item.span);
        let own = iface
            .methods
            .iter()
            .map(|m| IfaceAst {
                name: m.name.clone(),
                params: m.params.iter().map(|p| p.ty.clone()).collect(),
                ret: m.ret.clone(),
                is_async: false,
                span: m.span,
            })
            .collect();
        self.ifaces.push(IfaceRec {
            name: iface.name.clone(),
            package: package.into(),
            file,
            span: item.span,
            vis: item.vis,
            parents: Vec::new(),
            unresolved_parents: iface.extends.clone(),
            own,
            methods: Vec::new(),
            flat_done: false,
            hier_done: false,
        });
    }

    fn collect_enum(&mut self, file: u32, package: &str, item: &ast::Item, e: &ast::EnumDecl) {
        self.bad_mods(item, "");
        let id = self.enums.len() as u32;
        self.define(package, &e.name, Symbol::Enum(id), item.span);
        let mut seen = HashSet::new();
        let mut variants = Vec::new();
        for (name, span) in &e.variants {
            if !seen.insert(name.clone()) {
                self.err(*span, DiagKind::Duplicate { name: name.clone() });
            }
            variants.push(name.clone());
        }
        self.enums.push(EnumRec {
            name: e.name.clone(),
            package: package.into(),
            file,
            span: item.span,
            vis: item.vis,
            variants,
        });
    }

    fn collect_const(&mut self, file: u32, package: &str, item: &ast::Item, field: &ast::Field) {
        self.bad_mods(item, "final");
        if field.mutable != Mutability::Const {
            self.err(field.span, DiagKind::TopLevelBinding);
            return;
        }
        let Some(init) = field.init.clone() else {
            self.err(field.span, DiagKind::NeedConst);
            return;
        };
        let id = self.consts.len() as u32;
        self.define(package, &field.name, Symbol::Const(id), field.span);
        self.consts.push(ConstRec {
            name: field.name.clone(),
            package: package.into(),
            file,
            span: field.span,
            vis: item.vis,
            ty_ast: field.ty.clone(),
            ty: Ty::Error,
            init,
            value: None,
            evaluating: false,
        });
    }

    fn collect_field(&mut self, item: &ast::Item, field: &ast::Field, class: u32) {
        self.bad_mods(item, "static,final");
        if item.mods.is_async || item.mods.is_override {
            self.err(
                item.span,
                DiagKind::BadModifier {
                    detail: "async/override cannot apply to a field".into(),
                },
            );
        }
        if field.mutable == Mutability::Mut && item.mods.is_final {
            self.err(
                field.span,
                DiagKind::BadModifier {
                    detail: "final mut is contradictory".into(),
                },
            );
        }
        let is_const = field.mutable == Mutability::Const;
        let mutable = field.mutable == Mutability::Mut && !item.mods.is_final;
        if is_const && field.init.is_none() {
            self.err(field.span, DiagKind::NeedConst);
        }
        if field.mutable == Mutability::Var && field.init.is_none() {
            self.err(
                field.span,
                DiagKind::BadModifier {
                    detail: "var requires an initializer".into(),
                },
            );
        }
        let own = OwnField {
            name: field.name.clone(),
            ty_ast: field.ty.clone(),
            ty: Ty::Error,
            mutable,
            is_const,
            vis: item.vis,
            span: field.span,
            init: field.init.clone(),
            index: 0,
        };
        let class = &mut self.classes[class as usize];
        let list = if item.mods.is_static {
            &mut class.statics
        } else {
            &mut class.own_fields
        };
        if list.iter().any(|f| f.name == own.name) {
            self.diags.push(Diagnostic::error(
                field.span,
                DiagKind::Duplicate {
                    name: field.name.clone(),
                },
            ));
            return;
        }
        list.push(own);
    }

    fn collect_func(
        &mut self,
        file: u32,
        package: &str,
        item: &ast::Item,
        f: &ast::Function,
        class: Option<u32>,
    ) {
        let is_ctor = class.is_some_and(|c| self.classes[c as usize].name == f.name);
        if class.is_none() {
            self.bad_mods(item, "async");
        } else if is_ctor {
            self.bad_mods(item, "");
            if item.mods.is_async {
                self.err(item.span, DiagKind::AsyncCtor);
            }
            if item.mods.is_static {
                self.err(
                    item.span,
                    DiagKind::BadModifier {
                        detail: "a constructor cannot be static".into(),
                    },
                );
            }
            if f.ret.is_some() {
                self.err(f.span, DiagKind::CtorReturn);
            }
        } else {
            self.bad_mods(item, "static,final,async,override");
        }
        if item.mods.is_static && item.mods.is_override {
            self.err(
                item.span,
                DiagKind::BadModifier {
                    detail: "static method cannot be override".into(),
                },
            );
        }
        let body = if let Some(body) = &f.body {
            BodyKind::Ast(body.clone())
        } else {
            self.err(
                f.span,
                DiagKind::MissingBody {
                    name: f.name.clone(),
                },
            );
            BodyKind::Empty
        };
        if let Some(cid) = class {
            // shortcut: no overloading, one method per name until signatures need it
            let dup = self.classes[cid as usize].methods.iter().any(|&m| {
                self.funcs[m as usize].simple == f.name
            });
            if dup {
                self.err(f.span, DiagKind::Duplicate { name: f.name.clone() });
                return;
            }
        }
        let id = self.funcs.len() as u32;
        if class.is_none() {
            self.define(package, &f.name, Symbol::Func(id), f.span);
        }
        self.funcs.push(FuncRec {
            simple: f.name.clone(),
            package: package.into(),
            file,
            span: f.span,
            vis: item.vis,
            is_static: item.mods.is_static || class.is_none(),
            is_final: item.mods.is_final,
            is_async: item.mods.is_async,
            is_override: item.mods.is_override,
            is_ctor,
            class,
            params: Vec::new(),
            unresolved_params: f
                .params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone()))
                .collect(),
            ret: Ty::Void,
            unresolved_ret: f.ret.clone(),
            infer: f.ret.is_none() && !is_ctor,
            body,
            slot: None,
            done: false,
            locals: Vec::new(),
            hir: Vec::new(),
        });
        if let Some(cid) = class {
            if is_ctor {
                self.classes[cid as usize].ctor = Some(id);
            }
            self.classes[cid as usize].methods.push(id);
        }
    }

    fn build_imports(&mut self, files: &[ast::File]) {
        for i in 0..files.len() {
            self.import_package(i as u32, PRELUDE);
        }
        for (i, file) in files.iter().enumerate() {
            for u in &file.uses {
                let path = u.join();
                if self.packages.contains_key(&path) {
                    self.import_package(i as u32, &path);
                    continue;
                }
                if u.parts.len() >= 2 {
                    let pkg = u.parts[..u.parts.len() - 1].join(".");
                    let name = u.parts.last().unwrap().clone();
                    if let Some(sym) = self
                        .packages
                        .get(&pkg)
                        .and_then(|p| p.symbols.get(&name))
                        .cloned()
                    {
                        if self.symbol_vis(&sym) == Vis::Pub {
                            self.files[i].imports.insert(name, sym);
                        } else {
                            self.err(u.span, DiagKind::Private { name });
                        }
                        continue;
                    }
                }
                self.err(u.span, DiagKind::UnknownImport { path });
            }
        }
    }

    fn import_package(&mut self, file: u32, package: &str) {
        let symbols: Vec<(String, Symbol)> = match self.packages.get(package) {
            Some(pkg) => pkg
                .symbols
                .iter()
                .filter(|(_, sym)| self.symbol_vis(sym) == Vis::Pub)
                .map(|(n, s)| (n.clone(), s.clone()))
                .collect(),
            None => Vec::new(),
        };
        for (name, sym) in symbols {
            self.files[file as usize].imports.entry(name).or_insert(sym);
        }
    }

    fn symbol_vis(&self, sym: &Symbol) -> Vis {
        match sym {
            Symbol::Class(id) => self.classes[*id as usize].vis,
            Symbol::Func(id) => self.funcs[*id as usize].vis,
            Symbol::Iface(id) => self.ifaces[*id as usize].vis,
            Symbol::Enum(id) => self.enums[*id as usize].vis,
            Symbol::Const(id) => self.consts[*id as usize].vis,
            Symbol::Builtin(_) => Vis::Pub,
        }
    }

    fn symbol_visible(&self, sym: &Symbol, from_file: u32) -> bool {
        if from_file == u32::MAX {
            return true;
        }
        let vis = self.symbol_vis(sym);
        let owner_file = self.symbol_file(sym);
        let owner_pkg = self.symbol_package(sym);
        match vis {
            Vis::Pub => true,
            Vis::Priv => owner_file == from_file || owner_file == u32::MAX,
            Vis::Default => {
                owner_file == u32::MAX
                    || self.files.get(from_file as usize).map(|f| f.package.as_str())
                        == Some(owner_pkg.as_str())
            }
        }
    }

    fn symbol_file(&self, sym: &Symbol) -> u32 {
        match sym {
            Symbol::Class(id) => self.classes[*id as usize].file,
            Symbol::Func(id) => self.funcs[*id as usize].file,
            Symbol::Iface(id) => self.ifaces[*id as usize].file,
            Symbol::Enum(id) => self.enums[*id as usize].file,
            Symbol::Const(id) => self.consts[*id as usize].file,
            Symbol::Builtin(_) => u32::MAX,
        }
    }

    fn symbol_package(&self, sym: &Symbol) -> String {
        match sym {
            Symbol::Class(id) => self.classes[*id as usize].package.clone(),
            Symbol::Func(id) => self.funcs[*id as usize].package.clone(),
            Symbol::Iface(id) => self.ifaces[*id as usize].package.clone(),
            Symbol::Enum(id) => self.enums[*id as usize].package.clone(),
            Symbol::Const(id) => self.consts[*id as usize].package.clone(),
            Symbol::Builtin(_) => PRELUDE.into(),
        }
    }

    fn lookup_name(&self, file: u32, name: &str) -> Option<Symbol> {
        if file == u32::MAX {
            return self.packages.get(PRELUDE).and_then(|p| p.symbols.get(name)).cloned();
        }
        let package = &self.files[file as usize].package;
        if let Some(sym) = self.packages.get(package).and_then(|p| p.symbols.get(name)).cloned() {
            if self.symbol_visible(&sym, file) {
                return Some(sym);
            }
        }
        self.files[file as usize].imports.get(name).cloned()
    }

    fn resolve_hierarchy(&mut self) {
        let n = self.classes.len();
        for i in 0..n {
            self.resolve_super(i as u32, &mut Vec::new());
        }
        let n = self.ifaces.len();
        for i in 0..n {
            self.resolve_iface(i as u32, &mut Vec::new());
        }
    }

    fn resolve_super(&mut self, id: u32, stack: &mut Vec<u32>) {
        if self.classes[id as usize].hier_done {
            return;
        }
        if stack.contains(&id) {
            let name = self.classes[id as usize].name.clone();
            let span = self.classes[id as usize].span;
            self.err(span, DiagKind::Cycle { name });
            self.classes[id as usize].hier_done = true;
            return;
        }
        stack.push(id);
        let file = self.classes[id as usize].file;
        let span = self.classes[id as usize].span;
        let unresolved = self.classes[id as usize].unresolved_super.clone();
        let super_id = if let Some(ty) = unresolved {
            match self.resolve_type(file, &ty) {
                Ty::Class(pid) => {
                    self.resolve_super(pid, stack);
                    if self.classes[pid as usize].is_final {
                        let name = self.classes[pid as usize].name.clone();
                        self.err(span, DiagKind::FinalClass { name });
                    }
                    if self.classes[pid as usize].is_exception {
                        self.classes[id as usize].is_exception = true;
                    }
                    Some(pid)
                }
                Ty::Error => None,
                _ => {
                    self.err(
                        ty.span,
                        DiagKind::UnknownType {
                            name: ty.path.join("."),
                        },
                    );
                    None
                }
            }
        } else {
            Some(self.find_class(PRELUDE, "Object"))
        };
        self.classes[id as usize].super_id = super_id;
        let ifaces = self.classes[id as usize].unresolved_ifaces.clone();
        let mut iface_ids = Vec::new();
        for ty in ifaces {
            match self.resolve_type(file, &ty) {
                Ty::Iface(iid) => iface_ids.push(iid),
                Ty::Error => {}
                _ => self.err(
                    ty.span,
                    DiagKind::UnknownType {
                        name: ty.path.join("."),
                    },
                ),
            }
        }
        self.classes[id as usize].iface_ids = iface_ids;
        self.classes[id as usize].hier_done = true;
        stack.pop();
    }

    fn resolve_iface(&mut self, id: u32, stack: &mut Vec<u32>) {
        if self.ifaces[id as usize].hier_done {
            return;
        }
        if stack.contains(&id) {
            let name = self.ifaces[id as usize].name.clone();
            let span = self.ifaces[id as usize].span;
            self.err(span, DiagKind::Cycle { name });
            self.ifaces[id as usize].hier_done = true;
            return;
        }
        stack.push(id);
        let file = self.ifaces[id as usize].file;
        let parents = self.ifaces[id as usize].unresolved_parents.clone();
        let mut ids = Vec::new();
        for ty in parents {
            match self.resolve_type(file, &ty) {
                Ty::Iface(pid) => {
                    self.resolve_iface(pid, stack);
                    ids.push(pid);
                }
                Ty::Error => {}
                _ => self.err(
                    ty.span,
                    DiagKind::UnknownType {
                        name: ty.path.join("."),
                    },
                ),
            }
        }
        self.ifaces[id as usize].parents = ids;
        self.ifaces[id as usize].hier_done = true;
        stack.pop();
    }

    fn find_class(&self, package: &str, name: &str) -> u32 {
        match self.packages.get(package).and_then(|p| p.symbols.get(name)) {
            Some(Symbol::Class(id)) => *id,
            _ => 0,
        }
    }

    fn resolve_type(&mut self, file: u32, ty: &ast::TypeRef) -> Ty {
        let mut base = self.resolve_type_path(file, &ty.path, ty.span);
        for _ in 0..ty.dims {
            base = Ty::Array(Box::new(base));
        }
        base
    }

    fn resolve_type_path(&mut self, file: u32, path: &[String], span: Span) -> Ty {
        if path.len() == 1 {
            if let Some(t) = primitive(&path[0]) {
                return t;
            }
        }
        if path.len() > 1 {
            for i in 1..path.len() {
                let pkg = path[..i].join(".");
                if self.packages.contains_key(&pkg) && path.len() == i + 1 {
                    if let Some(sym) = self
                        .packages
                        .get(&pkg)
                        .and_then(|p| p.symbols.get(&path[i]))
                        .cloned()
                    {
                        return self.symbol_type(&sym, span);
                    }
                }
            }
        }
        match self.lookup_name(file, path.last().unwrap_or(&String::new())) {
            Some(sym) if path.len() == 1 => self.symbol_type(&sym, span),
            _ => {
                self.err(
                    span,
                    DiagKind::UnknownType {
                        name: path.join("."),
                    },
                );
                Ty::Error
            }
        }
    }

    fn symbol_type(&mut self, sym: &Symbol, span: Span) -> Ty {
        match sym {
            Symbol::Class(id) => Ty::Class(*id),
            Symbol::Iface(id) => Ty::Iface(*id),
            Symbol::Enum(id) => Ty::Enum(*id),
            _ => {
                self.err(span, DiagKind::UnknownType { name: "value".into() });
                Ty::Error
            }
        }
    }

    fn resolve_signatures(&mut self) {
        let n = self.funcs.len();
        for i in 0..n {
            if self.funcs[i].file == u32::MAX {
                continue;
            }
            let file = self.funcs[i].file;
            let unresolved = self.funcs[i].unresolved_params.clone();
            let mut params = Vec::new();
            for (name, ty) in unresolved {
                params.push((name, self.resolve_type(file, &ty)));
            }
            let explicit = self.funcs[i].unresolved_ret.clone();
            self.funcs[i].params = params;
            if let Some(t) = explicit {
                self.funcs[i].ret = self.resolve_type(file, &t);
                self.funcs[i].infer = false;
            }
        }
        let n = self.classes.len();
        for i in 0..n {
            if self.classes[i].is_prelude {
                continue;
            }
            let file = self.classes[i].file;
            for field in &mut self.classes[i].own_fields {
                if field.ty_ast.is_none() {
                    // filled below
                }
            }
            let own = self.classes[i].own_fields.clone();
            let statics = self.classes[i].statics.clone();
            let mut own_tys = Vec::new();
            for f in &own {
                own_tys.push(self.own_field_ty(file, f));
            }
            let mut static_tys = Vec::new();
            for f in &statics {
                static_tys.push(self.own_field_ty(file, f));
            }
            for (f, ty) in self.classes[i].own_fields.iter_mut().zip(own_tys) {
                f.ty = ty;
            }
            for (f, ty) in self.classes[i].statics.iter_mut().zip(static_tys) {
                f.ty = ty;
            }
        }
    }

    fn own_field_ty(&mut self, file: u32, field: &OwnField) -> Ty {
        if let Some(t) = &field.ty_ast {
            self.resolve_type(file, t)
        } else {
            self.err(
                field.span,
                DiagKind::UnknownType {
                    name: field.name.clone(),
                },
            );
            Ty::Error
        }
    }

    fn layout_all(&mut self) {
        let n = self.classes.len();
        for i in 0..n {
            self.layout_class(i as u32);
        }
    }

    fn layout_class(&mut self, id: u32) {
        if self.classes[id as usize].layout_done {
            return;
        }
        if let Some(pid) = self.classes[id as usize].super_id {
            self.layout_class(pid);
            let parent = self.classes[pid as usize].layout.clone();
            self.classes[id as usize].layout = parent;
        }
        let own_names: Vec<String> = self.classes[id as usize]
            .own_fields
            .iter()
            .map(|f| f.name.clone())
            .collect();
        for name in own_names {
            if self.classes[id as usize]
                .layout
                .iter()
                .any(|f| f.name == name)
            {
                let span = self.classes[id as usize]
                    .own_fields
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| f.span)
                    .unwrap_or(self.classes[id as usize].span);
                self.err(span, DiagKind::Duplicate { name: name.clone() });
                continue;
            }
            let field = self.classes[id as usize]
                .own_fields
                .iter()
                .find(|f| f.name == name)
                .unwrap()
                .clone();
            let index = self.classes[id as usize].layout.len() as u16;
            if let Some(slot) = self.classes[id as usize]
                .own_fields
                .iter_mut()
                .find(|f| f.name == name)
            {
                slot.index = index;
            }
            self.classes[id as usize].layout.push(LayField {
                name: field.name,
                ty: field.ty,
                mutable: field.mutable,
                vis: field.vis,
                owner: id,
                index,
            });
        }
        for (i, f) in self.classes[id as usize].statics.iter_mut().enumerate() {
            f.index = i as u16;
        }
        self.classes[id as usize].layout_done = true;
    }

    fn build_vtables(&mut self) {
        let n = self.classes.len();
        for i in 0..n {
            self.build_vtable(i as u32);
        }
    }

    fn build_vtable(&mut self, id: u32) {
        if self.classes[id as usize].vtable_done {
            return;
        }
        if let Some(pid) = self.classes[id as usize].super_id {
            self.build_vtable(pid);
            let vt = self.classes[pid as usize].vtable.clone();
            let names = self.classes[pid as usize].vtable_names.clone();
            self.classes[id as usize].vtable = vt;
            self.classes[id as usize].vtable_names = names;
        }
        let methods = self.classes[id as usize].methods.clone();
        for mid in methods {
            if self.funcs[mid as usize].is_ctor || self.funcs[mid as usize].is_static {
                continue;
            }
            let name = self.funcs[mid as usize].simple.clone();
            let span = self.funcs[mid as usize].span;
            if let Some(slot) = self.classes[id as usize]
                .vtable_names
                .iter()
                .position(|n| n == &name)
            {
                let parent_id = self.classes[id as usize].vtable[slot];
                if self.funcs[parent_id as usize].is_final {
                    self.err(span, DiagKind::FinalMethod { name: name.clone() });
                }
                if !self.funcs[mid as usize].is_override {
                    self.err(span, DiagKind::NeedOverride { name: name.clone() });
                }
                self.match_override(parent_id, mid, span);
                self.classes[id as usize].vtable[slot] = mid;
                self.funcs[mid as usize].slot = Some(slot as u16);
            } else {
                if self.funcs[mid as usize].is_override {
                    self.err(span, DiagKind::NoOverride { name: name.clone() });
                }
                let slot = self.classes[id as usize].vtable.len() as u16;
                self.classes[id as usize].vtable.push(mid);
                self.classes[id as usize].vtable_names.push(name);
                self.funcs[mid as usize].slot = Some(slot);
            }
        }
        self.classes[id as usize].vtable_done = true;
    }

    fn match_override(&mut self, parent: u32, child: u32, span: Span) {
        let pp: Vec<Ty> = self.funcs[parent as usize]
            .params
            .iter()
            .map(|(_, t)| t.clone())
            .collect();
        let cp: Vec<Ty> = self.funcs[child as usize]
            .params
            .iter()
            .map(|(_, t)| t.clone())
            .collect();
        if pp != cp {
            self.err(
                span,
                DiagKind::TypeMismatch {
                    expected: self.fmt_sig(&pp),
                    found: self.fmt_sig(&cp),
                },
            );
        }
        let parent_infer = self.funcs[parent as usize].infer;
        let child_infer = self.funcs[child as usize].infer;
        if !parent_infer && child_infer {
            let ret = self.funcs[parent as usize].ret.clone();
            self.funcs[child as usize].ret = ret;
            self.funcs[child as usize].infer = false;
        } else if !parent_infer && !child_infer {
            let pr = self.funcs[parent as usize].ret.clone();
            let cr = self.funcs[child as usize].ret.clone();
            if pr != cr {
                self.err(
                    span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&pr),
                        found: self.fmt_ty(&cr),
                    },
                );
            }
        }
        if self.funcs[parent as usize].is_async != self.funcs[child as usize].is_async {
            self.err(
                span,
                DiagKind::BadModifier {
                    detail: "override changes async".into(),
                },
            );
        }
    }

    fn flatten_interfaces(&mut self) {
        let n = self.ifaces.len();
        for i in 0..n {
            self.flatten_iface(i as u32);
        }
    }

    fn flatten_iface(&mut self, id: u32) {
        if self.ifaces[id as usize].flat_done {
            return;
        }
        let parents = self.ifaces[id as usize].parents.clone();
        for p in &parents {
            self.flatten_iface(*p);
        }
        let mut methods: Vec<IfaceMethod> = Vec::new();
        for p in &parents {
            for m in &self.ifaces[*p as usize].methods {
                if let Some(existing) = methods.iter_mut().find(|e| e.name == m.name) {
                    *existing = m.clone();
                } else {
                    methods.push(m.clone());
                }
            }
        }
        let file = self.ifaces[id as usize].file;
        let own = self.ifaces[id as usize].own.clone();
        for m in own {
            let params: Vec<Ty> = m.params.iter().map(|t| self.resolve_type(file, t)).collect();
            let ret = if let Some(t) = &m.ret {
                self.resolve_type(file, t)
            } else {
                Ty::Void
            };
            let method = IfaceMethod {
                name: m.name.clone(),
                params,
                ret,
                is_async: m.is_async,
            };
            if let Some(existing) = methods.iter_mut().find(|e| e.name == method.name) {
                *existing = method;
            } else {
                methods.push(method);
            }
        }
        self.ifaces[id as usize].methods = methods;
        self.ifaces[id as usize].flat_done = true;
    }

    fn build_itables(&mut self) {
        let n = self.classes.len();
        for i in 0..n {
            let ifaces = self.all_ifaces(i as u32);
            let mut tables = Vec::new();
            for iid in ifaces {
                let methods = self.ifaces[iid as usize].methods.clone();
                let class_name = self.classes[i].name.clone();
                let span = self.classes[i].span;
                let mut ids = Vec::new();
                for m in &methods {
                    if let Some(fid) = self.find_instance_method(i as u32, &m.name) {
                        let params: Vec<Ty> = self.funcs[fid as usize]
                            .params
                            .iter()
                            .map(|(_, t)| t.clone())
                            .collect();
                        if params != m.params {
                            self.err(
                                span,
                                DiagKind::MissingImpl {
                                    class: class_name.clone(),
                                    method: m.name.clone(),
                                },
                            );
                        }
                        if self.funcs[fid as usize].infer {
                            self.funcs[fid as usize].ret = m.ret.clone();
                            self.funcs[fid as usize].infer = false;
                        } else if self.funcs[fid as usize].ret != m.ret {
                            self.err(
                                self.funcs[fid as usize].span,
                                DiagKind::TypeMismatch {
                                    expected: self.fmt_ty(&m.ret),
                                    found: self.fmt_ty(&self.funcs[fid as usize].ret.clone()),
                                },
                            );
                        }
                        ids.push(fid);
                    } else {
                        self.err(
                            span,
                            DiagKind::MissingImpl {
                                class: class_name.clone(),
                                method: m.name.clone(),
                            },
                        );
                        ids.push(0);
                    }
                }
                tables.push((iid, ids));
            }
            self.classes[i].itables = tables;
        }
    }

    fn all_ifaces(&self, class: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![class];
        let mut seen = HashSet::new();
        while let Some(c) = stack.pop() {
            if !seen.insert(c) {
                continue;
            }
            for i in &self.classes[c as usize].iface_ids {
                if !out.contains(i) {
                    out.push(*i);
                }
            }
            if let Some(p) = self.classes[c as usize].super_id {
                stack.push(p);
            }
        }
        let mut i = 0;
        while i < out.len() {
            let id = out[i];
            for p in &self.ifaces[id as usize].parents {
                if !out.contains(p) {
                    out.push(*p);
                }
            }
            i += 1;
        }
        out
    }

    fn find_instance_method(&self, class: u32, name: &str) -> Option<u32> {
        let mut c = Some(class);
        while let Some(id) = c {
            for mid in &self.classes[id as usize].methods {
                let f = &self.funcs[*mid as usize];
                if f.simple == name && !f.is_ctor && !f.is_static {
                    return Some(*mid);
                }
            }
            c = self.classes[id as usize].super_id;
        }
        None
    }

    fn find_any_method(&self, class: u32, name: &str) -> Option<u32> {
        let mut c = Some(class);
        while let Some(id) = c {
            for mid in &self.classes[id as usize].methods {
                let f = &self.funcs[*mid as usize];
                if f.simple == name && !f.is_ctor {
                    return Some(*mid);
                }
            }
            c = self.classes[id as usize].super_id;
        }
        None
    }

    fn eval_package_consts(&mut self) {
        for i in 0..self.consts.len() {
            let file = self.consts[i].file;
            if let Some(t) = self.consts[i].ty_ast.clone() {
                let ty = self.resolve_type(file, &t);
                self.consts[i].ty = ty;
            }
        }
        for i in 0..self.consts.len() {
            let v = self.force_const(i as u32);
            if self.consts[i].ty.is_error() {
                if let Some(v) = &v {
                    self.consts[i].ty = const_ty(v);
                }
            }
        }
    }

    fn force_const(&mut self, id: u32) -> Option<ConstValue> {
        if let Some(v) = self.consts[id as usize].value.clone() {
            return Some(v);
        }
        if self.consts[id as usize].evaluating {
            let name = self.consts[id as usize].name.clone();
            let span = self.consts[id as usize].span;
            self.err(span, DiagKind::Cycle { name });
            return None;
        }
        self.consts[id as usize].evaluating = true;
        let init = self.consts[id as usize].init.clone();
        let file = self.consts[id as usize].file;
        let value = self.eval_const_ast(&init, file);
        self.consts[id as usize].evaluating = false;
        if let Some(v) = &value {
            let expect = self.consts[id as usize].ty.clone();
            if !expect.is_error() && !self.assignable(&const_ty(v), &expect) {
                let span = self.consts[id as usize].span;
                self.err(
                    span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&expect),
                        found: self.fmt_ty(&const_ty(v)),
                    },
                );
            }
            self.consts[id as usize].value = Some(v.clone());
        } else {
            let span = self.consts[id as usize].span;
            self.err(span, DiagKind::NeedConst);
        }
        value
    }

    fn eval_static_fields(&mut self) {
        for c in 0..self.classes.len() {
            if self.classes[c].file == u32::MAX {
                continue;
            }
            let file = self.classes[c].file;
            let statics = self.classes[c].statics.clone();
            for f in statics {
                let value = if let Some(init) = &f.init {
                    let v = self.eval_const_ast(init, file);
                    if v.is_none() {
                        self.err(f.span, DiagKind::NeedConst);
                    }
                    v
                } else if f.is_const {
                    self.err(f.span, DiagKind::NeedConst);
                    None
                } else {
                    Some(default_const(&f.ty))
                };
                if let Some(value) = value {
                    if !f.ty.is_error() && !self.assignable(&const_ty(&value), &f.ty) {
                        self.err(
                            f.span,
                            DiagKind::TypeMismatch {
                                expected: self.fmt_ty(&f.ty),
                                found: self.fmt_ty(&const_ty(&value)),
                            },
                        );
                    }
                    self.static_inits.push(StaticInit {
                        class: c as u32,
                        field: f.index,
                        value,
                    });
                }
            }
        }
    }

    fn eval_const_ast(&mut self, expr: &ast::Expr, file: u32) -> Option<ConstValue> {
        match &expr.kind {
            ast::ExprKind::Int(n) => Some(ConstValue::Int(*n)),
            ast::ExprKind::Float(n) => Some(ConstValue::Float(*n)),
            ast::ExprKind::Double(n) => Some(ConstValue::Double(*n)),
            ast::ExprKind::Bool(b) => Some(ConstValue::Bool(*b)),
            ast::ExprKind::Str(s) => Some(ConstValue::Str(s.clone())),
            ast::ExprKind::Null => Some(ConstValue::Null),
            ast::ExprKind::Unary { op: AstUn::Neg, expr } => match self.eval_const_ast(expr, file)? {
                ConstValue::Int(n) => Some(ConstValue::Int(n.wrapping_neg())),
                ConstValue::Float(n) => Some(ConstValue::Float(-n)),
                ConstValue::Double(n) => Some(ConstValue::Double(-n)),
                _ => None,
            },
            ast::ExprKind::Unary { op: AstUn::Not, expr } => {
                match self.eval_const_ast(expr, file)? {
                    ConstValue::Bool(b) => Some(ConstValue::Bool(!b)),
                    _ => None,
                }
            }
            ast::ExprKind::Binary { op, left, right } => {
                let l = self.eval_const_ast(left, file)?;
                let r = self.eval_const_ast(right, file)?;
                eval_bin(*op, l, r)
            }
            ast::ExprKind::Name(name) => match self.lookup_name(file, name)? {
                Symbol::Const(id) => self.force_const(id),
                _ => None,
            },
            ast::ExprKind::Field { recv, name } => {
                let ast::ExprKind::Name(enum_name) = &recv.kind else {
                    return None;
                };
                let Symbol::Enum(id) = self.lookup_name(file, enum_name)? else {
                    return None;
                };
                let idx = self.enums[id as usize]
                    .variants
                    .iter()
                    .position(|v| v == name)?;
                Some(ConstValue::Enum {
                    id,
                    variant: idx as u16,
                })
            }
            _ => None,
        }
    }

    fn ensure_func(&mut self, id: u32) {
        if self.funcs[id as usize].done {
            return;
        }
        if self.checking.contains(&id) {
            if self.funcs[id as usize].infer {
                let name = self.funcs[id as usize].simple.clone();
                let span = self.funcs[id as usize].span;
                self.err(
                    span,
                    DiagKind::BadModifier {
                        detail: format!("recursive function '{name}' needs -> Type"),
                    },
                );
                self.funcs[id as usize].infer = false;
            }
            return;
        }
        self.checking.push(id);
        let saved = self.env.take();
        let body = std::mem::replace(&mut self.funcs[id as usize].body, BodyKind::Empty);
        match body {
            BodyKind::Ast(block) => self.lower_user_func(id, &block),
            BodyKind::DefaultCtor => self.synth_default_ctor(id),
            BodyKind::SetMessage => self.synth_set_message(id),
            BodyKind::SuperMsg(msg) => self.synth_super_msg(id, &msg),
            BodyKind::Empty => {}
        }
        self.funcs[id as usize].done = true;
        self.env = saved;
        self.checking.pop();
    }

    fn lower_user_func(&mut self, id: u32, block: &ast::Block) {
        self.begin_env(id);
        let is_ctor = self.funcs[id as usize].is_ctor;
        let class = self.funcs[id as usize].class;
        let body = if is_ctor {
            self.lower_ctor(class.unwrap(), block)
        } else {
            self.lower_stmts(&block.stmts)
        };
        self.funcs[id as usize].hir = body.clone();
        self.end_env(id);
        self.check_returns(id, &body);
    }

    fn begin_env(&mut self, id: u32) {
        let f = &self.funcs[id as usize];
        self.env = Some(FuncEnv {
            id,
            class: f.class,
            is_ctor: f.is_ctor,
            is_static: f.is_static,
            ret: f.ret.clone(),
            infer: f.infer,
            has_value: false,
            locals: Vec::new(),
            scopes: vec![HashMap::new()],
            loop_depth: 0,
            saw_super: false,
        });
        if f.class.is_some() && !f.is_static {
            let class = f.class.unwrap();
            self.alloc_local("self", Ty::Class(class), false, None);
        }
        let params = self.funcs[id as usize].params.clone();
        for (name, ty) in params {
            self.alloc_local(&name, ty, false, None);
        }
    }

    fn end_env(&mut self, id: u32) {
        let env = self.env.take().unwrap();
        self.funcs[id as usize].locals = env.locals;
        self.funcs[id as usize].ret = env.ret;
        self.funcs[id as usize].infer = env.infer && !env.has_value;
    }

    fn synth_set_message(&mut self, id: u32) {
        self.begin_env(id);
        let span = Span::new(0, 0, 0);
        let recv = self.self_expr(span);
        let value = self.hx(Ty::String, span, ExprKind::Local(1));
        let stmt = self.hs(span, StmtKind::Assign {
            place: Place::Field { recv, index: 0 },
            value,
        });
        self.funcs[id as usize].hir = vec![stmt];
        self.end_env(id);
    }

    fn synth_super_msg(&mut self, id: u32, msg: &str) {
        self.begin_env(id);
        let class = self.funcs[id as usize].class.unwrap();
        let parent = self.classes[class as usize].super_id.unwrap();
        let ctor = self.classes[parent as usize].ctor.unwrap();
        let span = self.funcs[id as usize].span;
        let arg = self.hx(Ty::String, span, ExprKind::Str(msg.into()));
        let stmt = self.hs(span, StmtKind::Super { ctor, args: vec![arg] });
        self.funcs[id as usize].hir = vec![stmt];
        self.end_env(id);
    }

    fn synth_default_ctor(&mut self, id: u32) {
        self.begin_env(id);
        let class = self.funcs[id as usize].class.unwrap();
        let span = self.funcs[id as usize].span;
        let mut body = Vec::new();
        if let Some(s) = self.implicit_super(class, span) {
            body.push(s);
        }
        body.extend(self.field_init_stmts(class));
        self.funcs[id as usize].hir = body;
        self.end_env(id);
    }

    fn implicit_super(&mut self, class: u32, span: Span) -> Option<Stmt> {
        let parent = self.classes[class as usize].super_id?;
        let ctor = self.classes[parent as usize].ctor?;
        if !self.funcs[ctor as usize].params.is_empty() {
            let name = self.classes[parent as usize].name.clone();
            self.err(span, DiagKind::NeedCtor { parent: name });
            return None;
        }
        if let Some(env) = self.env.as_mut() {
            env.saw_super = true;
        }
        Some(self.hs(span, StmtKind::Super { ctor, args: Vec::new() }))
    }

    fn field_init_stmts(&mut self, class: u32) -> Vec<Stmt> {
        let fields = self.classes[class as usize].own_fields.clone();
        let mut out = Vec::new();
        for f in fields {
            let value = if let Some(init) = &f.init {
                let e = self.lower_expr(init);
                if f.is_const && self.fold(&e).is_none() && !e.ty.is_error() {
                    self.err(f.span, DiagKind::NeedConst);
                }
                if !f.ty.is_error() && !self.assignable(&e.ty, &f.ty) {
                    self.err(
                        f.span,
                        DiagKind::TypeMismatch {
                            expected: self.fmt_ty(&f.ty),
                            found: self.fmt_ty(&e.ty),
                        },
                    );
                }
                e
            } else {
                self.default_expr(&f.ty, f.span)
            };
            let recv = self.self_expr(f.span);
            out.push(self.hs(f.span, StmtKind::Assign {
                place: Place::Field { recv, index: f.index },
                value,
            }));
        }
        out
    }

    fn lower_ctor(&mut self, class: u32, block: &ast::Block) -> Vec<Stmt> {
        let stmts: Vec<&ast::Stmt> = block
            .stmts
            .iter()
            .filter(|s| !matches!(s, ast::Stmt::Empty))
            .collect();
        let mut out = Vec::new();
        let mut rest: Vec<ast::Stmt> = stmts.into_iter().cloned().collect();
        if rest.first().is_some_and(is_super_stmt) {
            let first = rest.remove(0);
            if let Some(s) = self.lower_stmt(&first) {
                out.push(s);
            }
            if let Some(env) = self.env.as_mut() {
                env.saw_super = true;
            }
        } else if let Some(s) = self.implicit_super(class, block.span) {
            out.push(s);
        }
        out.extend(self.field_init_stmts(class));
        out.extend(self.lower_stmts(&rest));
        out
    }

    fn lower_stmts(&mut self, stmts: &[ast::Stmt]) -> Vec<Stmt> {
        let mut out = Vec::new();
        let mut dead = false;
        for s in stmts {
            if matches!(s, ast::Stmt::Empty) {
                continue;
            }
            if dead {
                self.err(stmt_span(s), DiagKind::Unreachable);
                break;
            }
            if let Some(h) = self.lower_stmt(s) {
                if diverges(&h) {
                    dead = true;
                }
                out.push(h);
            }
        }
        out
    }

    fn lower_block(&mut self, block: &ast::Block) -> Vec<Stmt> {
        self.push_scope();
        let stmts = self.lower_stmts(&block.stmts);
        self.pop_scope();
        stmts
    }

    fn lower_stmt(&mut self, stmt: &ast::Stmt) -> Option<Stmt> {
        Some(match stmt {
            ast::Stmt::Empty => return None,
            ast::Stmt::Local { mutable, name, ty, init, span } => {
                self.lower_local(*mutable, name, ty.as_ref(), init.as_ref(), *span)
            }
            ast::Stmt::If { cond, then_branch, else_branch, span } => {
                let cond = self.lower_expr(cond);
                self.check_bool(&cond.ty, cond.span);
                let then_body = self.lower_block(then_branch);
                let else_body = else_branch.as_ref().map(|s| match s.as_ref() {
                    ast::Stmt::Block(b) => self.lower_block(b),
                    other => self.lower_stmt(other).into_iter().collect(),
                });
                self.hs(*span, StmtKind::If { cond, then_body, else_body })
            }
            ast::Stmt::While { cond, body, span } => {
                let cond = self.lower_expr(cond);
                self.check_bool(&cond.ty, cond.span);
                self.bump_loop(1);
                let body = self.lower_block(body);
                self.bump_loop(-1);
                self.hs(*span, StmtKind::While { cond, body })
            }
            ast::Stmt::Loop { body, span } => {
                self.bump_loop(1);
                let body = self.lower_block(body);
                self.bump_loop(-1);
                self.hs(*span, StmtKind::Loop { body })
            }
            ast::Stmt::ForC { init, cond, step, body, span } => {
                self.push_scope();
                let init = init.as_ref().and_then(|s| self.lower_stmt(s)).map(Box::new);
                let cond = cond.as_ref().map(|e| {
                    let e = self.lower_expr(e);
                    self.check_bool(&e.ty, e.span);
                    e
                });
                let step = step.as_ref().map(|e| Box::new(self.lower_step(e)));
                self.bump_loop(1);
                let body = self.lower_block(body);
                self.bump_loop(-1);
                self.pop_scope();
                self.hs(*span, StmtKind::ForC { init, cond, step, body })
            }
            ast::Stmt::ForIn { name, iter, body, span } => {
                let iter = self.lower_expr(iter);
                let elem = match &iter.ty {
                    Ty::Array(t) => (**t).clone(),
                    Ty::Range => Ty::Int,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(*span, DiagKind::BadIter);
                        Ty::Error
                    }
                };
                self.push_scope();
                let slot = self.alloc_local(name, elem, false, None);
                self.bump_loop(1);
                let body = self.lower_block(body);
                self.bump_loop(-1);
                self.pop_scope();
                self.hs(*span, StmtKind::ForIn { slot, iter, body })
            }
            ast::Stmt::DoWhile { body, cond, span } => {
                self.bump_loop(1);
                let body = self.lower_block(body);
                self.bump_loop(-1);
                let cond = self.lower_expr(cond);
                self.check_bool(&cond.ty, cond.span);
                self.hs(*span, StmtKind::DoWhile { body, cond })
            }
            ast::Stmt::Match { scrut, arms, span } => self.lower_match(scrut, arms, *span),
            ast::Stmt::Try { body, catches, span } => {
                let body = self.lower_block(body);
                let mut out = Vec::new();
                for c in catches {
                    let ty = self.resolve_type(self.current_file(), &c.ty);
                    let class = match ty {
                        Ty::Class(id) if self.classes[id as usize].is_exception => id,
                        Ty::Error => 0,
                        _ => {
                            self.err(c.ty.span, DiagKind::NotThrowable { name: self.fmt_ty(&ty) });
                            0
                        }
                    };
                    self.push_scope();
                    let slot = self.alloc_local(&c.name, Ty::Class(class), false, None);
                    let cbody = self.lower_block(&c.body);
                    self.pop_scope();
                    out.push(Catch { slot, class, body: cbody });
                }
                self.hs(*span, StmtKind::Try { body, catches: out })
            }
            ast::Stmt::Return { value, span } => self.lower_return(value.as_ref(), *span),
            ast::Stmt::Break { span } => {
                self.need_loop("break", *span);
                self.hs(*span, StmtKind::Break)
            }
            ast::Stmt::Continue { span } => {
                self.need_loop("continue", *span);
                self.hs(*span, StmtKind::Continue)
            }
            ast::Stmt::Throw { value, span } => {
                let value = self.lower_expr(value);
                self.expect_exception(&value);
                self.hs(*span, StmtKind::Throw { value })
            }
            ast::Stmt::Delete { value, span } => {
                let value = self.lower_expr(value);
                if !value.ty.is_error() && !value.ty.is_ref() {
                    self.err(*span, DiagKind::BadAssign);
                }
                self.hs(*span, StmtKind::Delete { value })
            }
            ast::Stmt::Expr { expr, span } => {
                if let ast::ExprKind::Assign { left, right } = &expr.kind {
                    let place = self.lower_place(left);
                    let value = self.lower_expr(right);
                    self.assign_ok(&place, &value);
                    return Some(self.hs(*span, StmtKind::Assign { place, value }));
                }
                if let ast::ExprKind::SuperCall { args } = &expr.kind {
                    return Some(self.lower_super_call(args, *span));
                }
                let expr = self.lower_expr(expr);
                self.hs(*span, StmtKind::Expr(expr))
            }
            ast::Stmt::Block(b) => {
                let span = b.span;
                let then_body = self.lower_block(b);
                self.hs(span, StmtKind::If {
                    cond: self.hx(Ty::Bool, span, ExprKind::Bool(true)),
                    then_body,
                    else_body: None,
                })
            }
        })
    }

    fn lower_local(
        &mut self,
        mutable: Mutability,
        name: &str,
        ty: Option<&ast::TypeRef>,
        init: Option<&ast::Expr>,
        span: Span,
    ) -> Stmt {
        let init_e = init.map(|e| self.lower_expr(e));
        let annotated = ty.map(|t| self.resolve_type(self.current_file(), t));
        let ty = if let Some(t) = annotated.clone() {
            t
        } else if let Some(e) = &init_e {
            e.ty.clone()
        } else {
            self.err(span, DiagKind::UnknownType { name: name.into() });
            Ty::Error
        };
        if mutable != Mutability::Mut && init_e.is_none() {
            self.err(
                span,
                DiagKind::BadModifier {
                    detail: format!("'{name}' needs an initializer"),
                },
            );
        }
        if let Some(e) = &init_e {
            if !self.assignable(&e.ty, &ty) {
                self.err(
                    span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&ty),
                        found: self.fmt_ty(&e.ty),
                    },
                );
            }
        }
        let folded = if mutable == Mutability::Const {
            match init_e.as_ref().and_then(|e| self.fold(e)) {
                Some(v) => Some(v),
                None => {
                    self.err(span, DiagKind::NeedConst);
                    None
                }
            }
        } else {
            None
        };
        let slot = self.alloc_local(name, ty.clone(), mutable == Mutability::Mut, folded.clone());
        let init = if let Some(v) = folded {
            Some(self.const_expr(v, ty, span))
        } else if let Some(e) = init_e {
            Some(e)
        } else {
            Some(self.default_expr(&ty, span))
        };
        self.hs(span, StmtKind::Local { slot, init })
    }

    fn lower_step(&mut self, expr: &ast::Expr) -> Stmt {
        if let ast::ExprKind::Assign { left, right } = &expr.kind {
            let place = self.lower_place(left);
            let value = self.lower_expr(right);
            self.assign_ok(&place, &value);
            return self.hs(expr.span, StmtKind::Assign { place, value });
        }
        let e = self.lower_expr(expr);
        self.hs(expr.span, StmtKind::Expr(e))
    }

    fn lower_super_call(&mut self, args: &[ast::Expr], span: Span) -> Stmt {
        let is_ctor = self.env.as_ref().is_some_and(|e| e.is_ctor);
        let saw = self.env.as_ref().is_some_and(|e| e.saw_super);
        if !is_ctor || saw {
            self.err(span, if is_ctor { DiagKind::SuperFirst } else { DiagKind::BadSuper });
        }
        let class = self.env.as_ref().and_then(|e| e.class);
        let args: Vec<Expr> = args.iter().map(|a| self.lower_expr(a)).collect();
        let Some(class) = class else {
            return self.hs(span, StmtKind::Expr(self.poison(span)));
        };
        let Some(parent) = self.classes[class as usize].super_id else {
            self.err(span, DiagKind::BadSuper);
            return self.hs(span, StmtKind::Expr(self.poison(span)));
        };
        let Some(ctor) = self.classes[parent as usize].ctor else {
            self.err(span, DiagKind::BadSuper);
            return self.hs(span, StmtKind::Expr(self.poison(span)));
        };
        let params: Vec<Ty> = self.funcs[ctor as usize]
            .params
            .iter()
            .map(|(_, t)| t.clone())
            .collect();
        let name = self.classes[parent as usize].name.clone();
        self.check_args(&name, &params, &args, span);
        self.hs(span, StmtKind::Super { ctor, args })
    }

    fn lower_return(&mut self, value: Option<&ast::Expr>, span: Span) -> Stmt {
        let expr = value.map(|v| self.lower_expr(v));
        let is_ctor = self.env.as_ref().is_some_and(|e| e.is_ctor);
        if is_ctor && expr.is_some() {
            self.err(span, DiagKind::CtorReturn);
        }
        if let Some(e) = &expr {
            let infer = self.env.as_ref().is_some_and(|env| env.infer);
            let has = self.env.as_ref().is_some_and(|env| env.has_value);
            let current = self.env.as_ref().map(|env| env.ret.clone()).unwrap_or(Ty::Void);
            if !is_ctor && infer && !has && matches!(current, Ty::Void | Ty::Error) && !e.ty.is_error() {
                let ty = e.ty.clone();
                let id = self.env.as_ref().unwrap().id;
                self.env.as_mut().unwrap().ret = ty.clone();
                self.env.as_mut().unwrap().has_value = true;
                self.env.as_mut().unwrap().infer = false;
                self.funcs[id as usize].ret = ty;
            } else if !is_ctor && !self.assignable(&e.ty, &current) {
                self.err(
                    span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&current),
                        found: self.fmt_ty(&e.ty),
                    },
                );
            }
        } else if !is_ctor {
            let current = self.env.as_ref().map(|env| env.ret.clone()).unwrap_or(Ty::Void);
            let has = self.env.as_ref().is_some_and(|env| env.has_value);
            if has || !matches!(current, Ty::Void | Ty::Error) {
                self.err(
                    span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&current),
                        found: "void".into(),
                    },
                );
            }
        }
        self.hs(span, StmtKind::Return { value: expr })
    }

    fn lower_match(&mut self, scrut: &ast::Expr, arms: &[ast::MatchArm], span: Span) -> Stmt {
        let scrut_e = self.lower_expr(scrut);
        let mut seen = HashSet::new();
        let mut wildcard = false;
        let mut out = Vec::new();
        for arm in arms {
            let pat = match &arm.pattern {
                ast::Pattern::Wildcard { .. } => {
                    wildcard = true;
                    Pat::Wildcard
                }
                ast::Pattern::Expr(e) => match self.eval_const_ast(e, self.current_file()).or_else(|| {
                    let v = self.lower_expr(e);
                    self.fold(&v)
                }) {
                    Some(v @ ConstValue::Enum { .. }) => {
                        if let ConstValue::Enum { variant, .. } = &v {
                            seen.insert(*variant);
                        }
                        Pat::Value(v)
                    }
                    Some(v) => Pat::Value(v),
                    None => {
                        self.err(arm.span, DiagKind::BadPattern);
                        Pat::Wildcard
                    }
                },
            };
            let body = self.lower_block(&arm.body);
            out.push(Arm { pat, body, span: arm.span });
        }
        if let Ty::Enum(id) = scrut_e.ty {
            if !wildcard && seen.len() != self.enums[id as usize].variants.len() {
                self.err(span, DiagKind::NonExhaustive);
            }
        } else if !wildcard && !scrut_e.ty.is_error() {
            self.err(span, DiagKind::NonExhaustive);
        }
        self.hs(span, StmtKind::Match { scrut: scrut_e, arms: out })
    }

    fn lower_expr(&mut self, expr: &ast::Expr) -> Expr {
        match &expr.kind {
            ast::ExprKind::Int(n) => self.hx(Ty::Int, expr.span, ExprKind::Int(*n)),
            ast::ExprKind::Float(n) => self.hx(Ty::Float, expr.span, ExprKind::Float(*n)),
            ast::ExprKind::Double(n) => self.hx(Ty::Double, expr.span, ExprKind::Double(*n)),
            ast::ExprKind::Bool(b) => self.hx(Ty::Bool, expr.span, ExprKind::Bool(*b)),
            ast::ExprKind::Str(s) => self.hx(Ty::String, expr.span, ExprKind::Str(s.clone())),
            ast::ExprKind::Null => self.hx(Ty::Null, expr.span, ExprKind::Null),
            ast::ExprKind::Name(name) => self.lower_name(name, expr.span),
            ast::ExprKind::SelfRef => self.self_expr(expr.span),
            ast::ExprKind::Unary { op, expr: inner } => {
                let e = self.lower_expr(inner);
                let (ty, hop) = match op {
                    AstUn::Not => {
                        self.check_bool(&e.ty, expr.span);
                        (Ty::Bool, UnaryOp::Not)
                    }
                    AstUn::Neg => {
                        if !e.ty.is_numeric() && !e.ty.is_error() {
                            self.err(
                                expr.span,
                                DiagKind::TypeMismatch {
                                    expected: "number".into(),
                                    found: self.fmt_ty(&e.ty),
                                },
                            );
                        }
                        (e.ty.clone(), UnaryOp::Neg)
                    }
                };
                self.hx(ty, expr.span, ExprKind::Unary { op: hop, expr: Box::new(e) })
            }
            ast::ExprKind::Binary { op, left, right } => self.lower_binary(*op, left, right, expr.span),
            ast::ExprKind::Assign { .. } => {
                self.err(expr.span, DiagKind::BadAssign);
                self.poison(expr.span)
            }
            ast::ExprKind::Call { callee, args } => self.lower_call(callee, args, expr.span),
            ast::ExprKind::Field { recv, name } => self.lower_field(recv, name, expr.span),
            ast::ExprKind::Index { recv, index } => {
                let recv = self.lower_expr(recv);
                let index = self.lower_expr(index);
                if !index.ty.is_error() && !matches!(index.ty, Ty::Int) {
                    self.err(index.span, DiagKind::NeedIntIndex);
                }
                let ty = match &recv.ty {
                    Ty::Array(t) => (**t).clone(),
                    Ty::Error => Ty::Error,
                    _ => {
                        self.err(expr.span, DiagKind::BadIter);
                        Ty::Error
                    }
                };
                self.hx(ty, expr.span, ExprKind::Index { recv: Box::new(recv), index: Box::new(index) })
            }
            ast::ExprKind::New { ty, args } => self.lower_new(ty, args, expr.span),
            ast::ExprKind::NewArray { ty, len } => {
                let elem = self.resolve_type(self.current_file(), ty);
                let len_e = self.lower_expr(len);
                if !len_e.ty.is_error() && !matches!(len_e.ty, Ty::Int) {
                    self.err(len_e.span, DiagKind::NeedIntIndex);
                }
                self.hx(Ty::Array(Box::new(elem)), expr.span, ExprKind::NewArray { len: Box::new(len_e) })
            }
            ast::ExprKind::Array { elems } => {
                if elems.is_empty() {
                    self.err(expr.span, DiagKind::EmptyArray);
                    return self.poison(expr.span);
                }
                let elems: Vec<Expr> = elems.iter().map(|e| self.lower_expr(e)).collect();
                let ty = elems[0].ty.clone();
                for e in &elems[1..] {
                    if !self.assignable(&e.ty, &ty) {
                        self.err(
                            e.span,
                            DiagKind::TypeMismatch {
                                expected: self.fmt_ty(&ty),
                                found: self.fmt_ty(&e.ty),
                            },
                        );
                    }
                }
                self.hx(Ty::Array(Box::new(ty)), expr.span, ExprKind::Array { elems })
            }
            ast::ExprKind::Range { lo, hi } => {
                let lo = self.lower_expr(lo);
                let hi = self.lower_expr(hi);
                if !matches!(lo.ty, Ty::Int | Ty::Error) || !matches!(hi.ty, Ty::Int | Ty::Error) {
                    self.err(expr.span, DiagKind::TypeMismatch { expected: "int".into(), found: "range bound".into() });
                }
                self.hx(Ty::Range, expr.span, ExprKind::Range { lo: Box::new(lo), hi: Box::new(hi) })
            }
            ast::ExprKind::Await { expr: inner } => {
                let e = self.lower_expr(inner);
                match &e.ty {
                    Ty::Task(t) => self.hx((**t).clone(), expr.span, ExprKind::Await { expr: Box::new(e) }),
                    Ty::Error => self.poison(expr.span),
                    _ => {
                        self.err(expr.span, DiagKind::NotTask);
                        self.poison(expr.span)
                    }
                }
            }
            ast::ExprKind::SuperCall { .. } | ast::ExprKind::SuperField { .. } => {
                self.err(expr.span, DiagKind::BadSuper);
                self.poison(expr.span)
            }
        }
    }

    fn lower_binary(&mut self, op: AstBin, left: &ast::Expr, right: &ast::Expr, span: Span) -> Expr {
        let l = self.lower_expr(left);
        let r = self.lower_expr(right);
        let hop = map_bin(op);
        let ty = if l.ty.is_error() || r.ty.is_error() {
            if matches!(op, AstBin::Eq | AstBin::Ne | AstBin::Lt | AstBin::Le | AstBin::Gt | AstBin::Ge | AstBin::And | AstBin::Or) {
                Ty::Bool
            } else {
                Ty::Error
            }
        } else {
            match op {
                AstBin::And | AstBin::Or => {
                    self.check_bool(&l.ty, span);
                    self.check_bool(&r.ty, span);
                    Ty::Bool
                }
                AstBin::Eq | AstBin::Ne => {
                    if !self.comparable(&l.ty, &r.ty) {
                        self.err(span, DiagKind::TypeMismatch { expected: self.fmt_ty(&l.ty), found: self.fmt_ty(&r.ty) });
                    }
                    Ty::Bool
                }
                AstBin::Lt | AstBin::Le | AstBin::Gt | AstBin::Ge => {
                    if !((l.ty.is_numeric() && l.ty == r.ty) || (matches!(l.ty, Ty::String) && matches!(r.ty, Ty::String))) {
                        self.err(span, DiagKind::TypeMismatch { expected: self.fmt_ty(&l.ty), found: self.fmt_ty(&r.ty) });
                    }
                    Ty::Bool
                }
                AstBin::Add if matches!(l.ty, Ty::String) && matches!(r.ty, Ty::String) => Ty::String,
                AstBin::Add | AstBin::Sub | AstBin::Mul | AstBin::Div | AstBin::Mod => {
                    if l.ty.is_numeric() && l.ty == r.ty {
                        l.ty.clone()
                    } else {
                        self.err(span, DiagKind::TypeMismatch { expected: self.fmt_ty(&l.ty), found: self.fmt_ty(&r.ty) });
                        Ty::Error
                    }
                }
            }
        };
        self.hx(ty, span, ExprKind::Binary { op: hop, left: Box::new(l), right: Box::new(r) })
    }

    fn lower_name(&mut self, name: &str, span: Span) -> Expr {
        if let Some(b) = self.lookup_local(name) {
            if let Some(cv) = b.const_value.clone() {
                return self.const_expr(cv, b.ty.clone(), span);
            }
            return self.hx(b.ty.clone(), span, ExprKind::Local(b.slot));
        }
        if let Some(class) = self.env.as_ref().and_then(|e| if e.is_static { None } else { e.class }) {
            if let Some(f) = self.layout_field(class, name) {
                if !self.field_visible(&f) {
                    self.err(span, DiagKind::Private { name: name.into() });
                }
                let recv = self.self_expr(span);
                return self.hx(f.ty.clone(), span, ExprKind::Field { recv: Box::new(recv), index: f.index });
            }
        }
        let file = self.current_file();
        match self.lookup_name(file, name) {
            Some(Symbol::Const(id)) => {
                if let Some(cv) = self.force_const(id) {
                    let ty = self.consts[id as usize].ty.clone();
                    return self.const_expr(cv, ty, span);
                }
                self.poison(span)
            }
            Some(Symbol::Func(_)) | Some(Symbol::Builtin(_)) | Some(Symbol::Class(_)) | Some(Symbol::Iface(_)) | Some(Symbol::Enum(_)) => {
                self.err(span, DiagKind::NotCallable { name: name.into() });
                self.poison(span)
            }
            _ => {
                self.err(span, DiagKind::UndefinedName { name: name.into() });
                self.poison(span)
            }
        }
    }

    fn lower_field(&mut self, recv: &ast::Expr, name: &str, span: Span) -> Expr {
        if let Some(path) = flatten(recv) {
            let mut path = path;
            path.push(name.to_string());
            if let Some(e) = self.try_path_value(&path, span) {
                return e;
            }
        }
        if matches!(recv.kind, ast::ExprKind::SuperField { .. } | ast::ExprKind::SelfRef) || true {
            let recv_e = if let ast::ExprKind::SuperField { name: super_name } = &recv.kind {
                return self.call_super_value(super_name, name, span);
            } else {
                self.lower_expr(recv)
            };
            self.field_access(recv_e, name, span)
        } else {
            self.poison(span)
        }
    }

    fn try_path_value(&mut self, path: &[String], span: Span) -> Option<Expr> {
        if path.len() == 2 {
            let file = self.current_file();
            if let Some(Symbol::Enum(id)) = self.lookup_name(file, &path[0]) {
                if let Some(idx) = self.enums[id as usize].variants.iter().position(|v| v == &path[1]) {
                    return Some(self.hx(Ty::Enum(id), span, ExprKind::Enum { enum_id: id, variant: idx as u16 }));
                }
                self.err(span, DiagKind::NoField { name: path[1].clone() });
                return Some(self.poison(span));
            }
            if let Some(Symbol::Class(cid)) = self.lookup_name(file, &path[0]) {
                if let Some(f) = self.classes[cid as usize].statics.iter().find(|f| f.name == path[1]).cloned() {
                    if !self.member_visible(f.vis, cid) {
                        self.err(span, DiagKind::Private { name: f.name.clone() });
                    }
                    return Some(self.hx(f.ty.clone(), span, ExprKind::StaticField { class: cid, index: f.index }));
                }
            }
        }
        if path.len() >= 2 {
            let file = self.current_file();
            for i in 1..path.len() {
                let pkg = path[..i].join(".");
                if self.packages.contains_key(&pkg) && path.len() == i + 1 {
                    if let Some(Symbol::Const(id)) = self.packages.get(&pkg).and_then(|p| p.symbols.get(&path[i])).cloned() {
                        if let Some(cv) = self.force_const(id) {
                            let ty = self.consts[id as usize].ty.clone();
                            return Some(self.const_expr(cv, ty, span));
                        }
                    }
                }
            }
            let _ = file;
        }
        None
    }

    fn field_access(&mut self, recv: Expr, name: &str, span: Span) -> Expr {
        match &recv.ty {
            Ty::Class(cid) => {
                let cid = *cid;
                if let Some(f) = self.layout_field(cid, name) {
                    if !self.field_visible(&f) {
                        self.err(span, DiagKind::Private { name: name.into() });
                    }
                    let ty = f.ty.clone();
                    let index = f.index;
                    self.hx(ty, span, ExprKind::Field { recv: Box::new(recv), index })
                } else {
                    self.err(span, DiagKind::NoField { name: name.into() });
                    self.poison(span)
                }
            }
            Ty::Error => self.poison(span),
            _ => {
                self.err(span, DiagKind::NoField { name: name.into() });
                self.poison(span)
            }
        }
    }

    fn call_super_value(&mut self, _super_name: &str, name: &str, span: Span) -> Expr {
        // super.field
        let class = match self.env.as_ref().and_then(|e| e.class) {
            Some(c) => c,
            None => {
                self.err(span, DiagKind::BadSuper);
                return self.poison(span);
            }
        };
        let Some(parent) = self.classes[class as usize].super_id else {
            self.err(span, DiagKind::BadSuper);
            return self.poison(span);
        };
        self.field_access(self.self_expr(span).with_ty(Ty::Class(parent)), name, span)
    }

    fn lower_call(&mut self, callee: &ast::Expr, args: &[ast::Expr], span: Span) -> Expr {
        let args: Vec<Expr> = args.iter().map(|a| self.lower_expr(a)).collect();
        match &callee.kind {
            ast::ExprKind::SuperField { name } => self.call_super_method(name, args, span),
            ast::ExprKind::Name(name) => self.call_simple(name, args, span),
            ast::ExprKind::Field { recv, name } => {
                if let Some(path) = flatten(callee) {
                    self.call_path(&path, args, span)
                } else {
                    let recv = self.lower_expr(recv);
                    self.call_on_expr(recv, name, args, span)
                }
            }
            _ => {
                self.err(span, DiagKind::NotCallable { name: "expression".into() });
                self.poison(span)
            }
        }
    }

    fn call_simple(&mut self, name: &str, args: Vec<Expr>, span: Span) -> Expr {
        if self.lookup_local(name).is_some() {
            self.err(span, DiagKind::NotCallable { name: name.into() });
            return self.poison(span);
        }
        if let Some(class) = self.env.as_ref().and_then(|e| e.class) {
            if let Some(mid) = self.find_any_method(class, name) {
                return self.emit_named_method(mid, args, span);
            }
        }
        match self.lookup_name(self.current_file(), name) {
            Some(Symbol::Func(id)) => self.emit_func(id, args, span),
            Some(Symbol::Builtin(b)) => self.emit_builtin(b, args, span),
            _ => {
                self.err(span, DiagKind::UndefinedName { name: name.into() });
                self.poison(span)
            }
        }
    }

    fn call_path(&mut self, path: &[String], args: Vec<Expr>, span: Span) -> Expr {
        if path.len() >= 2 {
            for i in 1..path.len() {
                let pkg = path[..i].join(".");
                if !self.packages.contains_key(&pkg) {
                    continue;
                }
                let rest = &path[i..];
                if rest.len() == 1 {
                    if let Some(sym) = self.packages.get(&pkg).and_then(|p| p.symbols.get(&rest[0])).cloned() {
                        return match sym {
                            Symbol::Func(id) => self.emit_func(id, args, span),
                            Symbol::Builtin(b) => self.emit_builtin(b, args, span),
                            _ => {
                                self.err(span, DiagKind::NotCallable { name: rest[0].clone() });
                                self.poison(span)
                            }
                        };
                    }
                }
                if rest.len() == 2 {
                    if let Some(Symbol::Class(cid)) = self.packages.get(&pkg).and_then(|p| p.symbols.get(&rest[0])).cloned() {
                        if let Some(mid) = self.find_any_method(cid, &rest[1]) {
                            if self.funcs[mid as usize].is_static {
                                return self.emit_static_method(mid, args, span);
                            }
                        }
                    }
                }
            }
        }
        if path.len() == 2 {
            if let Some(Symbol::Class(cid)) = self.lookup_name(self.current_file(), &path[0]) {
                if let Some(mid) = self.find_any_method(cid, &path[1]) {
                    if self.funcs[mid as usize].is_static {
                        return self.emit_static_method(mid, args, span);
                    }
                    self.err(span, DiagKind::BadModifier { detail: format!("'{}' is an instance method", path[1]) });
                    return self.poison(span);
                }
                self.err(span, DiagKind::NoMethod { name: path[1].clone() });
                return self.poison(span);
            }
        }
        if let Some(b) = self.lookup_local(&path[0]) {
            let mut recv = self.hx(b.ty.clone(), span, ExprKind::Local(b.slot));
            for field in &path[1..path.len() - 1] {
                recv = self.field_access(recv, field, span);
            }
            let method = path.last().unwrap();
            return self.call_on_expr(recv, method, args, span);
        }
        if path.len() >= 2 {
            if let Some(Symbol::Class(_)) = self.lookup_name(self.current_file(), &path[0]) {
                self.err(span, DiagKind::NoMethod { name: path[1].clone() });
                return self.poison(span);
            }
        }
        // obj.method where obj is not a local name but the path was flattened from fields.
        // Rebuild: lower path[0] as name already failed. Treat path as field chain ending in a call
        // only when path[0] resolved. Otherwise undefined.
        self.err(span, DiagKind::UndefinedName { name: path.join(".") });
        self.poison(span)
    }

    fn call_super_method(&mut self, name: &str, args: Vec<Expr>, span: Span) -> Expr {
        let class = match self.env.as_ref().and_then(|e| e.class) {
            Some(c) if !self.env.as_ref().unwrap().is_static => c,
            _ => {
                self.err(span, DiagKind::BadSuper);
                return self.poison(span);
            }
        };
        let Some(parent) = self.classes[class as usize].super_id else {
            self.err(span, DiagKind::BadSuper);
            return self.poison(span);
        };
        let Some(mid) = self.find_instance_method(parent, name) else {
            self.err(span, DiagKind::NoMethod { name: name.into() });
            return self.poison(span);
        };
        let recv = self.self_expr(span);
        self.direct_method(mid, recv, args, span)
    }

    fn emit_named_method(&mut self, mid: u32, args: Vec<Expr>, span: Span) -> Expr {
        if self.funcs[mid as usize].is_static {
            self.emit_static_method(mid, args, span)
        } else if self.env.as_ref().is_some_and(|e| e.is_static) {
            self.err(span, DiagKind::StaticSelf);
            self.poison(span)
        } else {
            let recv = self.self_expr(span);
            self.virtual_method(mid, recv, args, span)
        }
    }

    fn emit_static_method(&mut self, mid: u32, args: Vec<Expr>, span: Span) -> Expr {
        if !self.func_visible(mid) {
            let name = self.funcs[mid as usize].simple.clone();
            self.err(span, DiagKind::Private { name });
        }
        let params: Vec<Ty> = self.funcs[mid as usize].params.iter().map(|(_, t)| t.clone()).collect();
        let name = self.funcs[mid as usize].simple.clone();
        self.check_args(&name, &params, &args, span);
        self.ensure_func(mid);
        let ret = self.funcs[mid as usize].ret.clone();
        let is_async = self.funcs[mid as usize].is_async;
        let ty = if is_async { Ty::Task(Box::new(ret)) } else { ret };
        self.hx(ty, span, ExprKind::Call { kind: CallKind::Direct { func: mid, is_async }, args })
    }

    fn emit_func(&mut self, id: u32, args: Vec<Expr>, span: Span) -> Expr {
        if !self.func_visible(id) {
            let name = self.funcs[id as usize].simple.clone();
            self.err(span, DiagKind::Private { name });
        }
        let params: Vec<Ty> = self.funcs[id as usize].params.iter().map(|(_, t)| t.clone()).collect();
        let name = self.funcs[id as usize].simple.clone();
        self.check_args(&name, &params, &args, span);
        self.ensure_func(id);
        let ret = self.funcs[id as usize].ret.clone();
        let is_async = self.funcs[id as usize].is_async;
        let ty = if is_async { Ty::Task(Box::new(ret)) } else { ret };
        self.hx(ty, span, ExprKind::Call { kind: CallKind::Direct { func: id, is_async }, args })
    }

    fn direct_method(&mut self, mid: u32, recv: Expr, args: Vec<Expr>, span: Span) -> Expr {
        let params: Vec<Ty> = self.funcs[mid as usize].params.iter().map(|(_, t)| t.clone()).collect();
        let name = self.funcs[mid as usize].simple.clone();
        self.check_args(&name, &params, &args, span);
        self.ensure_func(mid);
        let ret = self.funcs[mid as usize].ret.clone();
        let is_async = self.funcs[mid as usize].is_async;
        let ty = if is_async { Ty::Task(Box::new(ret)) } else { ret };
        let mut all = vec![recv];
        all.extend(args);
        self.hx(ty, span, ExprKind::Call { kind: CallKind::Direct { func: mid, is_async }, args: all })
    }

    fn virtual_method(&mut self, mid: u32, recv: Expr, args: Vec<Expr>, span: Span) -> Expr {
        if !self.func_visible(mid) {
            let name = self.funcs[mid as usize].simple.clone();
            self.err(span, DiagKind::Private { name });
        }
        let params: Vec<Ty> = self.funcs[mid as usize].params.iter().map(|(_, t)| t.clone()).collect();
        let name = self.funcs[mid as usize].simple.clone();
        self.check_args(&name, &params, &args, span);
        self.ensure_func(mid);
        let ret = self.funcs[mid as usize].ret.clone();
        let is_async = self.funcs[mid as usize].is_async;
        let slot = self.funcs[mid as usize].slot.unwrap_or(0);
        let ty = if is_async { Ty::Task(Box::new(ret)) } else { ret };
        let mut all = vec![recv];
        all.extend(args);
        let argc = u8::try_from(all.len()).unwrap_or(255);
        self.hx(ty, span, ExprKind::Call {
            kind: CallKind::Virtual { slot, argc, is_async },
            args: all,
        })
    }

    fn call_on_expr(&mut self, recv: Expr, name: &str, args: Vec<Expr>, span: Span) -> Expr {
        match &recv.ty {
            Ty::Class(cid) => {
                let cid = *cid;
                if let Some(mid) = self.find_instance_method(cid, name) {
                    self.virtual_method(mid, recv, args, span)
                } else {
                    self.err(span, DiagKind::NoMethod { name: name.into() });
                    self.poison(span)
                }
            }
            Ty::Iface(iid) => {
                let iid = *iid;
                if let Some((slot, method)) = self.ifaces[iid as usize]
                    .methods
                    .iter()
                    .enumerate()
                    .find(|(_, m)| m.name == name)
                    .map(|(i, m)| (i as u16, m.clone()))
                {
                    self.check_args(&method.name, &method.params, &args, span);
                    let ty = if method.is_async { Ty::Task(Box::new(method.ret.clone())) } else { method.ret.clone() };
                    let mut all = vec![recv];
                    all.extend(args);
                    let argc = u8::try_from(all.len()).unwrap_or(255);
                    self.hx(ty, span, ExprKind::Call {
                        kind: CallKind::Interface { iface: iid, slot, argc, is_async: method.is_async },
                        args: all,
                    })
                } else {
                    self.err(span, DiagKind::NoMethod { name: name.into() });
                    self.poison(span)
                }
            }
            Ty::Error => self.poison(span),
            _ => {
                self.err(span, DiagKind::NoMethod { name: name.into() });
                self.poison(span)
            }
        }
    }

    fn lower_new(&mut self, ty: &ast::TypeRef, args: &[ast::Expr], span: Span) -> Expr {
        let bare = ast::TypeRef { path: ty.path.clone(), dims: 0, span: ty.span };
        if ty.dims != 0 {
            self.err(span, DiagKind::CannotNew { name: ty.path.join(".") });
            return self.poison(span);
        }
        let args: Vec<Expr> = args.iter().map(|a| self.lower_expr(a)).collect();
        match self.resolve_type(self.current_file(), &bare) {
            Ty::Class(cid) => {
                let Some(ctor) = self.classes[cid as usize].ctor else {
                    self.err(span, DiagKind::CannotNew { name: self.classes[cid as usize].name.clone() });
                    return self.poison(span);
                };
                if !self.func_visible(ctor) || !self.class_visible(cid) {
                    self.err(span, DiagKind::Private { name: self.classes[cid as usize].name.clone() });
                }
                let params: Vec<Ty> = self.funcs[ctor as usize].params.iter().map(|(_, t)| t.clone()).collect();
                let name = self.classes[cid as usize].name.clone();
                self.check_args(&name, &params, &args, span);
                self.ensure_func(ctor);
                self.hx(Ty::Class(cid), span, ExprKind::New { class: cid, ctor, args })
            }
            Ty::Error => self.poison(span),
            other => {
                self.err(span, DiagKind::CannotNew { name: self.fmt_ty(&other) });
                self.poison(span)
            }
        }
    }

    fn emit_builtin(&mut self, b: Builtin, args: Vec<Expr>, span: Span) -> Expr {
        if let Builtin::Host(id) = b {
            return self.emit_host(id, args, span);
        }
        let name = builtin_name(b);
        if args.len() != 1 {
            self.err(span, DiagKind::BadArity { name: name.into(), expected: 1, found: args.len() });
            return self.poison(span);
        }
        let arg_ty = args[0].ty.clone();
        let ty = match b {
            Builtin::Print | Builtin::PrintLn => Ty::Void,
            Builtin::ToString => Ty::String,
            Builtin::Len => {
                if !matches!(arg_ty, Ty::Array(_) | Ty::String | Ty::Error) {
                    self.err(span, DiagKind::TypeMismatch { expected: "array or string".into(), found: self.fmt_ty(&arg_ty) });
                }
                Ty::Int
            }
            Builtin::ToInt => {
                if !arg_ty.is_numeric() && !matches!(arg_ty, Ty::Bool | Ty::String | Ty::Error) {
                    self.err(span, DiagKind::TypeMismatch { expected: "number, bool, or string".into(), found: self.fmt_ty(&arg_ty) });
                }
                Ty::Int
            }
            Builtin::ToFloat => {
                if !arg_ty.is_numeric() && !matches!(arg_ty, Ty::String | Ty::Bool | Ty::Error) {
                    self.err(span, DiagKind::TypeMismatch { expected: "number or string".into(), found: self.fmt_ty(&arg_ty) });
                }
                Ty::Float
            }
            Builtin::ToDouble => {
                if !arg_ty.is_numeric() && !matches!(arg_ty, Ty::String | Ty::Bool | Ty::Error) {
                    self.err(span, DiagKind::TypeMismatch { expected: "number or string".into(), found: self.fmt_ty(&arg_ty) });
                }
                Ty::Double
            }
            Builtin::Host(_) => Ty::Error,
        };
        self.hx(ty, span, ExprKind::Call { kind: CallKind::Builtin(b), args })
    }

    fn emit_host(&mut self, id: u16, args: Vec<Expr>, span: Span) -> Expr {
        let Some(sig) = terra_bytecode::host(id) else {
            self.err(span, DiagKind::Internal { detail: format!("bad host {id}") });
            return self.poison(span);
        };
        if args.len() != sig.params.len() {
            self.err(
                span,
                DiagKind::BadArity {
                    name: sig.name.into(),
                    expected: sig.params.len(),
                    found: args.len(),
                },
            );
            return self.poison(span);
        }
        for (expect, arg) in sig.params.iter().zip(&args) {
            if !host_accepts(*expect, &arg.ty) {
                self.err(
                    arg.span,
                    DiagKind::TypeMismatch {
                        expected: terra_bytecode::host_ty_name(*expect).into(),
                        found: self.fmt_ty(&arg.ty),
                    },
                );
            }
        }
        let ty = match sig.ret {
            terra_bytecode::HostTy::Void => Ty::Void,
            terra_bytecode::HostTy::Int => Ty::Int,
            terra_bytecode::HostTy::Float => Ty::Float,
            terra_bytecode::HostTy::Double => Ty::Double,
            terra_bytecode::HostTy::Bool => Ty::Bool,
            terra_bytecode::HostTy::String => Ty::String,
            terra_bytecode::HostTy::Any => Ty::Any,
        };
        self.hx(ty, span, ExprKind::Call { kind: CallKind::Builtin(Builtin::Host(id)), args })
    }

    fn lower_place(&mut self, expr: &ast::Expr) -> Place {
        match &expr.kind {
            ast::ExprKind::Name(name) => {
                if let Some(b) = self.lookup_local(name) {
                    if !b.mutable {
                        self.err(expr.span, DiagKind::Immutable { name: name.clone() });
                    }
                    return Place::Local(b.slot);
                }
                if let Some(class) = self.env.as_ref().and_then(|e| if e.is_static { None } else { e.class }) {
                    if let Some(f) = self.layout_field(class, name) {
                        if !f.mutable && !self.in_ctor() {
                            self.err(expr.span, DiagKind::Immutable { name: name.clone() });
                        }
                        if !self.field_visible(&f) {
                            self.err(expr.span, DiagKind::Private { name: name.clone() });
                        }
                        return Place::Field { recv: self.self_expr(expr.span), index: f.index };
                    }
                }
                self.err(expr.span, DiagKind::UndefinedName { name: name.clone() });
                Place::Local(0)
            }
            ast::ExprKind::Field { recv, name } => {
                if let Some(mut path) = flatten(recv) {
                    path.push(name.clone());
                    if path.len() == 2 {
                        if let Some(Symbol::Class(cid)) = self.lookup_name(self.current_file(), &path[0]) {
                            if let Some(f) = self.classes[cid as usize].statics.iter().find(|f| f.name == path[1]).cloned() {
                                if !f.mutable {
                                    self.err(expr.span, DiagKind::Immutable { name: f.name.clone() });
                                }
                                return Place::Static { class: cid, index: f.index };
                            }
                        }
                    }
                }
                let recv_e = self.lower_expr(recv);
                if let Ty::Class(cid) = recv_e.ty {
                    if let Some(f) = self.layout_field(cid, name) {
                        if !f.mutable && !self.in_ctor() {
                            self.err(expr.span, DiagKind::Immutable { name: name.clone() });
                        }
                        if !self.field_visible(&f) {
                            self.err(expr.span, DiagKind::Private { name: name.clone() });
                        }
                        return Place::Field { recv: recv_e, index: f.index };
                    }
                }
                self.err(expr.span, DiagKind::NoField { name: name.clone() });
                Place::Local(0)
            }
            ast::ExprKind::Index { recv, index } => {
                let recv = self.lower_expr(recv);
                let index = self.lower_expr(index);
                Place::Index { recv, index }
            }
            _ => {
                self.err(expr.span, DiagKind::BadAssign);
                Place::Local(0)
            }
        }
    }

    fn assign_ok(&mut self, place: &Place, value: &Expr) {
        let expect = match place {
            Place::Local(slot) => self.env.as_ref().and_then(|e| e.locals.get(*slot as usize)).map(|l| l.ty.clone()),
            Place::Field { index, recv } => {
                if let Ty::Class(cid) = recv.ty {
                    self.classes[cid as usize].layout.iter().find(|f| f.index == *index).map(|f| f.ty.clone())
                } else {
                    None
                }
            }
            Place::Static { class, index } => self.classes[*class as usize]
                .statics
                .iter()
                .find(|f| f.index == *index)
                .map(|f| f.ty.clone()),
            Place::Index { recv, .. } => match &recv.ty {
                Ty::Array(t) => Some((**t).clone()),
                _ => None,
            },
        };
        if let Some(expect) = expect {
            if !self.assignable(&value.ty, &expect) {
                self.err(
                    value.span,
                    DiagKind::TypeMismatch {
                        expected: self.fmt_ty(&expect),
                        found: self.fmt_ty(&value.ty),
                    },
                );
            }
        }
    }

    fn check_args(&mut self, name: &str, params: &[Ty], args: &[Expr], span: Span) {
        if params.len() != args.len() {
            self.err(span, DiagKind::BadArity { name: name.into(), expected: params.len(), found: args.len() });
            return;
        }
        for (p, a) in params.iter().zip(args) {
            if !self.assignable(&a.ty, p) {
                self.err(a.span, DiagKind::TypeMismatch { expected: self.fmt_ty(p), found: self.fmt_ty(&a.ty) });
            }
        }
    }

    fn check_returns(&mut self, id: u32, body: &[Stmt]) {
        if self.funcs[id as usize].is_ctor {
            return;
        }
        let ret = self.funcs[id as usize].ret.clone();
        if matches!(ret, Ty::Void | Ty::Error) {
            return;
        }
        if !block_diverges(body) {
            let name = self.funcs[id as usize].simple.clone();
            let span = self.funcs[id as usize].span;
            self.err(span, DiagKind::MissingReturn { name });
        }
    }

    fn check_override_results(&mut self) {
        let ids: Vec<u32> = (0..self.funcs.len() as u32).collect();
        for id in ids {
            if !self.funcs[id as usize].is_override {
                continue;
            }
            let Some(class) = self.funcs[id as usize].class else { continue };
            let Some(parent) = self.classes[class as usize].super_id else { continue };
            let name = self.funcs[id as usize].simple.clone();
            let Some(pid) = self.find_instance_method(parent, &name) else { continue };
            let cr = self.funcs[id as usize].ret.clone();
            let pr = self.funcs[pid as usize].ret.clone();
            if cr != pr && !cr.is_error() && !pr.is_error() {
                let span = self.funcs[id as usize].span;
                self.err(span, DiagKind::TypeMismatch { expected: self.fmt_ty(&pr), found: self.fmt_ty(&cr) });
            }
        }
    }

    fn pick_entry(&mut self) {
        let mut found: Vec<Entry> = Vec::new();
        let mut hint = String::new();
        for (id, f) in self.funcs.iter().enumerate() {
            if f.simple != "main" || f.is_ctor {
                continue;
            }
            if f.class.is_none() && is_entry_params(&f.params) {
                found.push(Entry::Free(id as u32));
            } else if f.class.is_none() {
                hint = format!("; found main with a different signature");
            }
        }
        for (cid, class) in self.classes.iter().enumerate() {
            if class.is_prelude {
                continue;
            }
            for mid in &class.methods {
                let f = &self.funcs[*mid as usize];
                if f.simple == "main" && !f.is_ctor && is_entry_params(&f.params) {
                    found.push(Entry::Method { class: cid as u32, func: *mid });
                }
            }
        }
        if found.is_empty() {
            self.diags.push(Diagnostic::error_at_none(DiagKind::MissingMain { hint }));
            return;
        }
        if found.len() > 1 {
            self.diags.push(Diagnostic::error_at_none(DiagKind::MultipleMain));
            return;
        }
        match &found[0] {
            Entry::Free(id) => {
                let ret = self.funcs[*id as usize].ret.clone();
                if !matches!(ret, Ty::Void | Ty::Int | Ty::Error) {
                    self.err(self.funcs[*id as usize].span, DiagKind::BadEntry { detail: "main must return void or int".into() });
                }
                self.entry = Some(Entry::Free(*id));
            }
            Entry::Method { class, func } => {
                let ret = self.funcs[*func as usize].ret.clone();
                if !matches!(ret, Ty::Void | Ty::Int | Ty::Error) {
                    self.err(self.funcs[*func as usize].span, DiagKind::BadEntry { detail: "main must return void or int".into() });
                }
                if !self.funcs[*func as usize].is_static {
                    let ctor = self.classes[*class as usize].ctor;
                    if let Some(ctor) = ctor {
                        if !self.funcs[ctor as usize].params.is_empty() {
                            let name = self.classes[*class as usize].name.clone();
                            self.err(self.funcs[*func as usize].span, DiagKind::EntryCtor { name });
                        }
                    }
                }
                self.entry = Some(Entry::Method { class: *class, func: *func });
            }
        }
    }

    fn finish(self) -> Program {
        let (entry_func, entry_class, entry_ctor, entry_returns_int) = match self.entry {
            Some(Entry::Free(id)) => (id, None, None, matches!(self.funcs[id as usize].ret, Ty::Int)),
            Some(Entry::Method { class, func }) => {
                let returns = matches!(self.funcs[func as usize].ret, Ty::Int);
                if self.funcs[func as usize].is_static {
                    (func, None, None, returns)
                } else {
                    (func, Some(class), self.classes[class as usize].ctor, returns)
                }
            }
            None => (0, None, None, false),
        };
        Program {
            functions: self.funcs.iter().map(|f| {
                let name = if let Some(cid) = f.class {
                    format!("{}.{}", self.classes[cid as usize].name, f.simple)
                } else {
                    f.simple.clone()
                };
                let arity = f.params.len() as u16 + if f.class.is_some() && !f.is_static { 1 } else { 0 };
                Function {
                    name,
                    arity,
                    locals: f.locals.clone(),
                    ret: f.ret.clone(),
                    is_async: f.is_async,
                    is_ctor: f.is_ctor,
                    body: f.hir.clone(),
                    span: f.span,
                }
            }).collect(),
            classes: self.classes.iter().map(|c| Class {
                name: c.name.clone(),
                package: c.package.clone(),
                super_id: c.super_id,
                fields: c.layout.iter().map(|f| Field { name: f.name.clone(), ty: f.ty.clone() }).collect(),
                vtable: c.vtable.clone(),
                itables: c.itables.clone(),
                ctor: c.ctor,
                is_exception: c.is_exception,
            }).collect(),
            interfaces: self.ifaces.iter().map(|i| Interface {
                name: i.name.clone(),
                package: i.package.clone(),
                methods: i.methods.iter().map(|m| m.name.clone()).collect(),
            }).collect(),
            enums: self.enums.iter().map(|e| Enum {
                name: e.name.clone(),
                package: e.package.clone(),
                variants: e.variants.clone(),
            }).collect(),
            static_inits: self.static_inits,
            entry_func,
            entry_class,
            entry_ctor,
            entry_returns_int,
        }
    }

    fn alloc_local(&mut self, name: &str, ty: Ty, mutable: bool, const_value: Option<ConstValue>) -> u16 {
        let shadowed = self
            .env
            .as_ref()
            .unwrap()
            .scopes
            .last()
            .unwrap()
            .contains_key(name);
        if shadowed {
            let span = self.funcs[self.env.as_ref().unwrap().id as usize].span;
            self.err(span, DiagKind::Shadow { name: name.into() });
        }
        let env = self.env.as_mut().unwrap();
        let slot = env.locals.len() as u16;
        env.locals.push(Local { name: name.into(), ty: ty.clone() });
        env.scopes.last_mut().unwrap().insert(name.into(), Binding { slot, ty, mutable, const_value });
        slot
    }

    fn lookup_local(&self, name: &str) -> Option<BindingRef> {
        let env = self.env.as_ref()?;
        for scope in env.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(BindingRef { slot: b.slot, ty: b.ty.clone(), mutable: b.mutable, const_value: b.const_value.clone() });
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.env.as_mut().unwrap().scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.env.as_mut().unwrap().scopes.pop();
    }

    fn bump_loop(&mut self, d: i32) {
        let env = self.env.as_mut().unwrap();
        if d < 0 {
            env.loop_depth = env.loop_depth.saturating_sub((-d) as u32);
        } else {
            env.loop_depth += d as u32;
        }
    }

    fn need_loop(&mut self, what: &str, span: Span) {
        if self.env.as_ref().map(|e| e.loop_depth).unwrap_or(0) == 0 {
            self.err(span, DiagKind::NotInLoop { what: what.into() });
        }
    }

    fn current_file(&self) -> u32 {
        self.env.as_ref().map(|e| self.funcs[e.id as usize].file).unwrap_or(0)
    }

    fn layout_field(&self, class: u32, name: &str) -> Option<LayField> {
        self.classes[class as usize].layout.iter().find(|f| f.name == name).cloned()
    }

    fn field_visible(&self, field: &LayField) -> bool {
        self.member_visible(field.vis, field.owner)
    }

    fn member_visible(&self, vis: Vis, owner: u32) -> bool {
        let from_class = self.env.as_ref().and_then(|e| e.class);
        let from_pkg = self.files.get(self.current_file() as usize).map(|f| f.package.clone()).unwrap_or_default();
        let owner_pkg = self.classes[owner as usize].package.clone();
        match vis {
            Vis::Pub => true,
            Vis::Priv => from_class == Some(owner),
            Vis::Default => from_pkg == owner_pkg || from_class == Some(owner),
        }
    }

    fn func_visible(&self, id: u32) -> bool {
        let f = &self.funcs[id as usize];
        if f.file == u32::MAX || f.vis == Vis::Pub {
            return true;
        }
        if let Some(class) = f.class {
            return self.member_visible(f.vis, class);
        }
        self.symbol_visible(&Symbol::Func(id), self.current_file())
    }

    fn class_visible(&self, id: u32) -> bool {
        self.symbol_visible(&Symbol::Class(id), self.current_file())
    }

    fn in_ctor(&self) -> bool {
        self.env.as_ref().is_some_and(|e| e.is_ctor)
    }

    fn self_expr(&self, span: Span) -> Expr {
        let class = self.env.as_ref().and_then(|e| e.class).unwrap_or(0);
        Expr { ty: Ty::Class(class), span, kind: ExprKind::Local(0) }
    }

    fn hx(&self, ty: Ty, span: Span, kind: ExprKind) -> Expr {
        Expr { ty, span, kind }
    }

    fn hs(&self, span: Span, kind: StmtKind) -> Stmt {
        Stmt { span, kind }
    }

    fn poison(&self, span: Span) -> Expr {
        Expr { ty: Ty::Error, span, kind: ExprKind::Int(0) }
    }

    fn const_expr(&self, value: ConstValue, ty: Ty, span: Span) -> Expr {
        let kind = match value {
            ConstValue::Int(n) => ExprKind::Int(n),
            ConstValue::Float(n) => ExprKind::Float(n),
            ConstValue::Double(n) => ExprKind::Double(n),
            ConstValue::Bool(b) => ExprKind::Bool(b),
            ConstValue::Str(s) => ExprKind::Str(s),
            ConstValue::Null => ExprKind::Null,
            ConstValue::Enum { id, variant } => ExprKind::Enum { enum_id: id, variant },
        };
        Expr { ty, span, kind }
    }

    fn default_expr(&self, ty: &Ty, span: Span) -> Expr {
        self.const_expr(default_const(ty), ty.clone(), span)
    }

    fn fold(&self, expr: &Expr) -> Option<ConstValue> {
        match &expr.kind {
            ExprKind::Int(n) => Some(ConstValue::Int(*n)),
            ExprKind::Float(n) => Some(ConstValue::Float(*n)),
            ExprKind::Double(n) => Some(ConstValue::Double(*n)),
            ExprKind::Bool(b) => Some(ConstValue::Bool(*b)),
            ExprKind::Str(s) => Some(ConstValue::Str(s.clone())),
            ExprKind::Null => Some(ConstValue::Null),
            ExprKind::Enum { enum_id, variant } => Some(ConstValue::Enum { id: *enum_id, variant: *variant }),
            ExprKind::Unary { op: UnaryOp::Neg, expr } => match self.fold(expr)? {
                ConstValue::Int(n) => Some(ConstValue::Int(n.wrapping_neg())),
                ConstValue::Float(n) => Some(ConstValue::Float(-n)),
                ConstValue::Double(n) => Some(ConstValue::Double(-n)),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_bool(&mut self, ty: &Ty, span: Span) {
        if !ty.is_error() && !matches!(ty, Ty::Bool) {
            self.err(span, DiagKind::NeedBool);
        }
    }

    fn expect_exception(&mut self, expr: &Expr) {
        match &expr.ty {
            Ty::Class(id) if self.classes[*id as usize].is_exception => {}
            Ty::Error => {}
            _ => self.err(expr.span, DiagKind::NotThrowable { name: self.fmt_ty(&expr.ty) }),
        }
    }

    fn assignable(&self, from: &Ty, to: &Ty) -> bool {
        if from.is_error() || to.is_error() || from == to || matches!(from, Ty::Any) || matches!(to, Ty::Any) {
            return true;
        }
        if matches!(from, Ty::Null) && to.is_ref() {
            return true;
        }
        match (from, to) {
            (Ty::Class(a), Ty::Class(b)) => self.is_subclass(*a, *b),
            (Ty::Class(a), Ty::Iface(i)) => self.implements_iface(*a, *i),
            (Ty::Array(a), Ty::Array(b)) => a == b,
            _ => false,
        }
    }

    fn comparable(&self, a: &Ty, b: &Ty) -> bool {
        a == b || self.assignable(a, b) || self.assignable(b, a)
    }

    fn is_subclass(&self, mut a: u32, b: u32) -> bool {
        loop {
            if a == b {
                return true;
            }
            match self.classes[a as usize].super_id {
                Some(p) => a = p,
                None => return false,
            }
        }
    }

    fn implements_iface(&self, class: u32, iface: u32) -> bool {
        self.all_ifaces(class).contains(&iface)
    }

    fn fmt_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Error => "error".into(),
            Ty::Void => "void".into(),
            Ty::Int => "int".into(),
            Ty::Float => "float".into(),
            Ty::Double => "double".into(),
            Ty::Bool => "bool".into(),
            Ty::String => "string".into(),
            Ty::Null => "null".into(),
            Ty::Range => "range".into(),
            Ty::Array(t) => format!("{}[]", self.fmt_ty(t)),
            Ty::Class(id) => self.classes.get(*id as usize).map(|c| c.name.clone()).unwrap_or_else(|| "class".into()),
            Ty::Iface(id) => self.ifaces.get(*id as usize).map(|c| c.name.clone()).unwrap_or_else(|| "interface".into()),
            Ty::Enum(id) => self.enums.get(*id as usize).map(|c| c.name.clone()).unwrap_or_else(|| "enum".into()),
            Ty::Task(t) => format!("task<{}>", self.fmt_ty(t)),
            Ty::Any => "any".into(),
        }
    }

    fn fmt_sig(&self, tys: &[Ty]) -> String {
        tys.iter().map(|t| self.fmt_ty(t)).collect::<Vec<_>>().join(", ")
    }
}

include!("sema_helpers.rs");
