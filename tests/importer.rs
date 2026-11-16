//! Tests for the line-based importer, its diagnostics and the strict mode.
//! Many inputs are taken from malformed `defaultproperties` blocks in shipped
//! Unreal Engine 2 code.

use t3d::syntax::{Index, Statement, StatementKind, ValueKind};
use t3d::{parse_document, parse_properties, DiagnosticKind, ParseMode, ParseOptions, T3dError};

const STRICT: ParseOptions = ParseOptions {
    mode: ParseMode::Strict,
};

fn permissive(text: &str) -> (Vec<Statement>, Vec<t3d::Diagnostic>) {
    let parsed =
        parse_properties(text, &ParseOptions::default()).expect("permissive parsing never fails");
    (parsed.value, parsed.diagnostics)
}

fn kinds(text: &str) -> Vec<DiagnosticKind> {
    permissive(text).1.iter().map(|d| d.kind).collect()
}

fn value_of<'a>(statements: &'a [Statement], name: &str) -> Option<&'a ValueKind> {
    statements.iter().find_map(|s| match &s.kind {
        StatementKind::Assignment(a) if a.name == name => a.value.as_ref().map(|v| &v.kind),
        _ => None,
    })
}

/// Asserts the text gives exactly the given diagnostic kinds in permissive
/// mode, and fails with the same diagnostics in strict mode.
#[track_caller]
fn assert_diagnostics(text: &str, expected: &[DiagnosticKind]) {
    assert_eq!(
        kinds(text),
        expected,
        "permissive diagnostics for {:?}",
        text
    );
    match parse_properties(text, &STRICT) {
        Err(T3dError::Strict(d)) => {
            assert_eq!(d.iter().map(|d| d.kind).collect::<Vec<_>>(), expected)
        }
        Ok(_) => assert!(
            expected.is_empty(),
            "strict mode should have failed for {:?}",
            text
        ),
    }
}

#[test]
fn clean_input_has_no_diagnostics() {
    let text = "A=1\nB=(X=1.0,Y=-.5,Z=+2)\nC(0)=\"str\"\nC[1]=Texture'Pkg.Tex'\nD=None\n";
    assert_diagnostics(text, &[]);
    let (statements, _) = permissive(text);
    assert_eq!(statements.len(), 5);
    assert!(parse_properties(text, &STRICT).is_ok());
}

#[test]
fn values() {
    let (s, _) = permissive(
        "I=-42\nF=.45\nG=5.\nE=1E5\nB=True\nS=\"a \\\"b\\\"\"\nR=Texture'Pkg.Group.Tex'\n\
         P=Pkg.Class.SubObject\nN=idle-empty\nM=20_seconds\nA=(1,,3)\nE2=()\n",
    );
    assert_eq!(value_of(&s, "I"), Some(&ValueKind::Int(-42)));
    assert_eq!(value_of(&s, "F"), Some(&ValueKind::Float(0.45)));
    assert_eq!(value_of(&s, "G"), Some(&ValueKind::Float(5.0)));
    assert_eq!(value_of(&s, "E"), Some(&ValueKind::Float(1e5)));
    assert_eq!(value_of(&s, "B"), Some(&ValueKind::Bool(true)));
    assert_eq!(
        value_of(&s, "S"),
        Some(&ValueKind::String("a \\\"b\\\"".into()))
    );
    assert!(
        matches!(value_of(&s, "R"), Some(ValueKind::Reference(r)) if r.type_ == "Texture" && r.path == "Pkg.Group.Tex")
    );
    assert_eq!(
        value_of(&s, "P"),
        Some(&ValueKind::Identifier("Pkg.Class.SubObject".into()))
    );
    assert_eq!(
        value_of(&s, "N"),
        Some(&ValueKind::Identifier("idle-empty".into()))
    );
    assert_eq!(
        value_of(&s, "M"),
        Some(&ValueKind::Identifier("20_seconds".into()))
    );
    match value_of(&s, "A") {
        Some(ValueKind::Array(a)) => {
            assert_eq!(a.len(), 3);
            assert!(a[1].is_none());
        }
        other => panic!("expected array, got {:?}", other),
    }
    assert_eq!(value_of(&s, "E2"), Some(&ValueKind::Struct(vec![])));
}

