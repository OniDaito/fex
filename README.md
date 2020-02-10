# F(its)EX(plorer)
A small rust program to browse fits files in a directory.

## Usage

Build using the following command:

    cargo build --release

Run using the following command (also builds as well):

    cargo run --release <path to image files>

## Requirements

* Rust
* A load of crates such as gtk-rs, fitrs, tiff etc. All of these install pretty easily.
* The gtk+ development libraries so that one can build gtk-rs.

Here are a few links to the major crates used in FEX:

*[https://docs.rs/fitrs/0.5.0/fitrs/ https://docs.rs/fitrs/0.5.0/fitrs/]
*[https://docs.rs/tiff/0.3.1 https://docs.rs/tiff/0.3.1]
*[https://gtk-rs.org/docs/ https://gtk-rs.org/docs/]
