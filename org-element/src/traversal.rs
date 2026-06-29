//! Tree traversal and iteration APIs.
//!
//! This module provides efficient ways to traverse and query the AST.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::ast::{Element, Node, Object};

/// Iterator for depth-first traversal of the AST.
pub struct DepthFirstIter {
    stack: Vec<Rc<RefCell<Node>>>,
}

impl DepthFirstIter {
    /// Create a new depth-first iterator starting from a node.
    pub fn new(root: Rc<RefCell<Node>>) -> Self {
        Self { stack: vec![root] }
    }
}

impl Iterator for DepthFirstIter {
    type Item = Rc<RefCell<Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.stack.pop() {
            // Push children in reverse order so they're popped in correct order
            let children = node.borrow().children().to_vec();
            for child in children.into_iter().rev() {
                self.stack.push(child);
            }
            Some(node)
        } else {
            None
        }
    }
}

/// Iterator for breadth-first traversal of the AST.
pub struct BreadthFirstIter {
    queue: VecDeque<Rc<RefCell<Node>>>,
}

impl BreadthFirstIter {
    /// Create a new breadth-first iterator starting from a node.
    pub fn new(root: Rc<RefCell<Node>>) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(root);
        Self { queue }
    }
}

impl Iterator for BreadthFirstIter {
    type Item = Rc<RefCell<Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.queue.pop_front() {
            // Enqueue children
            for child in node.borrow().children() {
                self.queue.push_back(child.clone());
            }
            Some(node)
        } else {
            None
        }
    }
}

/// Query builder for filtering and finding nodes in the AST.
pub struct Query {
    root: Rc<RefCell<Node>>,
    element_filter: Option<Vec<Element>>,
    object_filter: Option<Vec<Object>>,
    predicate: Option<Box<dyn Fn(&Node) -> bool>>,
}

impl Query {
    /// Create a new query starting from a root node.
    pub fn new(root: Rc<RefCell<Node>>) -> Self {
        Self {
            root,
            element_filter: None,
            object_filter: None,
            predicate: None,
        }
    }

    /// Filter by element types.
    pub fn filter_elements(mut self, elements: Vec<Element>) -> Self {
        self.element_filter = Some(elements);
        self
    }

    /// Filter by a single element type.
    pub fn filter_element(self, element: Element) -> Self {
        self.filter_elements(vec![element])
    }

    /// Filter by object types.
    pub fn filter_objects(mut self, objects: Vec<Object>) -> Self {
        self.object_filter = Some(objects);
        self
    }

    /// Filter by a single object type.
    pub fn filter_object(self, object: Object) -> Self {
        self.filter_objects(vec![object])
    }

