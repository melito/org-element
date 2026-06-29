//! Emacs `org-element.el` compatible API.
//!
//! This module provides function names and interfaces that closely mirror
//! the Emacs Lisp org-element API, making it easier for Emacs users to
//! transition to using this Rust implementation. It is gated behind the
//! `compat` feature.
//!
//! # Example
//!
//! ```rust
//! use org_element::compat::*;
//!
//! # fn main() -> org_element::Result<()> {
//! let ast = org_element_parse_buffer("* Headline\n\nParagraph.", None)?;
//!
//! // Map over all headlines
//! org_element_map(&ast, &[org_element::Element::Headline], |node| {
//!     println!("type: {}", org_element_type(node));
//! }, None, None);
//! # Ok(())
//! # }
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use crate::{Element, Node};

/// Parse an Org buffer into an AST.
///
/// This is analogous to `org-element-parse-buffer` in Emacs.
///
/// # Arguments
///
/// * `source` - The Org mode text to parse
/// * `granularity` - Optional parsing granularity (not yet implemented)
///
/// # Example
///
/// ```rust
/// # fn main() -> org_element::Result<()> {
/// let ast = org_element::compat::org_element_parse_buffer("* Headline\n\nParagraph.", None)?;
/// # Ok(())
/// # }
/// ```
pub fn org_element_parse_buffer(
    source: &str,
    _granularity: Option<&str>,
) -> crate::Result<Rc<RefCell<Node>>> {
    let mut parser = crate::Parser::new()?;
    parser.parse(source)
}

/// Get the type of an element or object.
///
/// Analogous to `org-element-type` in Emacs.
pub fn org_element_type(node: &Rc<RefCell<Node>>) -> String {
    node.borrow().node_type().to_string()
}

/// Map a function over all matching nodes in the AST.
///
/// Analogous to `org-element-map` in Emacs.
///
/// # Arguments
///
/// * `root` - The root node to start traversal
/// * `types` - Element/object types to match (empty = all)
/// * `func` - Function to call for each matching node
/// * `_info` - Optional parse tree info (not yet implemented)
/// * `_first_match` - Whether to stop after first match (not yet implemented)
///
/// # Example
///
/// ```rust
/// use org_element::{Element, compat::{org_element_parse_buffer, org_element_map}};
///
/// # fn main() -> org_element::Result<()> {
/// let ast = org_element_parse_buffer("* Headline\n", None)?;
/// org_element_map(&ast, &[Element::Headline], |node| {
///     println!("found headline at {}", node.borrow().begin());
/// }, None, None);
/// # Ok(())
/// # }
/// ```
pub fn org_element_map<F>(
    root: &Rc<RefCell<Node>>,
    types: &[Element],
    mut func: F,
    _info: Option<()>,
    _first_match: Option<bool>,
) where
    F: FnMut(&Rc<RefCell<Node>>),
{
    use crate::traversal::NodeExt;

    if types.is_empty() {
        // Map over all nodes
        for node in root.iter_descendants() {
            func(&node);
        }
    } else {
        // Map over matching types
        for element_type in types {
            for node in root.find_elements(*element_type) {
                func(&node);
            }
        }
    }
}

/// Get a property value from a node.
///
/// Analogous to `org-element-property` in Emacs.
pub fn org_element_property(node: &Rc<RefCell<Node>>, property: &str) -> Option<String> {
    let node_ref = node.borrow();

    // Check standard properties
    match property {
        "begin" => Some(node_ref.begin().to_string()),
        "end" => Some(node_ref.end().to_string()),
        "post-blank" => Some(node_ref.post_blank().to_string()),
        _ => {
            // Check custom properties
            node_ref
                .properties
                .get_string(property)
                .map(|s| s.to_string())
        }
    }
}

/// Get all contents (children) of a node.
///
/// Analogous to `org-element-contents` in Emacs.
pub fn org_element_contents(node: &Rc<RefCell<Node>>) -> Vec<Rc<RefCell<Node>>> {
    node.borrow().children().to_vec()
}

/// Get the lineage (all ancestors) of a node.
///
/// Analogous to `org-element-lineage` in Emacs.
pub fn org_element_lineage(node: &Rc<RefCell<Node>>) -> Vec<Rc<RefCell<Node>>> {
    use crate::traversal::NodeExt;
    node.ancestors()
}
