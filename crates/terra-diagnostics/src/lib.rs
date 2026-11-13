//! Diagnostics shared by the lexer, type checker, runtime, and CLI.
//!
//! Messages exist in English and Russian. The active language comes from
//! `terra.toml` (`tool.lang`) or `--lang`.

mod locale;

pub use locale::{describe, rt, RtMsg};

/// Language of compiler, debugger, and runtime messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            _ => None,
        }
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang::En
    }
}

/// Byte span in a source file. Offsets are UTF-8 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub file: u32,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(file: u32, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }
}

/// One compilation unit kept for snippets and debugger locations.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        let source = source.into();
        let mut line_starts = vec![0];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            name: name.into(),
            source,
            line_starts,
        }
    }

    /// 1-based line and column.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let off = offset.min(self.source.len() as u32);
        let idx = match self.line_starts.binary_search(&off) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = off - self.line_starts[idx];
        (idx as u32 + 1, col + 1)
    }

    pub fn line_text(&self, line: u32) -> &str {
        if line == 0 {
            return "";
        }
        let idx = (line - 1) as usize;
        if idx >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[idx] as usize;
        let end = if idx + 1 < self.line_starts.len() {
            self.line_starts[idx + 1] as usize
        } else {
            self.source.len()
        };
        self.source[start..end].trim_end_matches(['\r', '\n'])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Structured diagnostic. Render with [`render`] so the text follows [`Lang`].
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Option<Span>,
    pub kind: DiagKind,
}

impl Diagnostic {
    pub fn error(span: Span, kind: DiagKind) -> Self {
        Self {
            severity: Severity::Error,
            span: Some(span),
            kind,
        }
    }

    pub fn error_at_none(kind: DiagKind) -> Self {
        Self {
            severity: Severity::Error,
            span: None,
            kind,
        }
    }

    pub fn warning(kind: DiagKind) -> Self {
        Self {
            severity: Severity::Warning,
            span: None,
            kind,
        }
    }

    pub fn code(&self) -> &'static str {
        describe(Lang::En, &self.kind).0
    }
}

/// Every compiler and project error the toolchain can report.
#[derive(Clone, Debug)]
pub enum DiagKind {
    ExpectedSemicolon,
    UnexpectedEof,
    UnexpectedToken { found: String, expected: String },
    UnterminatedString,
    UnterminatedComment,
    InvalidNumber { literal: String },
    InvalidEscape,
    ExpectedIdent,
    UndefinedName { name: String },
    Duplicate { name: String },
    TypeMismatch { expected: String, found: String },
    MissingMain { hint: String },
    MultipleMain,
    Immutable { name: String },
    NeedBool,
    BadArity { name: String, expected: usize, found: usize },
    Private { name: String },
    NeedOverride { name: String },
    NoOverride { name: String },
    FinalMethod { name: String },
    FinalClass { name: String },
    MissingImpl { class: String, method: String },
    NotThrowable { name: String },
    NeedConst,
    Cycle { name: String },
    NoField { name: String },
    NoMethod { name: String },
    BadIter,
    PackageMismatch { declared: String, expected: String },
    CannotNew { name: String },
    SuperFirst,
    NeedCtor { parent: String },
    BadModifier { detail: String },
    NotTask,
    NeedIntIndex,
    UnknownImport { path: String },
    CtorReturn,
    BadEntry { detail: String },
    NotInLoop { what: String },
    UnknownType { name: String },
    NotCallable { name: String },
    MissingReturn { name: String },
    BadAssign,
    InterfaceMember,
    MissingBody { name: String },
    ProjectMissing,
    ProjectExists,
    NotRegistered { name: String },
    AmbiguousProject { name: String },
    BadManifest { detail: String },
    Git { detail: String },
    Build { detail: String },
    CompilerMissing { version: String },
    TargetMissing { target: String },
    BadPackage { name: String },
    BadLang,
    TopLevelBinding,
    Unreachable,
    EmptyArray,
    BadSuper,
    StaticSelf,
    NestedType,
    MissingHandler,
    NonExhaustive,
    BadPattern,
    ReturnOutside,
    ConstType { detail: String },
    Shadow { name: String },
    UseAfterScope,
    AsyncCtor,
    EntryCtor { name: String },
    NoPackage,
    Internal { detail: String },
}

/// Render one diagnostic with a source snippet when a span is available.
pub fn render(diag: &Diagnostic, files: &[SourceFile], lang: Lang) -> String {
    let (code, message) = describe(lang, &diag.kind);
    let label = match (lang, diag.severity) {
        (Lang::Ru, Severity::Error) => "ошибка",
        (Lang::Ru, Severity::Warning) => "предупреждение",
        (Lang::En, Severity::Error) => "error",
        (Lang::En, Severity::Warning) => "warning",
    };
    let mut out = format!("{label}[{code}]: {message}");
    let Some(span) = diag.span else {
        return out;
    };
    let Some(file) = files.get(span.file as usize) else {
        return out;
    };
    let (line, col) = file.line_col(span.start);
    let (end_line, end_col) = file.line_col(span.end.max(span.start));
    out.push_str(&format!("\n --> {}:{}:{}", file.name, line, col));
    let text = file.line_text(line);
    let gutter = line.to_string().len();
    out.push_str(&format!("\n{:>width$} |", "", width = gutter));
    out.push_str(&format!("\n{line:>width$} | {text}", width = gutter));
    let start_col = (col as usize).saturating_sub(1);
    let mut len = if end_line == line {
        (end_col as usize).saturating_sub(col as usize).max(1)
    } else {
        1
    };
    if start_col + len > text.chars().count() {
        len = 1;
    }
    out.push_str(&format!(
        "\n{:>width$} | {spaces}{carets}",
        "",
        spaces = " ".repeat(start_col),
        carets = "^".repeat(len),
        width = gutter
    ));
    out
}

pub fn render_all(diags: &[Diagnostic], files: &[SourceFile], lang: Lang) -> Vec<String> {
    diags.iter().map(|d| render(d, files, lang)).collect()
}
