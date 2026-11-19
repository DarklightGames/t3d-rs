# t3d-rs

A Rust parser for Unreal Engine [T3D files](https://wiki.beyondunreal.com/Legacy:T3D_File) and the T3D-style property text used in UnrealScript `defaultproperties` blocks.

Python bindings are available in [t3d-python](https://github.com/DarklightGames/t3d-python).

This library is still a work-in-progress.

# Usage

```rust
let contents = std::fs::read_to_string("data.t3d")?;
let objects = t3d::parse_t3d(&contents);

for object in &objects {
    println!("{}", object.type_);
    println!("{:?}", object.children);
    println!("{:?}", object.properties);
    println!("{:?}", object.get("SomeProperty"));
}
```

# Malformed input

Text is parsed line by line, the way Unreal Engine imports it. By default parsing is permissive: a line that cannot be parsed is discarded, text after a complete statement is ignored, and unbalanced `Begin`/`End` lines are repaired. Each problem is reported as a `Diagnostic`. In strict mode, any problem is an error instead.

```rust
use t3d::{parse_t3d_with, ParseMode, ParseOptions};

let parsed = parse_t3d_with(&contents, &ParseOptions::default())?;
for diagnostic in &parsed.diagnostics {
    eprintln!("{}", diagnostic); // e.g. "line 3: invalid line was ignored: `???`"
}

// Fails with `T3dError::Strict` listing every problem.
let strict = parse_t3d_with(&contents, &ParseOptions { mode: ParseMode::Strict });
```

`parse_t3d` is permissive and discards diagnostics.

# Ordered syntax tree

`T3dObject` stores properties in a `HashMap`. For tools that need source order, duplicates and byte spans (for example linters), `parse_document` and `parse_properties` return the ordered tree in `t3d::syntax`. `parse_properties` parses a bare list of statements, such as the body of a `defaultproperties` block.
