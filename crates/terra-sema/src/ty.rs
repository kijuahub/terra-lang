#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Error,
    Void,
    Int,
    Float,
    Double,
    Bool,
    String,
    Null,
    Array(Box<Ty>),
    Class(u32),
    Iface(u32),
    Enum(u32),
    Task(Box<Ty>),
    Range,
    /// Dynamic value used by the standard library collections and JSON.
    Any,
}

impl Ty {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float | Ty::Double)
    }

    pub fn is_ref(&self) -> bool {
        matches!(self, Ty::Class(_) | Ty::Iface(_) | Ty::Array(_) | Ty::Null)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
}
