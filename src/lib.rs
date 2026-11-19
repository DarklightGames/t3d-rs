//! A parser for Unreal Engine [T3D files](https://wiki.beyondunreal.com/Legacy:T3D_File)
//! and the T3D-style property text used by UnrealScript `defaultproperties` blocks.
//!
//! ```
//! let objects = t3d::parse_t3d("Begin Object\n    Foo=1\nEnd Object");
//! assert_eq!(objects[0].type_, "Object");
//! ```
//!
//! # Permissive and strict parsing
//!
//! Text is parsed line by line, the way Unreal Engine imports it. By default
//! ([`ParseMode::Permissive`]) problems are recovered from and reported as
//! [`Diagnostic`]s:
//!
//! - a line that cannot be parsed is discarded ([`DiagnosticKind::InvalidLine`]);
//! - a complete statement followed by junk is kept and the rest of the line is
//!   ignored ([`DiagnosticKind::TrailingInput`]);
//! - unbalanced `Begin`/`End` lines are reported and repaired.
//!
//! In [`ParseMode::Strict`], any diagnostic makes parsing fail with
//! [`T3dError::Strict`].
//!
//! ```
//! use t3d::{parse_properties, ParseMode, ParseOptions};
//!
//! let text = "A=1\nB=2/\n???\n";
//! let parsed = parse_properties(text, &ParseOptions::default()).unwrap();
//! assert_eq!(parsed.value.len(), 2); // `A=1` and `B=2`
//! assert_eq!(parsed.diagnostics.len(), 2); // trailing `/` and the invalid line
//!
//! let strict = ParseOptions { mode: ParseMode::Strict };
//! assert!(parse_properties(text, &strict).is_err());
//! ```

pub mod ast;
mod importer;
mod legacy;
pub mod syntax;

use std::fmt;

pub use ast::*;
pub use syntax::{Diagnostic, DiagnosticKind, Span};

/// How parsing problems are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParseMode {
    /// Recover from problems and report them as diagnostics.
    #[default]
    Permissive,
    /// Fail on the first problem; all diagnostics are returned in the error.
    Strict,
}

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub mode: ParseMode,
}

/// A successfully parsed value together with the problems that were recovered
/// from while parsing it (always empty in strict mode).
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum T3dError {
    /// Parsing in strict mode found problems.
    Strict(Vec<Diagnostic>),
}

impl fmt::Display for T3dError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            T3dError::Strict(diagnostics) => {
                write!(f, "{} problem(s) found", diagnostics.len())?;
                for d in diagnostics {
                    write!(f, "\n{}", d)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for T3dError {}

fn finish<T>(
    value: T,
    diagnostics: Vec<Diagnostic>,
    options: &ParseOptions,
) -> Result<Parsed<T>, T3dError> {
    if options.mode == ParseMode::Strict && !diagnostics.is_empty() {
        return Err(T3dError::Strict(diagnostics));
    }
    Ok(Parsed { value, diagnostics })
}

/// Parses a list of property statements, such as the body of an UnrealScript
/// `defaultproperties` block. Statements may be assignments or nested
/// `Begin Object` ... `End Object` blocks.
///
/// Spans are byte offsets into `text`.
pub fn parse_properties(
    text: &str,
    options: &ParseOptions,
) -> Result<Parsed<Vec<syntax::Statement>>, T3dError> {
    let (statements, diagnostics) = importer::Importer::run(text);
    finish(statements, diagnostics, options)
}

/// Parses a T3D document: a sequence of top-level `Begin` ... `End` objects.
/// Statements outside of any object are reported as
/// [`DiagnosticKind::UnexpectedStatement`] and discarded.
pub fn parse_document(
    text: &str,
    options: &ParseOptions,
) -> Result<Parsed<Vec<syntax::Object>>, T3dError> {
    let (statements, mut diagnostics) = importer::Importer::run(text);
    let mut objects = Vec::new();
    for statement in statements {
        match statement.kind {
            syntax::StatementKind::Object(o) => objects.push(o),
            _ => {
                let line = text[..statement.span.start].matches('\n').count() + 1;
                diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::UnexpectedStatement,
                    message: format!(
                        "statement outside of an object was ignored: `{}`",
                        &text[statement.span.start..statement.span.end]
                    ),
                    span: statement.span,
                    line,
                });
            }
        }
    }
    diagnostics.sort_by_key(|d| d.span.start);
    finish(objects, diagnostics, options)
}

/// Parses a T3D document into the `HashMap`-based [`T3dObject`] representation,
/// together with any diagnostics.
pub fn parse_t3d_with(
    contents: &str,
    options: &ParseOptions,
) -> Result<Parsed<Vec<T3dObject>>, T3dError> {
    let parsed = parse_document(contents, options)?;
    Ok(Parsed {
        value: parsed.value.into_iter().map(legacy::object).collect(),
        diagnostics: parsed.diagnostics,
    })
}

/// Parses a T3D document permissively into [`T3dObject`]s. Invalid lines are
/// discarded; use [`parse_t3d_with`] to see what was discarded or to parse
/// strictly.
pub fn parse_t3d(contents: &str) -> Vec<T3dObject> {
    let (statements, _) = importer::Importer::run(contents);
    legacy::objects(statements)
}
