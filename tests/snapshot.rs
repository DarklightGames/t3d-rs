//! Snapshot tests that pin the output of `parse_t3d` for the files in `tests/data`.
//!
//! `T3dObject` stores properties in a `HashMap`, so the output is dumped in a
//! canonical form (sorted keys) and compared by a stable FNV-1a hash stored in
//! `tests/snapshots.txt`. On a mismatch the full dump is written to
//! `target/snapshots/<file>.txt` so it can be diffed.
//!
//! To regenerate after an intentional change: `UPDATE_SNAPSHOTS=1 cargo test --test snapshot`.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use t3d::{parse_t3d, T3dObject, T3dPropertyValue, T3dStruct, T3dValue};

const DATA_FILES: &[&str] = &[
    "DMDeathFan.t3d",
    "GryffindorM212.t3d",
    "MG_NTW2_Lib_Stairway.t3d",
    "mirra.t3d",
    "terraininfo.t3d",
];

fn dump_struct(out: &mut String, s: &T3dStruct) {
    let sorted: BTreeMap<_, _> = s.iter().collect();
    out.push('{');
    for (k, v) in sorted {
        write!(out, "{}=", k).unwrap();
        dump_value(out, v);
        out.push(',');
    }
    out.push('}');
}

fn dump_value(out: &mut String, v: &T3dValue) {
    match v {
        T3dValue::Struct(s) => {
            out.push_str("Struct");
            dump_struct(out, s)
        }
        T3dValue::InlineStruct(s) => {
            out.push_str("InlineStruct");
            dump_struct(out, s)
        }
        T3dValue::Array(a) => {
            out.push_str("Array[");
            for e in a {
                match e {
                    Some(e) => dump_value(out, e),
                    None => out.push_str("<empty>"),
                }
                out.push(',');
            }
            out.push(']');
        }
        // Leaf values have a deterministic Debug representation.
        other => write!(out, "{:?}", other).unwrap(),
    }
}

fn dump_object(out: &mut String, o: &T3dObject, depth: usize) {
    let pad = "  ".repeat(depth);
    writeln!(out, "{}Begin {}", pad, o.type_).unwrap();
    let sorted: BTreeMap<_, _> = o.properties.iter().collect();
    for (k, v) in sorted {
        write!(out, "{}  {} = ", pad, k).unwrap();
        match v {
            T3dPropertyValue::Value(v) => dump_value(out, v),
            T3dPropertyValue::Array(a) => {
                out.push_str("Indexed[");
                for (i, v) in a {
                    write!(out, "{:?}:", i).unwrap();
                    dump_value(out, v);
                    out.push(',');
                }
                out.push(']');
            }
        }
        out.push('\n');
    }
    for (k, v) in &o.vector_properties {
        writeln!(out, "{}  {} ~ {:?}", pad, k, v).unwrap();
    }
    for c in &o.children {
        dump_object(out, c, depth + 1);
    }
    writeln!(out, "{}End {}", pad, o.type_).unwrap();
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn snapshot_line(name: &str) -> (String, String) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let contents = std::fs::read_to_string(root.join("tests/data").join(name)).unwrap();
    let objects = parse_t3d(&contents);
    let mut dump = String::new();
    for o in &objects {
        dump_object(&mut dump, o, 0);
    }
    let line = format!(
        "{} objects={} lines={} hash={:016x}",
        name,
        objects.len(),
        dump.lines().count(),
        fnv1a(&dump)
    );
    (line, dump)
}

#[test]
fn snapshots_match() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = root.join("tests/snapshots.txt");
    let results: Vec<_> = DATA_FILES.iter().map(|n| (n, snapshot_line(n))).collect();

    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        let text: String = results
            .iter()
            .map(|(_, (l, _))| format!("{}\n", l))
            .collect();
        std::fs::write(&snapshot_path, text).unwrap();
    }

    let expected = std::fs::read_to_string(&snapshot_path).expect("missing tests/snapshots.txt");
    let mut mismatches = Vec::new();
    for (name, (line, dump)) in &results {
        let want = expected.lines().find(|l| l.split(' ').next() == Some(name));
        if want != Some(line.as_str()) {
            let out_dir = root.join("target/snapshots");
            std::fs::create_dir_all(&out_dir).unwrap();
            std::fs::write(out_dir.join(format!("{}.txt", name)), dump).unwrap();
            mismatches.push(format!("  expected: {:?}\n  actual:   {}", want, line));
        }
    }
    assert!(
        mismatches.is_empty(),
        "snapshot mismatch (dumps in target/snapshots/):\n{}",
        mismatches.join("\n")
    );
}