    /// Add a custom predicate filter.
    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&Node) -> bool + 'static,
    {
        self.predicate = Some(Box::new(predicate));
        self
    }

    /// Execute the query and return matching nodes.
    pub fn collect(self) -> Vec<Rc<RefCell<Node>>> {
        DepthFirstIter::new(self.root)
            .filter(|node| {
                let node_ref = node.borrow();

                // Check element filter
                if let Some(ref elements) = self.element_filter {
                    if let Some(element) = node_ref.as_element() {
                        if !elements.contains(element) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Check object filter
                if let Some(ref objects) = self.object_filter {
                    if let Some(object) = node_ref.as_object() {
                        if !objects.contains(object) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Check custom predicate
                if let Some(ref pred) = self.predicate {
                    if !pred(&node_ref) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Find the first matching node.
    pub fn find_first(self) -> Option<Rc<RefCell<Node>>> {
        DepthFirstIter::new(self.root).find(|node| {
            let node_ref = node.borrow();

            if let Some(ref elements) = self.element_filter {
                if let Some(element) = node_ref.as_element() {
                    if !elements.contains(element) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(ref objects) = self.object_filter {
                if let Some(object) = node_ref.as_object() {
                    if !objects.contains(object) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(ref pred) = self.predicate {
                if !pred(&node_ref) {
                    return false;
                }
            }

            true
        })
    }

    /// Count matching nodes.
    pub fn count(self) -> usize {
        self.collect().len()
    }
}

/// Extension trait for Node to add traversal methods.
pub trait NodeExt {
    /// Create a query starting from this node.
    fn query(&self) -> Query;

    /// Iterate over all descendant nodes (depth-first).
    fn iter_descendants(&self) -> DepthFirstIter;

    /// Iterate over all descendant nodes (breadth-first).
    fn iter_breadth_first(&self) -> BreadthFirstIter;

    /// Find all nodes of a specific element type.
    fn find_elements(&self, element: Element) -> Vec<Rc<RefCell<Node>>>;

    /// Find all nodes of a specific object type.
    fn find_objects(&self, object: Object) -> Vec<Rc<RefCell<Node>>>;

    /// Get all ancestors of this node (from parent to root).
    fn ancestors(&self) -> Vec<Rc<RefCell<Node>>>;

    /// Get the first ancestor of a specific element type.
    fn ancestor_element(&self, element: Element) -> Option<Rc<RefCell<Node>>>;
}

impl NodeExt for Rc<RefCell<Node>> {
    fn query(&self) -> Query {
        Query::new(self.clone())
    }

    fn iter_descendants(&self) -> DepthFirstIter {
        DepthFirstIter::new(self.clone())
    }

    fn iter_breadth_first(&self) -> BreadthFirstIter {
        BreadthFirstIter::new(self.clone())
    }

    fn find_elements(&self, element: Element) -> Vec<Rc<RefCell<Node>>> {
        self.query().filter_element(element).collect()
    }

    fn find_objects(&self, object: Object) -> Vec<Rc<RefCell<Node>>> {
        self.query().filter_object(object).collect()
    }

    fn ancestors(&self) -> Vec<Rc<RefCell<Node>>> {
        let mut result = Vec::new();
        let mut current = self.borrow().parent();

        while let Some(parent) = current {
            result.push(parent.clone());
            current = parent.borrow().parent();
        }

        result
    }

    fn ancestor_element(&self, element: Element) -> Option<Rc<RefCell<Node>>> {
        self.ancestors().into_iter().find(|node| {
            node.borrow()
                .as_element()
                .map(|e| *e == element)
                .unwrap_or(false)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::StandardProperties;

    #[test]
    fn test_depth_first_iter() {
        // Create a simple tree
        let root = Rc::new(RefCell::new(Node::element(
            Element::OrgData,
            StandardProperties::new(0, 100),
        )));

        let child1 = Rc::new(RefCell::new(Node::element(
            Element::Headline,
            StandardProperties::new(0, 50),
        )));

        let child2 = Rc::new(RefCell::new(Node::element(
            Element::Paragraph,
            StandardProperties::new(50, 100),
        )));

        root.borrow_mut().add_child(child1);
        root.borrow_mut().add_child(child2);

        let nodes: Vec<_> = DepthFirstIter::new(root).collect();
        assert_eq!(nodes.len(), 3); // root + 2 children
    }

    #[test]
    fn test_query_filter() {
        let root = Rc::new(RefCell::new(Node::element(
            Element::OrgData,
            StandardProperties::new(0, 100),
        )));

        let headline = Rc::new(RefCell::new(Node::element(
            Element::Headline,
            StandardProperties::new(0, 50),
        )));

        let paragraph = Rc::new(RefCell::new(Node::element(
            Element::Paragraph,
            StandardProperties::new(50, 100),
        )));

        root.borrow_mut().add_child(headline);
        root.borrow_mut().add_child(paragraph);

        // Query for headlines
        let headlines = root.query().filter_element(Element::Headline).collect();
        assert_eq!(headlines.len(), 1);
    }

    #[test]
    fn test_node_ext_methods() {
        let root = Rc::new(RefCell::new(Node::element(
            Element::OrgData,
            StandardProperties::new(0, 100),
        )));

        let child = Rc::new(RefCell::new(Node::element(
            Element::Headline,
            StandardProperties::new(0, 50),
        )));

        root.borrow_mut().add_child(child.clone());

        let headlines = root.find_elements(Element::Headline);
        assert_eq!(headlines.len(), 1);
    }
}
