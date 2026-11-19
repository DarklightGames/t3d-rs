use std::path::Path;

use t3d::{parse_t3d, T3dObject, T3dPropertyValue, T3dReference, T3dValue};

fn parse_file(name: &str) -> Vec<T3dObject> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    parse_t3d(&contents)
}

#[test]
fn property_assignment_with_reference_array() {
    let objects = parse_t3d(
        "
        Begin Object
            SomeArray=(StaticMesh'Foo.Bar',StaticMesh'Baz.Boo')
        End Object
        ",
    );
    let object = objects.first().unwrap();
    let reference = |type_: &str, path: &str| {
        Some(T3dValue::Reference(T3dReference {
            type_: type_.to_string(),
            path: path.to_string(),
        }))
    };
    assert_eq!(
        object.get("SomeArray"),
        Some(&T3dPropertyValue::Value(T3dValue::Array(vec![
            reference("StaticMesh", "Foo.Bar"),
            reference("StaticMesh", "Baz.Boo"),
        ])))
    );
}

#[test]
fn ut99_polygon() {
    let objects = parse_t3d(
        "
            Begin Polygon Item=2DLoftSIDE Texture=DecayedS.Wall.dWallA3 Flags=32768
                Origin   -01023.999878,+00000.000000,-01056.000122
                Normal   -00000.923880,+00000.382684,-00000.000000
                TextureU +00000.191342,+00000.461940,+00000.000000
                TextureV +00000.000000,+00000.000000,-00000.500000
                Pan      U=57 V=99
                Vertex   -01024.000000,+00000.000000,-00016.000090
                Vertex   -01024.000000,+00000.000000,+00495.999908
                Vertex   -00724.077332,+00724.077271,+00495.999939
                Vertex   -00724.077271,+00724.077271,-00016.000063
            End Polygon
        ",
    );
    assert_eq!(1, objects.len());
    let object = objects.first().unwrap();
    assert_eq!(4, object.properties.len());
    match object.get("Pan") {
        Some(T3dPropertyValue::Value(T3dValue::InlineStruct(pan))) => assert_eq!(2, pan.len()),
        other => panic!("Expected inline struct value for Pan, got {:?}", other),
    }
}

#[test]
fn property_assignment_empty_value() {
    let objects = parse_t3d(
        "
            Begin Object
                MyEmptyString=
                TheNextString=\"\"
            End Object
        ",
    );
    assert_eq!(1, objects.len());
    let object = objects.first().unwrap();
    assert_eq!(2, object.properties.len());
    assert_eq!(
        object.get("MyEmptyString"),
        Some(&T3dPropertyValue::Value(T3dValue::String(String::new())))
    );
}

#[test]
fn dm_death_fan() {
    parse_file("DMDeathFan.t3d");
}

#[test]
fn mg_ntw2_lib_stairway() {
    parse_file("MG_NTW2_Lib_Stairway.t3d");
}

#[test]
fn mirra() {
    parse_file("mirra.t3d");
}

#[test]
fn terrain_info() {
    parse_file("terraininfo.t3d");
}

#[test]
fn m212_64_bit() {
    // This has 64-bit integers in it which previously failed to parse.
    parse_file("GryffindorM212.t3d");
}
