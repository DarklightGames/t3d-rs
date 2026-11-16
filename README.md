# t3d-rs

A Rust parser for Unreal Engine [T3D files](https://wiki.beyondunreal.com/Legacy:T3D_File).

Python bindings are available in [t3d-python](https://github.com/DarklightGames/t3d-python).

This library is still a work-in-progress.

# Usage

```rust
let contents = std::fs::read_to_string("data.t3d")?;
let objects = t3d::parse_t3d(&contents)?;

for object in &objects {
    println!("{}", object.type_);
    println!("{:?}", object.children);
    println!("{:?}", object.properties);
    println!("{:?}", object.get("SomeProperty"));
}
```
