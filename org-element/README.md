# org-element

AST types and a [tree-sitter](https://tree-sitter.github.io/)-backed parser for
[Org mode](https://orgmode.org/) documents, compatible with Emacs
[`org-element.el`](https://orgmode.org/worg/dev/org-element-api.html).

## Features

- Complete coverage of all 26 Org element types
- Complete coverage of all 24 Org object types
- Incremental parsing via tree-sitter
- Iterator-based tree traversal
- AST modification and manipulation
- Export to HTML

## Example

```rust
use org_element::Parser;
use org_element::traversal::NodeExt;

fn main() -> org_element::Result<()> {
    let org_text = "* TODO Headline\n\nParagraph with *bold* text.";
    let mut parser = Parser::new()?;
    let ast = parser.parse(org_text)?;

    for node in ast.iter_descendants() {
        println!("{:?}", node.borrow().node_type());
    }
    Ok(())
}
```

## Cargo features

- `serde` *(default)* — derive `Serialize`/`Deserialize` for AST types.

## WebAssembly (`wasm32-unknown-unknown`)

The org grammar is compiled from C, and `wasm32-unknown-unknown` has no libc.
The build pulls in tree-sitter's bundled mini-sysroot automatically, but it
still needs a **clang with the WebAssembly backend** — Apple's `/usr/bin/clang`
does not have one (WebAssembly is LLVM-only; GCC cannot target it).

With the **`wasm` feature** enabled, this crate's build script will try to
locate a suitable clang on macOS (Homebrew LLVM) and emit a warning telling you
what to do otherwise. The feature is off by default, so a native build never
probes for a wasm compiler — users who don't need wasm are unaffected even if
they have no wasm-capable clang.

However, the upstream `tree-sitter` runtime crate also compiles C for wasm and
does **not** auto-detect, so the reliable, build-graph-wide fix is to point the
C toolchain at an LLVM clang regardless. Install one (`brew install llvm`) and
add a `.cargo/config.toml` to *your* project:

```toml
[env]
CC_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/clang"
AR_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/llvm-ar"
```

On most Linux/CI images the system `clang` already includes the wasm backend,
so no configuration is needed there.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
