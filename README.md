# Baked Font Generator

A simple tool to generate bitmap fonts that serializes into `baked-font`'s format.

## Usage
Clone this repository and invoke it via Cargo like this:
```shell
cargo run --release -- (font family) (font size) (padding) (output file) (zstd compression level) (glyph text files...)
```
* There could be one or more glyph text files, each glyph is separated by a newline. Each glyph could consist of one or two `char` units.
* Compression level of 0 means no compression at all.
