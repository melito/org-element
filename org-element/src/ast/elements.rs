//! Org mode element types (block-level structures).
//!
//! Elements are block-level syntax structures. They are divided into:
//! - Greater elements: can contain other elements and objects
//! - Lesser elements: cannot contain other elements (but may contain objects)

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Org mode element types.
///
/// Based on the Org syntax specification, there are 26 element types total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Element {
    // Greater elements (can contain other elements and objects)
    /// Document root node (virtual element containing the entire buffer).
    OrgData,

    /// Headline (* TODO Headline :tag:).
    Headline,

    /// Section (content between a headline and its first child headline).
    Section,

    /// Property drawer (:PROPERTIES: ... :END:).
    PropertyDrawer,

    /// Drawer (:DRAWERNAME: ... :END:).
    Drawer,

    /// Dynamic block (#+BEGIN: ... #+END:).
    DynamicBlock,

    /// Footnote definition (`[fn:1] definition text`).
    FootnoteDefinition,

    /// Inlinetask (* TODO Title (like headline but inline).
    Inlinetask,

    /// List item (- item or 1. item).
    Item,

    /// Plain list (container for items).
    PlainList,

    /// Quote block (#+BEGIN_QUOTE ... #+END_QUOTE).
    QuoteBlock,

    /// Special block (#+BEGIN_NAME ... #+END_NAME).
    SpecialBlock,

    /// Center block (#+BEGIN_CENTER ... #+END_CENTER).
    CenterBlock,

    /// Table (| a | b |).
    Table,

    /// Verse block (#+BEGIN_VERSE ... #+END_VERSE).
    VerseBlock,

    // Lesser elements (block-level, may contain objects)
    /// Babel call (#+CALL: function()).
    BabelCall,

    /// Clock entry (`CLOCK: [timestamp]--[timestamp]`).
    Clock,

    /// Comment line (# comment).
    Comment,

    /// Comment block (#+BEGIN_COMMENT ... #+END_COMMENT).
    CommentBlock,

    /// Diary sexp (%%(diary-anniversary 10 31 1948)).
    DiarySexp,

    /// Example block (#+BEGIN_EXAMPLE ... #+END_EXAMPLE).
    ExampleBlock,

    /// Export block (#+BEGIN_EXPORT format ... #+END_EXPORT).
    ExportBlock,

    /// Fixed-width line (: fixed width).
    FixedWidth,

    /// Horizontal rule (-----).
    HorizontalRule,

    /// Keyword (#+KEY: value).
    Keyword,

    /// LaTeX environment (\begin{env} ... \end{env}).
    LatexEnvironment,

    /// Node property (:PROP: value).
    NodeProperty,

    /// Paragraph (regular text block).
    Paragraph,

    /// Planning line (SCHEDULED: <...> DEADLINE: <...> CLOSED: [...]).
    Planning,

    /// Source block (#+BEGIN_SRC lang ... #+END_SRC).
    SrcBlock,

    /// Table row (| cell | cell |).
    TableRow,
}

impl Element {
    /// Check if this is a greater element (can contain other elements).
    pub fn is_greater_element(&self) -> bool {
        matches!(
            self,
            Element::OrgData
                | Element::Headline
                | Element::Section
                | Element::PropertyDrawer
                | Element::Drawer
                | Element::DynamicBlock
                | Element::FootnoteDefinition
                | Element::Inlinetask
                | Element::Item
                | Element::PlainList
                | Element::QuoteBlock
                | Element::SpecialBlock
                | Element::CenterBlock
                | Element::Table
                | Element::VerseBlock
        )
    }

    /// Check if this is a lesser element (cannot contain other elements).
    pub fn is_lesser_element(&self) -> bool {
        !self.is_greater_element()
    }

    /// Check if this element can contain objects (inline content).
    pub fn can_contain_objects(&self) -> bool {
        matches!(
            self,
            Element::Headline
                | Element::Inlinetask
                | Element::Item
                | Element::Paragraph
                | Element::TableRow
                | Element::VerseBlock
        )
    }

