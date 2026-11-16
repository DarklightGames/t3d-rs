//! Line-based importer, modelled on how Unreal Engine imports T3D text and
//! `defaultproperties` blocks: one statement per line, with `Begin`/`End`
//! lines opening and closing objects. Lines that cannot be parsed are reported
//! as [`Diagnostic`]s and either salvaged (when a complete statement precedes
//! the junk) or discarded.

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ast::T3dReference;
use crate::syntax::*;

#[derive(Parser)]
#[grammar = "t3d.pest"]
pub(crate) struct LineParser;

/// Replaces comments (`// ...` and nested `/* ... */`) with spaces, keeping
/// newlines, so that byte offsets into the result match the input exactly.
/// Quoted text (`"..."` and `'...'`) is skipped; quotes end at a newline.
pub(crate) fn blank_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'"' | b'\'') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote && bytes[i] != b'\n' {
                    if quote == b'"'
                        && bytes[i] == b'\\'
                        && i + 1 < bytes.len()
                        && bytes[i + 1] != b'\n'
                    {
                        i += 1;
                    }
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == quote {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    blank(&mut out, i);
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let mut depth = 0usize;
                while i < bytes.len() {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        blank(&mut out, i);
                        blank(&mut out, i + 1);
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        blank(&mut out, i);
                        blank(&mut out, i + 1);
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        blank(&mut out, i);
                        i += 1;
                    }
                }
            }
            _ => i += 1,
        }
    }
    // Only ASCII bytes and whole multi-byte sequences (byte by byte) are
    // replaced with ASCII spaces, so the result is still valid UTF-8.
    String::from_utf8(out).expect("blanking preserves UTF-8 validity")
}

fn blank(out: &mut [u8], i: usize) {
    if out[i] != b'\n' && out[i] != b'\r' {
        out[i] = b' ';
    }
}

struct Frame {
    object: Object,
    line: usize,
}

pub(crate) struct Importer<'a> {
    original: &'a str,
    diagnostics: Vec<Diagnostic>,
    stack: Vec<Frame>,
    top_level: Vec<Statement>,
}

impl<'a> Importer<'a> {
    pub(crate) fn run(text: &'a str) -> (Vec<Statement>, Vec<Diagnostic>) {
        let blanked = blank_comments(text);
        let mut importer = Importer {
            original: text,
            diagnostics: Vec::new(),
            stack: Vec::new(),
            top_level: Vec::new(),
        };

        let mut offset = 0;
        for (line_index, raw_line) in blanked.split('\n').enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            if !line.trim().is_empty() {
                importer.line(line, offset, line_number);
            }
            offset += raw_line.len() + 1;
        }

        // Close any objects that are still open.
        while let Some(frame) = importer.stack.pop() {
            let span = frame.object.span;
            importer.diagnostic(
                DiagnosticKind::UnclosedObject,
                format!(
                    "`Begin {}` has no matching `End {}`",
                    frame.object.type_, frame.object.type_
                ),
                span,
                frame.line,
            );
            importer.push_statement(Statement {
                span,
                kind: StatementKind::Object(frame.object),
            });
        }

