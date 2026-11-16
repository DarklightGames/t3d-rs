use std::collections::HashMap;
use std::fmt;

pub type T3dStruct = HashMap<String, T3dValue>;

#[derive(Debug, Clone, PartialEq)]
pub struct T3dReference {
    pub type_: String,
    pub path: String,
}

impl fmt::Display for T3dReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}'{}'", self.type_, self.path)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum T3dValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Struct(Box<T3dStruct>),
    InlineStruct(Box<T3dStruct>),
    Reference(T3dReference),
    Identifier(String),
    Vector((f64, f64, f64)),
    Array(Vec<Option<T3dValue>>),
}

impl fmt::Display for T3dValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            T3dValue::Int(value) => write!(f, "{}", value),
            T3dValue::Float(value) => write!(f, "{}", value),
            T3dValue::Bool(value) => write!(f, "{}", value),
            T3dValue::String(value) => write!(f, "\"{}\"", value),
            T3dValue::Struct(_) => write!(f, "Struct()"),
            T3dValue::InlineStruct(_) => write!(f, "InlineStruct()"),
            T3dValue::Reference(value) => write!(f, "{}", value),
            T3dValue::Identifier(value) => write!(f, "{}", value),
            T3dValue::Vector(value) => write!(f, "{:?}", value),
            T3dValue::Array(_) => write!(f, "Array()"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct T3dPropertyAssignment {
    pub name: String,
    pub index: Option<i64>,
    pub value: T3dValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct T3dPropertyAssignmentVector {
    pub name: String,
    pub value: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum T3dPropertyValue {
    Value(T3dValue),
    Array(Vec<(Option<i64>, T3dValue)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum T3dObjectStatement {
    Object(T3dObject),
    PropertyAssignment(T3dPropertyAssignment),
    PropertyAssignmentVector(T3dPropertyAssignmentVector),
}

#[derive(Debug, Clone, PartialEq)]
pub struct T3dObject {
    pub type_: String,
    pub children: Vec<Box<T3dObject>>,
    pub properties: HashMap<String, T3dPropertyValue>,
    pub vector_properties: Vec<(String, Vec<f64>)>,
}

impl T3dObject {
    /// Returns the value of the property with the given name, if present.
    pub fn get(&self, name: &str) -> Option<&T3dPropertyValue> {
        self.properties.get(name)
    }
}