#[test]
fn names_and_spaced_references() {
    let (s, d) = permissive("A='stand_idlehip'\nB = ''\nC=Texture 'Pkg.Tex'\n");
    assert!(d.is_empty(), "{:?}", d);
    assert_eq!(
        value_of(&s, "A"),
        Some(&ValueKind::Name("stand_idlehip".into()))
    );
    assert_eq!(value_of(&s, "B"), Some(&ValueKind::Name("".into())));
    assert!(
        matches!(value_of(&s, "C"), Some(ValueKind::Reference(r)) if r.type_ == "Texture" && r.path == "Pkg.Tex")
    );
}

#[test]
fn indices() {
    let (s, d) = permissive("A(0)=1\nA[1]=2\nA( 2 )=3\nB(SOME_ENUM)=4\n");
    assert!(d.is_empty());
    let indices: Vec<_> = s
        .iter()
        .map(|s| match &s.kind {
            StatementKind::Assignment(a) => a.index.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(
        indices,
        vec![
            Some(Index::Int(0)),
            Some(Index::Int(1)),
            Some(Index::Int(2)),
            Some(Index::Identifier("SOME_ENUM".into()))
        ]
    );
}

#[test]
fn struct_with_indexed_members() {
    // From ROEngine/Classes/ROHud.uc (minus its stray closing parenthesis).
    let (s, d) = permissive("Digits=(DigitTexture=Material'HUD.numbers',TextureCoords[0]=(X1=15,Y1=0),TextureCoords[1]=(X1=79,Y1=0))\n");
    assert!(d.is_empty(), "{:?}", d);
    match value_of(&s, "Digits") {
        Some(ValueKind::Struct(members)) => assert_eq!(members.len(), 3),
        other => panic!("expected struct, got {:?}", other),
    }
}

#[test]
fn comments_and_semicolons_are_ignored() {
    let text = "A=1 // comment (with paren\nB=2; ;\n/* multi\n   line C=3\n   comment */ D=4\nE=\"// not a comment\"\n";
    let (s, d) = permissive(text);
    assert!(d.is_empty(), "{:?}", d);
    let names: Vec<_> = s
        .iter()
        .map(|s| match &s.kind {
            StatementKind::Assignment(a) => a.name.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(names, vec!["A", "B", "D", "E"]);
    assert_eq!(
        value_of(&s, "E"),
        Some(&ValueKind::String("// not a comment".into()))
    );
}

#[test]
fn spans_survive_multiline_comments() {
    let text = "/* one\ntwo */\n  Foo=Bar\n";
    let (s, _) = permissive(text);
    let span = s[0].span;
    assert_eq!(&text[span.start..span.end], "Foo=Bar");
    // Diagnostics report the right line after a multi-line comment.
    let (_, d) = permissive("/*\n\n*/\n???\n");
    assert_eq!(d[0].line, 4);
    assert_eq!(&"/*\n\n*/\n???\n"[d[0].span.start..d[0].span.end], "???");
}

#[test]
fn crlf_line_endings() {
    let (s, d) = permissive("A=Foo\r\nB=(X=1)\r\n");
    assert!(d.is_empty());
    assert_eq!(
        value_of(&s, "A"),
        Some(&ValueKind::Identifier("Foo".into()))
    );
}

#[test]
fn begin_object_blocks() {
    let text = "Begin Object Class=GUIButton Name=OkButton\n    Caption=\"OK\"\nEnd Object\nOkButtonRef=GUIButton'OkButton'\n";
    let (s, d) = permissive(text);
    assert!(d.is_empty(), "{:?}", d);
    match &s[0].kind {
        StatementKind::Object(o) => {
            assert_eq!(o.type_, "Object");
            assert_eq!(
                o.attribute("class").map(|v| &v.kind),
                Some(&ValueKind::Identifier("GUIButton".into()))
            );
            assert_eq!(o.statements.len(), 1);
            assert_eq!(
                &text[o.span.start..o.span.end],
                &text[..text.find("\nOkButtonRef").unwrap()]
            );
        }
        other => panic!("expected object, got {:?}", other),
    }
    // Lowercase keywords are accepted.
    assert_diagnostics("begin object class=Foo name=Bar\nend object\n", &[]);
}

// --- Recovery from malformed input (examples from shipped UE2 code) ---

#[test]
fn trailing_input_keeps_the_statement() {
    for (text, name) in [
        ("PitchDownLimit=50000/\n", "PitchDownLimit"),
        (
            "WinWidth= 0.43000                           .\n",
            "WinWidth",
        ),
        (
            "Grenades(0)=(Item=class'Foo.Bar',Amount=2)        NebelGranate39Weapon\n",
            "Grenades",
        ),
        ("Digits=(A=(X1=1),B=(X1=2)))\n", "Digits"),
        ("StyleName=\"AltComboButton\"\"\n", "StyleName"),
        (
            "BindingLabel(35)=\"Toggle \"BehindView\"\"\n",
            "BindingLabel",
        ),
    ] {
        assert_diagnostics(text, &[DiagnosticKind::TrailingInput]);
        let (s, _) = permissive(text);
        assert!(
            value_of(&s, name).is_some(),
            "{:?} should keep {}",
            text,
            name
        );
    }
    let (s, d) = permissive("PitchDownLimit=50000/\n");
    assert_eq!(value_of(&s, "PitchDownLimit"), Some(&ValueKind::Int(50000)));
    assert_eq!(d[0].line, 1);
    assert!(d[0].message.contains("`/`"), "{}", d[0].message);
}

#[test]
fn invalid_lines_are_discarded() {
    for text in [
        "0.905471\n",                                    // value with no property name
        "Texture'Pkg.Tex'\n",                            // ditto
        "/ Moving fire anims\n",                         // single-slash comment
        "WinHeight1.0\n",                                // missing `=`
        "RemoteRole==ROLE_None\n",                       // doubled `=`
        "DefaultStyleNames(35)=\"GUI2K4.STY2CheckBox\n", // unterminated string
        "Overlay=Texture'Pkg.Tex\n",                     // unterminated reference
    ] {
        assert_diagnostics(text, &[DiagnosticKind::InvalidLine]);
        let (s, _) = permissive(text);
        assert!(s.is_empty(), "{:?} should be discarded, got {:?}", text, s);
    }
    // Surrounding lines are unaffected.
    let (s, d) = permissive("A=1\n0.905471\nB=2\n");
    assert_eq!(s.len(), 2);
    assert_eq!(d[0].line, 2);
}

#[test]
fn unbalanced_objects() {
    assert_diagnostics(
        "Begin Object Class=Foo Name=Bar\nA=1\n",
        &[DiagnosticKind::UnclosedObject],
    );
    let (s, _) = permissive("Begin Object Class=Foo Name=Bar\nA=1\n");
    assert!(matches!(&s[0].kind, StatementKind::Object(o) if o.statements.len() == 1));

    assert_diagnostics("A=1\nEnd Object\n", &[DiagnosticKind::UnmatchedEnd]);
    assert_diagnostics("Begin Object\nEnd Actor\n", &[DiagnosticKind::UnmatchedEnd]);
}

#[test]
fn document_rejects_top_level_statements() {
    let text = "A=1\nBegin Map\nEnd Map\n";
    let parsed = parse_document(text, &ParseOptions::default()).unwrap();
    assert_eq!(parsed.value.len(), 1);
    assert_eq!(
        parsed.diagnostics[0].kind,
        DiagnosticKind::UnexpectedStatement
    );
    assert!(parse_document(text, &STRICT).is_err());
}

#[test]
fn parse_t3d_with_reports_diagnostics() {
    let text = "Begin Object\n    Foo=1\n    ???\nEnd Object\n";
    let parsed = t3d::parse_t3d_with(text, &ParseOptions::default()).unwrap();
    assert_eq!(parsed.value.len(), 1);
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].line, 3);
    assert!(t3d::parse_t3d_with(text, &STRICT).is_err());
    // The legacy API stays permissive.
    assert_eq!(t3d::parse_t3d(text).unwrap().len(), 1);
}

#[test]
fn offset_spans_shifts_everything() {
    let prefix = "defaultproperties\n{\n";
    let body = "Begin Object Name=Foo\n    A=(X=1,Y=(2,3))\nEnd Object\n";
    let full = format!("{}{}", prefix, body);
    let (mut statements, _) = permissive(body);
    statements
        .iter_mut()
        .for_each(|s| s.offset_spans(prefix.len()));
    let StatementKind::Object(o) = &statements[0].kind else {
        panic!()
    };
    assert_eq!(
        &full[o.attributes[0].span.start..o.attributes[0].span.end],
        "Name=Foo"
    );
    let StatementKind::Assignment(a) = &o.statements[0].kind else {
        panic!()
    };
    let Some(ValueKind::Struct(members)) = a.value.as_ref().map(|v| &v.kind) else {
        panic!()
    };
    let y = members[1].value.as_ref().unwrap();
    assert_eq!(&full[y.span.start..y.span.end], "(2,3)");
}