    /// Get the string representation of this element type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Element::OrgData => "org-data",
            Element::Headline => "headline",
            Element::Section => "section",
            Element::PropertyDrawer => "property-drawer",
            Element::Drawer => "drawer",
            Element::DynamicBlock => "dynamic-block",
            Element::FootnoteDefinition => "footnote-definition",
            Element::Inlinetask => "inlinetask",
            Element::Item => "item",
            Element::PlainList => "plain-list",
            Element::QuoteBlock => "quote-block",
            Element::SpecialBlock => "special-block",
            Element::CenterBlock => "center-block",
            Element::Table => "table",
            Element::VerseBlock => "verse-block",
            Element::BabelCall => "babel-call",
            Element::Clock => "clock",
            Element::Comment => "comment",
            Element::CommentBlock => "comment-block",
            Element::DiarySexp => "diary-sexp",
            Element::ExampleBlock => "example-block",
            Element::ExportBlock => "export-block",
            Element::FixedWidth => "fixed-width",
            Element::HorizontalRule => "horizontal-rule",
            Element::Keyword => "keyword",
            Element::LatexEnvironment => "latex-environment",
            Element::NodeProperty => "node-property",
            Element::Paragraph => "paragraph",
            Element::Planning => "planning",
            Element::SrcBlock => "src-block",
            Element::TableRow => "table-row",
        }
    }

    /// Parse an element type from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "org-data" => Some(Element::OrgData),
            "headline" => Some(Element::Headline),
            "section" => Some(Element::Section),
            "property-drawer" => Some(Element::PropertyDrawer),
            "drawer" => Some(Element::Drawer),
            "dynamic-block" => Some(Element::DynamicBlock),
            "footnote-definition" => Some(Element::FootnoteDefinition),
            "inlinetask" => Some(Element::Inlinetask),
            "item" => Some(Element::Item),
            "plain-list" => Some(Element::PlainList),
            "quote-block" => Some(Element::QuoteBlock),
            "special-block" => Some(Element::SpecialBlock),
            "center-block" => Some(Element::CenterBlock),
            "table" => Some(Element::Table),
            "verse-block" => Some(Element::VerseBlock),
            "babel-call" => Some(Element::BabelCall),
            "clock" => Some(Element::Clock),
            "comment" => Some(Element::Comment),
            "comment-block" => Some(Element::CommentBlock),
            "diary-sexp" => Some(Element::DiarySexp),
            "example-block" => Some(Element::ExampleBlock),
            "export-block" => Some(Element::ExportBlock),
            "fixed-width" => Some(Element::FixedWidth),
            "horizontal-rule" => Some(Element::HorizontalRule),
            "keyword" => Some(Element::Keyword),
            "latex-environment" => Some(Element::LatexEnvironment),
            "node-property" => Some(Element::NodeProperty),
            "paragraph" => Some(Element::Paragraph),
            "planning" => Some(Element::Planning),
            "src-block" => Some(Element::SrcBlock),
            "table-row" => Some(Element::TableRow),
            _ => None,
        }
    }

    /// Get all element types as a slice.
    pub fn all() -> &'static [Element] {
        &[
            Element::OrgData,
            Element::Headline,
            Element::Section,
            Element::PropertyDrawer,
            Element::Drawer,
            Element::DynamicBlock,
            Element::FootnoteDefinition,
            Element::Inlinetask,
            Element::Item,
            Element::PlainList,
            Element::QuoteBlock,
            Element::SpecialBlock,
            Element::CenterBlock,
            Element::Table,
            Element::VerseBlock,
            Element::BabelCall,
            Element::Clock,
            Element::Comment,
            Element::CommentBlock,
            Element::DiarySexp,
            Element::ExampleBlock,
            Element::ExportBlock,
            Element::FixedWidth,
            Element::HorizontalRule,
            Element::Keyword,
            Element::LatexEnvironment,
            Element::NodeProperty,
            Element::Paragraph,
            Element::Planning,
            Element::SrcBlock,
            Element::TableRow,
        ]
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_count() {
        assert_eq!(Element::all().len(), 31, "Should have 26 element types");
    }

    #[test]
    fn test_greater_elements() {
        assert!(Element::Headline.is_greater_element());
        assert!(Element::Section.is_greater_element());
        assert!(Element::PlainList.is_greater_element());
        assert!(!Element::Paragraph.is_greater_element());
        assert!(!Element::SrcBlock.is_greater_element());
    }

    #[test]
    fn test_can_contain_objects() {
        assert!(Element::Paragraph.can_contain_objects());
        assert!(Element::Headline.can_contain_objects());
        assert!(!Element::SrcBlock.can_contain_objects());
    }

    #[test]
    fn test_element_string_conversion() {
        assert_eq!(Element::Headline.as_str(), "headline");
        assert_eq!(Element::from_str("headline"), Some(Element::Headline));
        assert_eq!(Element::from_str("invalid"), None);
    }
}