        (importer.top_level, importer.diagnostics)
    }

    fn line(&mut self, line: &str, offset: usize, line_number: usize) {
        let trimmed_start = offset + (line.len() - line.trim_start().len());
        let content_span = Span::new(trimmed_start, offset + line.trim_end().len());

        // `Begin <Type> [attributes]`
        if let Some((pair, end)) = parse_line(Rule::begin_line, Rule::begin_prefix, line) {
            let begin = pair.into_inner().next().unwrap();
            let mut inner = begin.into_inner();
            let type_ = inner.next().unwrap().as_str().to_string();
            let attributes = inner.map(|p| build_assignment(p, offset)).collect();
            if let Some(end) = end {
                self.trailing_input(line, offset, end, line_number);
            }
            self.stack.push(Frame {
                object: Object {
                    type_,
                    attributes,
                    statements: Vec::new(),
                    span: content_span,
                },
                line: line_number,
            });
            return;
        }

        // `End <Type>`
        if let Some((pair, end)) = parse_line(Rule::end_line, Rule::end_prefix, line) {
            let end_pair = pair.into_inner().next().unwrap();
            let type_ = end_pair.into_inner().next().unwrap().as_str();
            if let Some(end) = end {
                self.trailing_input(line, offset, end, line_number);
            }
            match self.stack.pop() {
                Some(mut frame) => {
                    if !frame.object.type_.eq_ignore_ascii_case(type_) {
                        self.diagnostic(
                            DiagnosticKind::UnmatchedEnd,
                            format!(
                                "`End {}` does not match `Begin {}`",
                                type_, frame.object.type_
                            ),
                            content_span,
                            line_number,
                        );
                    }
                    frame.object.span.end = content_span.end;
                    let span = frame.object.span;
                    self.push_statement(Statement {
                        span,
                        kind: StatementKind::Object(frame.object),
                    });
                }
                None => self.diagnostic(
                    DiagnosticKind::UnmatchedEnd,
                    format!("`End {}` has no matching `Begin`", type_),
                    content_span,
                    line_number,
                ),
            }
            return;
        }

        // Any other statement.
        match parse_line(Rule::line, Rule::line_prefix, line) {
            Some((pair, end)) => {
                let statement = pair.into_inner().next().unwrap();
                let statement = build_statement(statement, offset);
                // A salvaged prefix must be a meaningful statement: an empty
                // assignment (`Foo=`) followed by junk is not.
                let salvageable =
                    !matches!(&statement.kind, StatementKind::Assignment(a) if a.value.is_none());
                match end {
                    Some(_) if !salvageable => self.invalid_line(line, content_span, line_number),
                    Some(end) => {
                        self.trailing_input(line, offset, end, line_number);
                        self.push_statement(statement);
                    }
                    None => self.push_statement(statement),
                }
            }
            None => self.invalid_line(line, content_span, line_number),
        }
    }

    fn push_statement(&mut self, statement: Statement) {
        match self.stack.last_mut() {
            Some(frame) => frame.object.statements.push(statement),
            None => self.top_level.push(statement),
        }
    }

    fn invalid_line(&mut self, _line: &str, span: Span, line_number: usize) {
        let text = &self.original[span.start..span.end];
        self.diagnostic(
            DiagnosticKind::InvalidLine,
            format!("invalid line was ignored: `{}`", text),
            span,
            line_number,
        );
    }

    fn trailing_input(&mut self, line: &str, offset: usize, end: usize, line_number: usize) {
        let rest = &line[end..];
        let start = offset + end + (rest.len() - rest.trim_start().len());
        let span = Span::new(start, offset + line.trim_end().len());
        let text = &self.original[span.start..span.end];
        self.diagnostic(
            DiagnosticKind::TrailingInput,
            format!("unexpected text after statement was ignored: `{}`", text),
            span,
            line_number,
        );
    }

    fn diagnostic(&mut self, kind: DiagnosticKind, message: String, span: Span, line: usize) {
        self.diagnostics.push(Diagnostic {
            kind,
            message,
            span,
            line,
        });
    }
}

/// Parses `line` with the `full` rule, falling back to the `prefix` rule.
/// Returns the matched pair and, for a prefix match, the byte offset in `line`
/// at which the unparsed remainder starts.
fn parse_line(full: Rule, prefix: Rule, line: &str) -> Option<(Pair<'_, Rule>, Option<usize>)> {
    if let Ok(mut pairs) = LineParser::parse(full, line) {
        return Some((pairs.next().unwrap(), None));
    }
    let pair = LineParser::parse(prefix, line).ok()?.next().unwrap();
    let end = pair.as_span().end();
    Some((pair, Some(end)))
}

