//! A parser for Unreal Engine [T3D files](https://wiki.beyondunreal.com/Legacy:T3D_File).
//!
//! ```
//! let objects = t3d::parse_t3d("Begin Object\n    Foo=1\nEnd Object").unwrap();
//! assert_eq!(objects[0].type_, "Object");
//! ```

pub mod ast;
mod parser;

pub use ast::*;
pub use parser::{parse_t3d, Rule, T3dSyntaxError};
