//! Wasm link smoke test — see this crate's `Cargo.toml` for why it exists.
//!
//! The body must actually *reference* the parser and exporter so the linker
//! keeps their object code (and the tree-sitter grammar's). A `#[no_mangle]`
//! exported function guarantees `rust-lld` performs a real link rather than
//! dead-stripping everything to nothing.

use org_element::{HtmlExporter, Parser};

/// Parse a fixed Org snippet and export it to HTML.
fn run() -> Result<String, Box<dyn std::error::Error>> {
    let mut parser = Parser::new()?;
    let ast = parser.parse("* TODO Headline\n\nA *bold* paragraph.\n")?;
    Ok(HtmlExporter::new().export(&ast)?)
}

/// Exported from the cdylib so the call graph above cannot be optimized away;
/// the return value is incidental — the point is forcing parser + exporter +
/// tree-sitter grammar to all link into one wasm module. Returns the exported
/// HTML's byte length, or 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn org_html_len() -> u32 {
    run().map(|html| html.len() as u32).unwrap_or(0)
}
