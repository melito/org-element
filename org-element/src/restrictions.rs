//! Object restriction rules for Org syntax.
//!
//! Not all objects can appear in all contexts. This module defines which
//! objects are allowed within each element and object type.

use crate::ast::{Element, Object};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};

/// Type alias for a set of allowed objects.
pub type ObjectSet = HashSet<Object>;

/// Global map of object restrictions per element/object type.
static OBJECT_RESTRICTIONS: Lazy<HashMap<&'static str, ObjectSet>> = Lazy::new(|| {
    let mut map = HashMap::new();

    // Standard set - most inline content
    let standard = HashSet::from([
        Object::Bold,
        Object::Code,
        Object::Entity,
        Object::ExportSnippet,
        Object::FootnoteReference,
        Object::InlineBabelCall,
        Object::InlineSrcBlock,
        Object::Italic,
        Object::LineBreak,
        Object::LatexFragment,
        Object::Link,
        Object::Macro,
        Object::RadioTarget,
        Object::StatisticsCookie,
        Object::StrikeThrough,
        Object::Subscript,
        Object::Superscript,
        Object::Target,
        Object::Timestamp,
        Object::Underline,
        Object::Verbatim,
    ]);

    // Minimal set - basic markup only
    let minimal = HashSet::from([
        Object::Bold,
        Object::Code,
        Object::Entity,
        Object::Italic,
        Object::LatexFragment,
        Object::StrikeThrough,
        Object::Subscript,
        Object::Superscript,
        Object::Underline,
        Object::Verbatim,
    ]);

    // Citations set - extends standard with citations
    let mut citations = standard.clone();
    citations.insert(Object::Citation);

    // Table cell set - limited markup plus specific objects
    let table_cell = HashSet::from([
        Object::Citation,
        Object::CitationReference,
        Object::Entity,
        Object::ExportSnippet,
        Object::FootnoteReference,
        Object::LatexFragment,
        Object::Link,
        Object::Macro,
        Object::RadioTarget,
        Object::StatisticsCookie,
        Object::Subscript,
        Object::Superscript,
        Object::Target,
        Object::Timestamp,
    ]);

    // Element restrictions
    map.insert("headline", minimal.clone());
    map.insert("inlinetask", minimal.clone());
    map.insert("item", standard.clone());
    map.insert("keyword", citations.clone());
    map.insert("paragraph", citations.clone());
    map.insert("table-row", table_cell.clone());
    map.insert("verse-block", citations.clone());

    // Object restrictions (for recursive objects)
    map.insert("bold", standard.clone());
    map.insert("footnote-reference", standard.clone());
    map.insert("italic", standard.clone());
    map.insert("link", {
        // Links cannot contain other links, radio targets, or line breaks
        let mut link_set = standard.clone();
        link_set.remove(&Object::Link);
        link_set.remove(&Object::RadioTarget);
        link_set.remove(&Object::LineBreak);
        link_set
    });
    map.insert("radio-target", minimal.clone());
    map.insert("strike-through", standard.clone());
    map.insert("subscript", {
        // Subscript/superscript cannot contain sub/superscripts or line breaks
        let mut sub_set = standard.clone();
        sub_set.remove(&Object::Subscript);
        sub_set.remove(&Object::Superscript);
        sub_set.remove(&Object::LineBreak);
        sub_set
    });
    map.insert("superscript", {
        let mut sup_set = standard.clone();
        sup_set.remove(&Object::Subscript);
        sup_set.remove(&Object::Superscript);
        sup_set.remove(&Object::LineBreak);
        sup_set
    });
    map.insert("table-cell", table_cell);
    map.insert("underline", standard.clone());

    // Citation can only contain citation-reference
    map.insert("citation", HashSet::from([Object::CitationReference]));

    map
});

/// Get the allowed objects for a given element type.
pub fn allowed_objects_for_element(element: Element) -> Option<&'static ObjectSet> {
    OBJECT_RESTRICTIONS.get(element.as_str())
}

/// Get the allowed objects for a given object type (if it's recursive).
pub fn allowed_objects_for_object(object: Object) -> Option<&'static ObjectSet> {
    OBJECT_RESTRICTIONS.get(object.as_str())
}

/// Check if an object is allowed within an element.
pub fn is_object_allowed_in_element(object: Object, element: Element) -> bool {
    allowed_objects_for_element(element)
        .map(|set| set.contains(&object))
        .unwrap_or(false)
}

/// Check if an object is allowed within another object.
pub fn is_object_allowed_in_object(child: Object, parent: Object) -> bool {
    allowed_objects_for_object(parent)
        .map(|set| set.contains(&child))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph_restrictions() {
        let allowed = allowed_objects_for_element(Element::Paragraph).unwrap();
        assert!(allowed.contains(&Object::Bold));
        assert!(allowed.contains(&Object::Link));
        assert!(allowed.contains(&Object::Citation));
    }

    #[test]
    fn test_headline_restrictions() {
        // Headlines only allow minimal markup (no line breaks)
        let allowed = allowed_objects_for_element(Element::Headline).unwrap();
        assert!(allowed.contains(&Object::Bold));
        assert!(!allowed.contains(&Object::LineBreak));
        assert!(!allowed.contains(&Object::Link));
    }

    #[test]
    fn test_link_restrictions() {
        // Links cannot contain other links
        let allowed = allowed_objects_for_object(Object::Link).unwrap();
        assert!(allowed.contains(&Object::Bold));
        assert!(!allowed.contains(&Object::Link));
        assert!(!allowed.contains(&Object::RadioTarget));
        assert!(!allowed.contains(&Object::LineBreak));
    }

    #[test]
    fn test_citation_restrictions() {
        // Citations can only contain citation-reference
        let allowed = allowed_objects_for_object(Object::Citation).unwrap();
        assert!(allowed.contains(&Object::CitationReference));
        assert_eq!(allowed.len(), 1);
    }

    #[test]
    fn test_table_cell_restrictions() {
        let allowed = allowed_objects_for_element(Element::TableRow).unwrap();
        assert!(allowed.contains(&Object::Link));
        assert!(allowed.contains(&Object::Timestamp));
        assert!(!allowed.contains(&Object::Bold)); // No text markup in table cells
    }

    #[test]
    fn test_is_allowed_functions() {
        assert!(is_object_allowed_in_element(
            Object::Bold,
            Element::Paragraph
        ));
        assert!(!is_object_allowed_in_element(
            Object::LineBreak,
            Element::Headline
        ));
        assert!(!is_object_allowed_in_object(Object::Link, Object::Link));
    }
}
