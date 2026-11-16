//! Conversion from the ordered [`crate::syntax`] tree into the `HashMap`-based
//! [`T3dObject`] representation returned by [`crate::parse_t3d`].

use std::collections::HashMap;

use crate::ast::*;
use crate::syntax::{Assignment, Index, Object, Statement, StatementKind, Value, ValueKind};

pub(crate) fn object(object: Object) -> T3dObject {
    let mut properties: HashMap<String, T3dPropertyValue> = HashMap::new();
    let mut vector_properties = Vec::new();
    let mut children = Vec::new();

    // Attributes on the `Begin` line (e.g. `Class=Light Name=Light0`) are
    // treated as ordinary properties.
    for attribute in object.attributes {
        insert_property(&mut properties, attribute);
    }
    for statement in object.statements {
        match statement.kind {
            StatementKind::Assignment(a) => insert_property(&mut properties, a),
            StatementKind::InlineStruct(s) => {
                let value = T3dValue::InlineStruct(Box::new(struct_(s.assignments)));
                insert_value(&mut properties, s.name, None, value);
            }
            StatementKind::Vector(v) => vector_properties.push((v.name, v.values)),
            StatementKind::Object(o) => children.push(Box::new(self::object(o))),
        }
    }
    T3dObject {
        type_: object.type_,
        children,
        properties,
        vector_properties,
    }
}

/// Converts the top-level statements of a document, keeping only objects.
pub(crate) fn objects(statements: Vec<Statement>) -> Vec<T3dObject> {
    statements
        .into_iter()
        .filter_map(|s| match s.kind {
            StatementKind::Object(o) => Some(object(o)),
            _ => None,
        })
        .collect()
}

fn insert_property(properties: &mut HashMap<String, T3dPropertyValue>, assignment: Assignment) {
    // An empty assignment (`Foo=`) is how an empty string is exported.
    let value = assignment
        .value
        .map(value)
        .unwrap_or_else(|| T3dValue::String(String::new()));
    let index = assignment.index.map(index);
    insert_value(properties, assignment.name, index, value);
}

fn insert_value(
    properties: &mut HashMap<String, T3dPropertyValue>,
    name: String,
    index: Option<i64>,
    value: T3dValue,
) {
    match properties.get_mut(&name) {
        // Property has already been encountered: append to arrays, otherwise
        // keep the first value.
        Some(T3dPropertyValue::Array(a)) => a.push((index, value)),
        Some(_) => {}
        None => {
            let property = match index {
                Some(_) => T3dPropertyValue::Array(vec![(index, value)]),
                None => T3dPropertyValue::Value(value),
            };
            properties.insert(name, property);
        }
    }
}

fn index(index: Index) -> i64 {
    match index {
        Index::Int(i) => i,
        // The HashMap representation only supports integer indices.
        Index::Identifier(_) => 0,
    }
}

fn struct_(assignments: Vec<Assignment>) -> T3dStruct {
    let mut s = HashMap::new();
    for a in assignments {
        // Indexed struct members (`TextureCoords[0]=...`) are keyed as `Name(Index)`.
        let key = match &a.index {
            Some(i) => format!("{}({})", a.name, i),
            None => a.name.clone(),
        };
        let value = a
            .value
            .map(value)
            .unwrap_or_else(|| T3dValue::String(String::new()));
        s.insert(key, value);
    }
    s
}

fn value(value: Value) -> T3dValue {
    match value.kind {
        ValueKind::Int(i) => T3dValue::Int(i),
        ValueKind::Float(f) => T3dValue::Float(f),
        ValueKind::Bool(b) => T3dValue::Bool(b),
        ValueKind::String(s) => T3dValue::String(s),
        ValueKind::Reference(r) => T3dValue::Reference(r),
        ValueKind::Identifier(s) => T3dValue::Identifier(s),
        // `T3dValue` has no name variant; names are identifier-like.
        ValueKind::Name(s) => T3dValue::Identifier(s),
        ValueKind::Struct(a) => T3dValue::Struct(Box::new(struct_(a))),
        ValueKind::Array(a) => T3dValue::Array(a.into_iter().map(|e| e.map(self::value)).collect()),
    }
}