fn span_of(pair: &Pair<Rule>, offset: usize) -> Span {
    let s = pair.as_span();
    Span::new(s.start() + offset, s.end() + offset)
}

fn build_statement(pair: Pair<Rule>, offset: usize) -> Statement {
    debug_assert_eq!(pair.as_rule(), Rule::statement);
    let span = span_of(&pair, offset);
    let inner = pair.into_inner().next().unwrap();
    let kind = match inner.as_rule() {
        Rule::assignment => StatementKind::Assignment(build_assignment(inner, offset)),
        Rule::vector_line => {
            let mut parts = inner.into_inner();
            let name = parts.next().unwrap().as_str().to_string();
            let values = parts
                .map(|p| p.as_str().parse::<f64>().unwrap_or(0.0))
                .collect();
            StatementKind::Vector(VectorLine { name, values })
        }
        Rule::inline_struct => {
            let mut parts = inner.into_inner();
            let name = parts.next().unwrap().as_str().to_string();
            let assignments = parts.map(|p| build_assignment(p, offset)).collect();
            StatementKind::InlineStruct(InlineStruct { name, assignments })
        }
        rule => unreachable!("unexpected statement rule {:?}", rule),
    };
    Statement { kind, span }
}

fn build_assignment(pair: Pair<Rule>, offset: usize) -> Assignment {
    debug_assert_eq!(pair.as_rule(), Rule::assignment);
    let span = span_of(&pair, offset);
    let mut name = String::new();
    let mut index = None;
    let mut value = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::id => name = part.as_str().to_string(),
            Rule::index_value => {
                let inner = part.into_inner().next().unwrap();
                index = Some(match inner.as_rule() {
                    Rule::int => match inner.as_str().parse::<i64>() {
                        Ok(i) => Index::Int(i),
                        Err(_) => Index::Identifier(inner.as_str().to_string()),
                    },
                    _ => Index::Identifier(inner.as_str().to_string()),
                });
            }
            Rule::value => value = Some(build_value(part, offset)),
            rule => unreachable!("unexpected assignment rule {:?}", rule),
        }
    }
    Assignment {
        name,
        index,
        value,
        span,
    }
}

fn build_value(pair: Pair<Rule>, offset: usize) -> Value {
    debug_assert_eq!(pair.as_rule(), Rule::value);
    let span = span_of(&pair, offset);
    let inner = pair.into_inner().next().unwrap();
    let text = inner.as_str();
    let kind = match inner.as_rule() {
        Rule::struct_ => ValueKind::Struct(
            inner
                .into_inner()
                .map(|p| build_assignment(p, offset))
                .collect(),
        ),
        Rule::array => ValueKind::Array(
            inner
                .into_inner()
                .map(|element| element.into_inner().next().map(|v| build_value(v, offset)))
                .collect(),
        ),
        Rule::float => ValueKind::Float(text.parse::<f64>().unwrap_or(0.0)),
        // Integers too large for i64 are kept verbatim.
        Rule::int => match text.parse::<i64>() {
            Ok(i) => ValueKind::Int(i),
            Err(_) => ValueKind::Identifier(text.to_string()),
        },
        Rule::string => ValueKind::String(inner.into_inner().next().unwrap().as_str().to_string()),
        Rule::name => ValueKind::Name(inner.into_inner().next().unwrap().as_str().to_string()),
        Rule::reference => {
            let mut parts = inner.into_inner();
            let type_ = parts.next().unwrap().as_str().to_string();
            let path = parts.next().unwrap().as_str().to_string();
            ValueKind::Reference(T3dReference { type_, path })
        }
        Rule::bare => {
            if text.eq_ignore_ascii_case("true") {
                ValueKind::Bool(true)
            } else if text.eq_ignore_ascii_case("false") {
                ValueKind::Bool(false)
            } else {
                ValueKind::Identifier(text.to_string())
            }
        }
        rule => unreachable!("unexpected value rule {:?}", rule),
    };
    Value { kind, span }
}
