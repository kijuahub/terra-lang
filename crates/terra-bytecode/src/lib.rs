//! Serializable bytecode executed by `terra-runtime`.
//!
//! The compiler is the only producer. A native launcher (`terra-exec`) embeds
//! the encoded module after the executable and reads it back at startup.

mod host;

pub use host::*;

use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 4] = b"TBC1";
pub const TRAILER_MAGIC: &[u8; 8] = b"TERRAOBJ";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Module {
    pub files: Vec<String>,
    pub constants: Vec<Constant>,
    pub functions: Vec<Function>,
    pub classes: Vec<ClassInfo>,
    pub interfaces: Vec<InterfaceInfo>,
    pub enums: Vec<EnumInfo>,
    pub static_inits: Vec<StaticInit>,
    pub entry: u32,
    pub global_count: u16,
    /// `en` or `ru`. The launcher uses this for runtime messages.
    pub lang: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constant {
    Int(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Null,
    Enum { id: u32, variant: u16 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub arity: u16,
    pub locals: u16,
    pub local_names: Vec<String>,
    pub code: Vec<Op>,
    pub locs: Vec<SourceLoc>,
    /// True when a call of this function builds a task instead of running it.
    pub is_async: bool,
    pub returns_void: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLoc {
    pub file: u32,
    pub line: u32,
    pub col: u32,
}

impl SourceLoc {
    pub fn unknown() -> Self {
        Self {
            file: 0,
            line: 0,
            col: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassInfo {
    pub name: String,
    pub package: String,
    pub super_id: Option<u32>,
    pub field_names: Vec<String>,
    /// Instance methods in slot order. Each entry is a function index.
    pub vtable: Vec<u32>,
    pub itables: Vec<ITable>,
    pub ctor: Option<u32>,
    pub is_exception: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ITable {
    pub iface: u32,
    pub methods: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub package: String,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: String,
    pub package: String,
    pub variants: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticInit {
    pub class: u32,
    pub field: u16,
    pub value: Constant,
}

/// Stack machine. Jumps are absolute instruction indexes inside the function.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Op {
    Const(u32),
    Load(u16),
    Store(u16),
    LoadField(u16),
    StoreField(u16),
    LoadStatic(u32, u16),
    StoreStatic(u32, u16),
    LoadGlobal(u16),
    Pop,
    Dup,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Not,
    Jump(u32),
    JumpIfFalse(u32),
    Call(u32),
    CallVirtual(u16, u8),
    CallInterface(u32, u16, u8),
    MakeTask(u32),
    MakeTaskVirtual(u16, u8),
    MakeTaskInterface(u32, u16, u8),
    Return,
    New(u32),
    NewArray,
    ArrayLit(u16),
    GetIndex,
    SetIndex,
    Len,
    Throw,
    PushHandler(u32),
    PopHandler,
    IsType(u32),
    Delete,
    Await,
    Print,
    PrintLn,
    ToString,
    ToInt,
    ToFloat,
    ToDouble,
    Range,
    RangeStart,
    RangeEnd,
    /// Call a host function. The operand is an index into `HOSTS`.
    Host(u16),
}

pub fn encode(module: &Module) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    let body = serde_json::to_vec(module).expect("module serializes");
    bytes.extend(body);
    bytes
}

pub fn decode(bytes: &[u8]) -> Result<Module, String> {
    if bytes.len() < 4 || &bytes[..4] != MAGIC {
        return Err("not a Terra bytecode module".into());
    }
    serde_json::from_slice(&bytes[4..]).map_err(|e| e.to_string())
}

/// Read a module appended to an executable, if the Terra trailer is present.
pub fn read_embedded(exe: &[u8]) -> Option<Vec<u8>> {
    if exe.len() < 16 {
        return None;
    }
    let magic = &exe[exe.len() - 8..];
    if magic != TRAILER_MAGIC {
        return None;
    }
    let len = u64::from_le_bytes(exe[exe.len() - 16..exe.len() - 8].try_into().ok()?) as usize;
    let start = exe.len().checked_sub(16 + len)?;
    Some(exe[start..start + len].to_vec())
}

pub fn append_payload(mut exe: Vec<u8>, module_bytes: &[u8]) -> Vec<u8> {
    if let Some(stripped) = strip_trailer(&exe) {
        exe = stripped;
    }
    exe.extend_from_slice(module_bytes);
    exe.extend_from_slice(&(module_bytes.len() as u64).to_le_bytes());
    exe.extend_from_slice(TRAILER_MAGIC);
    exe
}

fn strip_trailer(exe: &[u8]) -> Option<Vec<u8>> {
    let payload = read_embedded(exe)?;
    let keep = exe.len() - 16 - payload.len();
    Some(exe[..keep].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailer_roundtrip() {
        let module = Module {
            files: vec!["main.terra".into()],
            constants: vec![Constant::Str("hi".into())],
            functions: vec![],
            classes: vec![],
            interfaces: vec![],
            enums: vec![],
            static_inits: vec![],
            entry: 0,
            global_count: 1,
            lang: "en".into(),
        };
        let encoded = encode(&module);
        let bundled = append_payload(b"EXE".to_vec(), &encoded);
        let extracted = read_embedded(&bundled).unwrap();
        let back = decode(&extracted).unwrap();
        assert_eq!(back.files[0], "main.terra");
        assert!(read_embedded(b"EXE").is_none());
    }
}
