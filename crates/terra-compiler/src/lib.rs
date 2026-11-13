//! Compile Terra source to a bytecode [`Module`](terra_bytecode::Module).
//!
//! Pipeline: lex → parse → semantic analysis → stack bytecode. The standard
//! library is compiled into every program. A missing `-> Type` is not an error.

mod gen;

use terra_bytecode::Module;
use terra_diagnostics::{render_all, Lang, SourceFile};
use terra_sema::analyze;
use terra_syntax::{lex, parse};

const STDLIB: &[(&str, &str)] = &[
    (
        "stdlib/com/terra/lang/Math.terra",
        include_str!("../../../stdlib/com/terra/lang/Math.terra"),
    ),
    (
        "stdlib/com/terra/lang/Pair.terra",
        include_str!("../../../stdlib/com/terra/lang/Pair.terra"),
    ),
];

/// One compilation unit. `package` is required in project mode and optional
/// for a single file (the file may omit `package`).
pub struct Unit {
    pub name: String,
    pub source: String,
    pub package: Option<String>,
}

#[derive(Debug)]
pub struct Compiled {
    pub module: Module,
    pub files: Vec<SourceFile>,
}

#[derive(Debug)]
pub struct CompileError {
    pub messages: Vec<String>,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.messages.join("\n"))
    }
}

/// Compile user units. Stdlib sources are prepended.
pub fn compile(units: &[Unit], lang: Lang) -> Result<Compiled, CompileError> {
    let mut files = Vec::new();
    let mut asts = Vec::new();
    let mut expected = Vec::new();
    let mut diags = Vec::new();

    let mut push_unit = |name: &str, source: &str, package: Option<String>| {
        let id = files.len() as u32;
        let file = SourceFile::new(name, source);
        match lex(id, &file.source).and_then(|tokens| parse(id, &tokens)) {
            Ok(ast) => {
                files.push(file);
                asts.push(ast);
                expected.push(package);
            }
            Err(d) => {
                files.push(file);
                diags.push(d);
            }
        }
    };

    for (name, source) in STDLIB {
        push_unit(name, source, Some("com.terra.lang".into()));
    }
    for unit in units {
        push_unit(&unit.name, &unit.source, unit.package.clone());
    }

    if !diags.is_empty() {
        return Err(CompileError {
            messages: render_all(&diags, &files, lang),
        });
    }

    let program = match analyze(&asts, &expected) {
        Ok(p) => p,
        Err(diags) => {
            return Err(CompileError {
                messages: render_all(&diags, &files, lang),
            });
        }
    };

    let module = gen::emit(&program, &files, lang);
    Ok(Compiled { module, files })
}

/// Compile one file. `package` may be omitted in the source.
pub fn compile_file(name: &str, source: &str, lang: Lang) -> Result<Compiled, CompileError> {
    compile(
        &[Unit {
            name: name.into(),
            source: source.into(),
            package: None,
        }],
        lang,
    )
}
