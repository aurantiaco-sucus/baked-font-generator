# Baked Font Generator

A simple tool to generate bitmap fonts that serializes into a simple format, optimised for UTF-16 lookup (single unit 
or surrogate pair). This tool `baked-font-generator` and the corresponding bitmap font library `baked-font` are written 
in Rust, using `serde` and `postcard` for storage format.

## Usage
Clone this repository and invoke it via Cargo like this:
```shell
cargo run --release -- (font family) (font size) (padding) (output file) (glyph text files...)
```
There could be one or more glyph text files, each glyph is separated by a newline. Each glyph could consist of one or 
two UTF-16 code units.