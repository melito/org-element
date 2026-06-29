//! AST types for Org mode syntax.
//!
//! This module defines all element and object types according to the Org syntax specification.
//! Elements are block-level structures, while objects are inline content.

pub mod elements;
pub mod objects;

use std::fmt;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

use crate::properties::{Properties, StandardProperties};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use elements::Element;
pub use objects::Object;

/// A node in the Org document AST.
///
/// Nodes can be either elements (block-level) or objects (inline).
/// Each node has standard properties, type-specific properties, and optionally contains children.
#[derive(Debug, Clone)]
pub struct Node {
    /// The variant (element or object).
    pub variant: NodeVariant,

    /// Standard properties common to all nodes.
    pub standard_properties: StandardProperties,

    /// Type-specific properties.
    pub properties: Properties,

    /// Child nodes (for greater elements and recursive objects).
    pub children: Vec<Rc<RefCell<Node>>>,

    /// Weak reference to parent node.
    pub parent: Option<Weak<RefCell<Node>>>,
}

/// Variant of a node - either an element or an object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NodeVariant {
    /// Block-level element.
    Element(Element),
    /// Inline object.
    Object(Object),
}

impl Node {
    /// Create a new node with the given variant.
    pub fn new(variant: NodeVariant, standard_properties: StandardProperties) -> Self {
        Self {
            variant,
            standard_properties,
            properties: Properties::new(),
            children: Vec::new(),
            parent: None,
        }
    }

    /// Create a new element node.
    pub fn element(element: Element, standard_properties: StandardProperties) -> Self {
        Self::new(NodeVariant::Element(element), standard_properties)
    }

    /// Create a new object node.
    pub fn object(object: Object, standard_properties: StandardProperties) -> Self {
        Self::new(NodeVariant::Object(object), standard_properties)
    }

    /// Get the node type.
    pub fn node_type(&self) -> &NodeVariant {
        &self.variant
    }

    /// Check if this is an element node.
    pub fn is_element(&self) -> bool {
        matches!(self.variant, NodeVariant::Element(_))
    }

    /// Check if this is an object node.
    pub fn is_object(&self) -> bool {
        matches!(self.variant, NodeVariant::Object(_))
    }

    /// Get the element type if this is an element.
    pub fn as_element(&self) -> Option<&Element> {
        match &self.variant {
            NodeVariant::Element(e) => Some(e),
            _ => None,
        }
    }

    /// Get the object type if this is an object.
    pub fn as_object(&self) -> Option<&Object> {
        match &self.variant {
            NodeVariant::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Add a child node.
    pub fn add_child(&mut self, child: Rc<RefCell<Node>>) {
        // Set the parent weak reference on the child
        child.borrow_mut().parent = Some(Rc::downgrade(&Rc::new(RefCell::new(self.clone()))));
        self.children.push(child);
    }

    /// Get a reference to the child nodes.
    pub fn children(&self) -> &[Rc<RefCell<Node>>] {
        &self.children
    }

    /// Get the parent node if it exists.
    pub fn parent(&self) -> Option<Rc<RefCell<Node>>> {
        self.parent.as_ref().and_then(|weak| weak.upgrade())
    }

    /// Get the beginning position.
    pub fn begin(&self) -> usize {
        self.standard_properties.begin
    }

    /// Get the ending position.
    pub fn end(&self) -> usize {
        self.standard_properties.end
    }

    /// Get the contents beginning position if it exists.
    pub fn contents_begin(&self) -> Option<usize> {
        self.standard_properties.contents_begin
    }

    /// Get the contents ending position if it exists.
    pub fn contents_end(&self) -> Option<usize> {
        self.standard_properties.contents_end
    }

    /// Get post-blank count.
    pub fn post_blank(&self) -> usize {
        self.standard_properties.post_blank
    }
}

impl fmt::Display for NodeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeVariant::Element(e) => write!(f, "{}", e),
            NodeVariant::Object(o) => write!(f, "{}", o),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let props = StandardProperties::new(0, 100);
        let node = Node::element(Element::Paragraph, props);

        assert!(node.is_element());
        assert!(!node.is_object());
        assert_eq!(node.begin(), 0);
        assert_eq!(node.end(), 100);
    }

    #[test]
    fn test_node_children() {
        let mut parent = Node::element(Element::Paragraph, StandardProperties::new(0, 100));
        let child = Rc::new(RefCell::new(Node::object(
            Object::Bold,
            StandardProperties::new(10, 20),
        )));

        parent.add_child(child.clone());
        assert_eq!(parent.children().len(), 1);
    }
}
