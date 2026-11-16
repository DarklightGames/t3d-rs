//! Ordered, span-carrying syntax tree produced by the line-based importer.
//!
//! Unlike the `T3d*` types in [`crate::ast`], these keep statements in source
//! order (including duplicates) and record the byte span of every node, which
//! is what tools that inspect or lint T3D text need.

use crate::ast::T3dReference;
use std::fmt;

/// A byte range in the parsed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// Returns this span shifted by `offset` bytes.
    pub fn offset(self, offset: usize) -> Self {
        Span {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

/// An array index: `Foo(1)=`, `Foo[1]=` or `Foo(SomeEnumValue)=`.
#[derive(Debug, Clone, PartialEq)]
pub enum Index {
    Int(i64),
    Identifier(String),
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Index::Int(i) => write!(f, "{}", i),
            Index::Identifier(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    /// The raw contents between the quotes (escape sequences are not processed).
    String(String),
    Reference(T3dReference),
    /// A single-quoted name, e.g. `'SomeName'` (the contents between the quotes).
    Name(String),
    /// Any other unquoted token, e.g. `None`, `ROLE_Authority` or `Package.Class.SubObject`.
    Identifier(String),
    Struct(Vec<Assignment>),
    /// Array literal; `None` entries are empty slots, e.g. `(1,,3)`.
    Array(Vec<Option<Value>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub kind: ValueKind,
    pub span: Span,
}

/// `Name[(Index)]=Value`. `value` is `None` for an empty assignment (`Name=`).
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub name: String,
    pub index: Option<Index>,
    pub value: Option<Value>,
    pub span: Span,
}

/// `Origin -00512.000000,+00256.000000,+00000.000000`
#[derive(Debug, Clone, PartialEq)]
pub struct VectorLine {
    pub name: String,
    pub values: Vec<f64>,
}

/// `Pan U=57 V=99`
#[derive(Debug, Clone, PartialEq)]
pub struct InlineStruct {
    pub name: String,
    pub assignments: Vec<Assignment>,
}

/// `Begin <Type> [attributes...]` ... `End <Type>`
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub type_: String,
    /// Assignments on the `Begin` line, e.g. `Class=Light Name=Light0`.
    pub attributes: Vec<Assignment>,
    pub statements: Vec<Statement>,
    pub span: Span,
}

impl Object {
    /// Returns the value of the attribute on the `Begin` line with the given
    /// name (case-insensitive), e.g. `Class` or `Name`.
    pub fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .and_then(|a| a.value.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Assignment(Assignment),
    Vector(VectorLine),
    InlineStruct(InlineStruct),
    Object(Object),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticKind {
    /// The line could not be parsed and was discarded.
    InvalidLine,
    /// A complete statement was parsed but the rest of the line was ignored.
    TrailingInput,
    /// An `End` line with no matching `Begin`, or whose type does not match.
    UnmatchedEnd,
    /// A `Begin` line with no matching `End` before the end of the input.
    UnclosedObject,
    /// A statement in a place where it is not allowed (e.g. a property
    /// assignment outside of any object in a T3D document).
    UnexpectedStatement,
}

/// A problem that was recovered from while parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Span,
    /// 1-based line number of `span.start`.
    pub line: usize,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl Value {
    /// Shifts every span in this value by `offset` bytes.
    pub fn offset_spans(&mut self, offset: usize) {
        self.span = self.span.offset(offset);
        match &mut self.kind {
            ValueKind::Struct(assignments) => {
                assignments.iter_mut().for_each(|a| a.offset_spans(offset))
            }
            ValueKind::Array(elements) => elements
                .iter_mut()
                .flatten()
                .for_each(|v| v.offset_spans(offset)),
            _ => {}
        }
    }
}

impl Assignment {
    /// Shifts every span in this assignment by `offset` bytes.
    pub fn offset_spans(&mut self, offset: usize) {
        self.span = self.span.offset(offset);
        if let Some(value) = &mut self.value {
            value.offset_spans(offset);
        }
    }
}

impl Object {
    /// Shifts every span in this object by `offset` bytes.
    pub fn offset_spans(&mut self, offset: usize) {
        self.span = self.span.offset(offset);
        self.attributes
            .iter_mut()
            .for_each(|a| a.offset_spans(offset));
        self.statements
            .iter_mut()
            .for_each(|s| s.offset_spans(offset));
    }
}

impl Statement {
    /// Shifts every span in this statement by `offset` bytes. Useful when the
    /// parsed text was a slice of a larger document.
    pub fn offset_spans(&mut self, offset: usize) {
        self.span = self.span.offset(offset);
        match &mut self.kind {
            StatementKind::Assignment(a) => a.offset_spans(offset),
            StatementKind::InlineStruct(s) => s
                .assignments
                .iter_mut()
                .for_each(|a| a.offset_spans(offset)),
            StatementKind::Vector(_) => {}
            StatementKind::Object(o) => o.offset_spans(offset),
        }
    }
}
