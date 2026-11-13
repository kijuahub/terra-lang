//! Name resolution, type checking, and lowering to a typed IR.
//!
//! Passes: collect symbols, resolve inheritance, check signatures, lower bodies.
//! A missing `-> Type` is void (or inferred when every `return` agrees). It is
//! not a syntax error.

mod analyze;
mod hir;
mod ty;

pub use analyze::analyze;
pub use hir::*;
pub use ty::Ty;
