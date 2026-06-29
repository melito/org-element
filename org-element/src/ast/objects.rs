//! Org mode object types (inline content).
//!
//! Objects are inline syntax structures that can appear within elements.
//! Some objects are "recursive" and can contain other objects.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Org mode object types.
///
/// Based on the Org syntax specification, there are 24 object types total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Object {
    // Text markup (recursive - can contain other objects)
    /// Bold text (*bold*).
    Bold,

    /// Italic text (/italic/).
    Italic,

    /// Underline (_underline_).
    Underline,

    /// Strike-through (+strike+).
    StrikeThrough,

    /// Inline code (~code~).
    Code,

    /// Verbatim text (=verbatim=).
    Verbatim,

    // Links and references (some recursive)
    /// Link ([[url][description]]).
    Link,

    /// Footnote reference ([fn:1] or [fn:: inline definition]).
    FootnoteReference,

    /// Radio target (<<<target>>>).
    RadioTarget,

    /// Target (<<target>>).
    Target,

    // Citations (recursive)
    /// Citation ([cite:@key]).
    Citation,

    /// Citation reference (part of a citation).
    CitationReference,

    // Code execution
    /// Inline Babel call (call_function()).
    InlineBabelCall,

    /// Inline source block (src_lang{code}).
    InlineSrcBlock,

    // Math and special characters
    /// LaTeX fragment ($...$, \(...\), etc.).
    LatexFragment,

    /// Entity (\alpha, \nbsp, etc.).
    Entity,

    /// Subscript (x_{sub}).
    Subscript,

    /// Superscript (x^{super}).
    Superscript,

    // Other inline elements
    /// Export snippet (@@format:text@@).
    ExportSnippet,

    /// Line break (\\).
    LineBreak,

    /// Macro ({{{name(args)}}}).
    Macro,

    /// Statistics cookie ([1/2] or [50%]).
    StatisticsCookie,

    /// Table cell (part of a table row).
    TableCell,

    /// Timestamp (<2025-01-15 Wed>).
    Timestamp,
}

impl Object {
    /// Check if this object type is recursive (can contain other objects).
    pub fn is_recursive(&self) -> bool {
        matches!(
            self,
            Object::Bold
                | Object::Italic
                | Object::Underline
                | Object::StrikeThrough
                | Object::Link
                | Object::FootnoteReference
                | Object::RadioTarget
                | Object::Citation
                | Object::Subscript
                | Object::Superscript
                | Object::TableCell
        )
    }

    /// Check if this is a markup object (emphasis).
    pub fn is_markup(&self) -> bool {
        matches!(
            self,
            Object::Bold
                | Object::Italic
                | Object::Underline
                | Object::StrikeThrough
                | Object::Code
                | Object::Verbatim
        )
    }

    /// Check if this is a minimal markup type (for restricted contexts).
    pub fn is_minimal_markup(&self) -> bool {
        matches!(
            self,
            Object::Bold
                | Object::Code
                | Object::Entity
                | Object::Italic
                | Object::LatexFragment
                | Object::StrikeThrough
                | Object::Subscript
                | Object::Superscript
                | Object::Underline
                | Object::Verbatim
        )
    }

    /// Get the string representation of this object type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Object::Bold => "bold",
            Object::Italic => "italic",
            Object::Underline => "underline",
            Object::StrikeThrough => "strike-through",
            Object::Code => "code",
            Object::Verbatim => "verbatim",
            Object::Link => "link",
            Object::FootnoteReference => "footnote-reference",
            Object::RadioTarget => "radio-target",
            Object::Target => "target",
            Object::Citation => "citation",
            Object::CitationReference => "citation-reference",
            Object::InlineBabelCall => "inline-babel-call",
            Object::InlineSrcBlock => "inline-src-block",
            Object::LatexFragment => "latex-fragment",
            Object::Entity => "entity",
            Object::Subscript => "subscript",
            Object::Superscript => "superscript",
            Object::ExportSnippet => "export-snippet",
            Object::LineBreak => "line-break",
            Object::Macro => "macro",
            Object::StatisticsCookie => "statistics-cookie",
            Object::TableCell => "table-cell",
            Object::Timestamp => "timestamp",
        }
    }

    /// Parse an object type from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "bold" => Some(Object::Bold),
            "italic" => Some(Object::Italic),
            "underline" => Some(Object::Underline),
            "strike-through" => Some(Object::StrikeThrough),
            "code" => Some(Object::Code),
            "verbatim" => Some(Object::Verbatim),
            "link" => Some(Object::Link),
            "footnote-reference" => Some(Object::FootnoteReference),
            "radio-target" => Some(Object::RadioTarget),
            "target" => Some(Object::Target),
            "citation" => Some(Object::Citation),
            "citation-reference" => Some(Object::CitationReference),
            "inline-babel-call" => Some(Object::InlineBabelCall),
            "inline-src-block" => Some(Object::InlineSrcBlock),
            "latex-fragment" => Some(Object::LatexFragment),
            "entity" => Some(Object::Entity),
            "subscript" => Some(Object::Subscript),
            "superscript" => Some(Object::Superscript),
            "export-snippet" => Some(Object::ExportSnippet),
            "line-break" => Some(Object::LineBreak),
            "macro" => Some(Object::Macro),
            "statistics-cookie" => Some(Object::StatisticsCookie),
            "table-cell" => Some(Object::TableCell),
            "timestamp" => Some(Object::Timestamp),
            _ => None,
        }
    }

    /// Get all object types as a slice.
    pub fn all() -> &'static [Object] {
        &[
            Object::Bold,
            Object::Italic,
            Object::Underline,
            Object::StrikeThrough,
            Object::Code,
            Object::Verbatim,
            Object::Link,
            Object::FootnoteReference,
            Object::RadioTarget,
            Object::Target,
            Object::Citation,
            Object::CitationReference,
            Object::InlineBabelCall,
            Object::InlineSrcBlock,
            Object::LatexFragment,
            Object::Entity,
            Object::Subscript,
            Object::Superscript,
            Object::ExportSnippet,
            Object::LineBreak,
            Object::Macro,
            Object::StatisticsCookie,
            Object::TableCell,
            Object::Timestamp,
        ]
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_count() {
        assert_eq!(Object::all().len(), 24, "Should have 24 object types");
    }

    #[test]
    fn test_recursive_objects() {
        assert!(Object::Bold.is_recursive());
        assert!(Object::Link.is_recursive());
        assert!(!Object::Code.is_recursive());
        assert!(!Object::LineBreak.is_recursive());
    }

    #[test]
    fn test_markup_objects() {
        assert!(Object::Bold.is_markup());
        assert!(Object::Code.is_markup());
        assert!(!Object::Link.is_markup());
        assert!(!Object::Timestamp.is_markup());
    }

    #[test]
    fn test_object_string_conversion() {
        assert_eq!(Object::Bold.as_str(), "bold");
        assert_eq!(Object::from_str("bold"), Some(Object::Bold));
        assert_eq!(Object::from_str("invalid"), None);
    }
}
