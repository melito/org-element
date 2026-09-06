//! # org-element
//!
//! AST types and a tree-sitter-backed parser for Org mode documents.
//!
//! This crate provides a complete implementation of the Org mode element parser,
//! compatible with Emacs org-element.el. It uses tree-sitter for efficient,
//! incremental parsing and provides both idiomatic Rust APIs and Emacs-compatible
//! interfaces.
//!
//! ## Features
//!
//! - Complete coverage of all 26 Org element types
//! - Complete coverage of all 24 Org object types
//! - Incremental parsing via tree-sitter
//! - Iterator-based tree traversal
//! - AST modification and manipulation
//! - Export to HTML
//!
//! ## Example
//!
//! ```rust
//! use org_element::Parser;
//! use org_element::traversal::NodeExt;
//!
//! # fn main() -> org_element::Result<()> {
//! let org_text = "* TODO Headline\n\nParagraph with *bold* text.";
//! let mut parser = Parser::new()?;
//! let ast = parser.parse(org_text)?;
//!
//! // Traverse the AST
//! for node in ast.iter_descendants() {
//!     println!("{:?}", node.borrow().node_type());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Building for WebAssembly
//!
//! The org grammar is compiled from C, and `wasm32-unknown-unknown` has no
//! libc. tree-sitter's bundled mini-sysroot is pulled in automatically, but a
//! **clang with the WebAssembly backend** is still required — Apple's
//! `/usr/bin/clang` does not have one (WebAssembly is LLVM-only; GCC cannot
//! target it).
//!
//! If a wasm build fails with `unable to create target: 'No available targets
//! are compatible with triple "wasm32-unknown-unknown"'`, install LLVM
//! (`brew install llvm`) and add a `.cargo/config.toml` to *your* project:
//!
//! ```toml
//! [env]
//! CC_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/clang"
//! AR_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/llvm-ar"
//! ```
//!
//! This is required build-graph-wide (the upstream `tree-sitter` runtime crate
//! also compiles C for wasm), so setting it on this crate alone is not enough.
//! Most Linux/CI `clang` builds already include the wasm backend.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod ast;
#[cfg(feature = "compat")]
pub mod compat;
pub mod error;
pub mod export;
pub mod highlight;
pub mod parser;
pub mod properties;
pub mod restrictions;
pub mod traversal;

pub use ast::{Element, Node, Object};
pub use error::{Error, Result};
pub use export::HtmlExporter;
pub use highlight::{Highlight, TsHighlighter};
pub use parser::{Parser, language};

/// Re-exported so consumers can name the tree type our single-parse API returns
/// ([`Parser::parse_tree`], [`Parser::parse_tree_edited`]) without depending on
/// `tree-sitter` directly.
pub use tree_sitter::Tree;
pub use properties::Properties;
