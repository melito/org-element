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

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod ast;
pub mod error;
pub mod export;
pub mod parser;
pub mod properties;
pub mod restrictions;
pub mod traversal;

pub use ast::{Element, Node, Object};
pub use error::{Error, Result};
pub use export::HtmlExporter;
pub use parser::Parser;
pub use properties::Properties;
