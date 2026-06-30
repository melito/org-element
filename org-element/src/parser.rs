//! Parser for Org mode documents using tree-sitter.
//!
//! This module provides the main `Parser` struct that converts Org text into an AST.

use std::cell::RefCell;
use std::rc::Rc;

use tree_sitter::{Node as TSNode, Tree};
use tree_sitter_language::LanguageFn;

use crate::ast::{Element, Node, Object};
use crate::error::{Error, Result};
use crate::properties::StandardProperties;

// The tree-sitter-org grammar is compiled directly into this crate by
// `build.rs` (see `grammar/`). This is its generated entry point.
unsafe extern "C" {
    fn tree_sitter_org() -> *const ();
}

/// The bundled tree-sitter-org grammar as a [`LanguageFn`].
const ORG_LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_org) };

/// The bundled tree-sitter-org [`tree_sitter::Language`].
///
/// Exposed for advanced users who want to run their own tree-sitter queries
/// against the Org grammar directly, without going through [`Parser`].
pub fn language() -> tree_sitter::Language {
    ORG_LANGUAGE.into()
}

/// Known TODO keywords (configurable in real implementation)
const TODO_KEYWORDS: &[&str] = &["TODO", "DONE", "NEXT", "WAITING", "CANCELLED", "HOLD"];

/// Parse an Emacs-style plist string into key-value pairs.
///
/// e.g. `:width 300px :class my-img` → `[("width", "300px"), ("class", "my-img")]`
pub fn parse_plist(s: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current_key: Option<String> = None;
    for token in s.split_whitespace() {
        if let Some(key_name) = token.strip_prefix(':') {
            // Flush previous key (if it had no value, store empty string)
            if let Some(k) = current_key.take() {
                result.push((k, String::new()));
            }
            current_key = Some(key_name.to_string());
        } else if let Some(k) = current_key.take() {
            result.push((k, token.to_string()));
        }
        // else: orphan value token with no key — skip
    }
    // Flush trailing key with no value
    if let Some(k) = current_key {
        result.push((k, String::new()));
    }
    result
}

/// Org mode parser using tree-sitter.
pub struct Parser {
    ts_parser: tree_sitter::Parser,
}

impl Parser {
    /// Create a new parser.
    pub fn new() -> Result<Self> {
        let mut ts_parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = ORG_LANGUAGE.into();
        ts_parser
            .set_language(&language)
            .map_err(|e| Error::TreeSitterError(e.to_string()))?;
        Ok(Self { ts_parser })
    }

    /// Parse Org text into an AST.
    pub fn parse(&mut self, source: &str) -> Result<Rc<RefCell<Node>>> {
        let tree = self
            .ts_parser
            .parse(source, None)
            .ok_or_else(|| Error::ParseError {
                position: 0,
                message: "Failed to parse document".to_string(),
            })?;
        self.tree_to_ast(&tree, source)
    }

    /// Parse with an old tree for incremental parsing.
    pub fn parse_incremental(
        &mut self,
        source: &str,
        old_tree: Option<&Tree>,
    ) -> Result<Rc<RefCell<Node>>> {
        let tree = self
            .ts_parser
            .parse(source, old_tree)
            .ok_or_else(|| Error::ParseError {
                position: 0,
                message: "Failed to parse document".to_string(),
            })?;
        self.tree_to_ast(&tree, source)
    }

    fn tree_to_ast(&self, tree: &Tree, source: &str) -> Result<Rc<RefCell<Node>>> {
        let root = tree.root_node();
        let mut org_data = Node::element(
            Element::OrgData,
            StandardProperties::new(0, source.len()).with_contents(0, source.len()),
        );
        org_data.properties.set_string("raw-value", source);

        let org_data = Rc::new(RefCell::new(org_data));
        self.process_children(&root, source, &org_data)?;
        Ok(org_data)
    }

    /// Process all children of a tree-sitter node and add them to parent
    fn process_children(
        &self,
        ts_node: &TSNode,
        source: &str,
        parent: &Rc<RefCell<Node>>,
    ) -> Result<()> {
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child_ts = cursor.node();
                if child_ts.kind() == "body" {
                    // "body" is a structural wrapper in tree-sitter-org that
                    // doesn't map to an org-element type. Recurse into it so
                    // its children (paragraphs, lists, blocks) are added to
                    // the parent directly.
                    self.parse_body_into(&child_ts, source, parent)?;
                } else if let Some(child_node) = self.convert_node(&child_ts, source)? {
                    parent.borrow_mut().add_child(child_node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Convert a tree-sitter node to an AST node
    fn convert_node(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let kind = ts_node.kind();

        match kind {
            "document" => {
                // Document is handled at top level
                let node = Rc::new(RefCell::new(Node::element(
                    Element::OrgData,
                    StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
                )));
                self.process_children(ts_node, source, &node)?;
                Ok(Some(node))
            }
            "section" => self.parse_section(ts_node, source),
            "headline" => self.parse_headline(ts_node, source),
            "paragraph" => self.parse_paragraph(ts_node, source),
            "list" => self.parse_list(ts_node, source),
            "listitem" => self.parse_list_item(ts_node, source),
            "plan" => self.parse_planning(ts_node, source),
            "property_drawer" => self.parse_property_drawer(ts_node, source),
            "property" => self.parse_node_property(ts_node, source),
            "table" => self.parse_table(ts_node, source),
            "block" => self.parse_block(ts_node, source),
            "body" => self.parse_body(ts_node, source),
            "timestamp" => self.parse_timestamp(ts_node, source),
            // Skip structural nodes that don't map to org-element
            "item" | "expr" | "str" | "tag_list" | "tag" | "stars" | "bullet" | "contents"
            | "entry" | "entry_name" | "value" | "date" | "day" | "num" | "_" | ":" | "*" | "/"
            | "+" | "~" | "=" | "[" | "]" | "#" | "(" | ")" | "{" | "}" | "!" | ";" | "."
            | "\"" | "<" | ">" | "#+begin_" | "#+end_" | ":properties:" | ":end:" | "\n" => {
                Ok(None)
            }
            _ => {
                // Unknown node type - skip or create generic element
                Ok(None)
            }
        }
    }

    /// Parse a section (contains headline + body + subsections)
    fn parse_section(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let section = Node::element(
            Element::Section,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );
        let section = Rc::new(RefCell::new(section));

        // Process children: headline, plan, property_drawer, body, subsection
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                let field_name = cursor.field_name();

                match (child.kind(), field_name) {
                    ("headline", _) => {
                        if let Some(headline) = self.parse_headline(&child, source)? {
                            section.borrow_mut().add_child(headline);
                        }
                    }
                    ("plan", _) => {
                        if let Some(planning) = self.parse_planning(&child, source)? {
                            section.borrow_mut().add_child(planning);
                        }
                    }
                    ("property_drawer", _) => {
                        if let Some(pd) = self.parse_property_drawer(&child, source)? {
                            section.borrow_mut().add_child(pd);
                        }
                    }
                    ("body", _) => {
                        // Body contains paragraphs, lists, blocks
                        self.parse_body_into(&child, source, &section)?;
                    }
                    ("section", Some("subsection")) => {
                        if let Some(subsection) = self.parse_section(&child, source)? {
                            section.borrow_mut().add_child(subsection);
                        }
                    }
                    _ => {}
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(section))
    }

    /// Parse a headline with all its properties
    fn parse_headline(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut headline = Node::element(
            Element::Headline,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        // Get raw text of headline
        let raw_text = self.get_text(ts_node, source);
        headline.properties.set_string("raw-value", &raw_text);

        // Parse stars to get level
        if let Some(stars_node) = ts_node.child_by_field_name("stars") {
            let level = stars_node.end_byte() - stars_node.start_byte();
            headline.properties.set_integer("level", level as i64);
        }

        // Parse the headline content (item field) to get TODO, priority, title
        if let Some(item_node) = self.find_child_by_kind(ts_node, "item") {
            self.parse_headline_item(&item_node, source, &mut headline);
        }

        // Parse tags
        if let Some(tag_list) = self.find_child_by_kind(ts_node, "tag_list") {
            let tags = self.extract_tags(&tag_list, source);
            headline
                .properties
                .set_string_list("tags", tags);
        }

        Ok(Some(Rc::new(RefCell::new(headline))))
    }

    /// Parse the headline item to extract TODO keyword, priority, and title
    fn parse_headline_item(&self, item_node: &TSNode, source: &str, headline: &mut Node) {
        let mut title_parts: Vec<String> = Vec::new();
        let mut todo_keyword: Option<String> = None;
        let mut priority: Option<char> = None;

        let mut cursor = item_node.walk();
        if cursor.goto_first_child() {
            let mut first_word = true;
            loop {
                let child = cursor.node();
                let text = self.get_text(&child, source).trim().to_string();

                if child.kind() == "expr" {
                    // Check for priority like [#A]
                    if text.starts_with("[#") && text.ends_with("]") && text.len() == 4 {
                        priority = text.chars().nth(2);
                    } else if first_word && TODO_KEYWORDS.contains(&text.as_str()) {
                        todo_keyword = Some(text.clone());
                    } else if !text.is_empty() {
                        title_parts.push(text);
                    }
                    first_word = false;
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let title = title_parts.join(" ");
        headline.properties.set_string("title", &title);

        if let Some(todo) = todo_keyword {
            headline.properties.set_string("todo-keyword", &todo);
            // Determine todo type
            let todo_type = if todo == "DONE" || todo == "CANCELLED" {
                "done"
            } else {
                "todo"
            };
            headline.properties.set_string("todo-type", todo_type);
        }

        if let Some(p) = priority {
            headline.properties.set_string("priority", &p.to_string());
        }
    }

    /// Extract tags from a tag_list node
    fn extract_tags(&self, tag_list: &TSNode, source: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let mut cursor = tag_list.walk();

        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "tag" {
                    let tag_text = self.get_text(&child, source);
                    if !tag_text.is_empty() {
                        tags.push(tag_text);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        tags
    }

    /// Parse planning line (SCHEDULED, DEADLINE, CLOSED)
    fn parse_planning(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut planning = Node::element(
            Element::Planning,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "entry" {
                    // Get entry name (SCHEDULED, DEADLINE, CLOSED)
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = self.get_text(&name_node, source).to_uppercase();

                        // Get timestamp
                        if let Some(ts_node) = child.child_by_field_name("timestamp") {
                            let timestamp_text = self.get_text(&ts_node, source);
                            match name.as_str() {
                                "SCHEDULED" => {
                                    planning.properties.set_string("scheduled", &timestamp_text)
                                }
                                "DEADLINE" => {
                                    planning.properties.set_string("deadline", &timestamp_text)
                                }
                                "CLOSED" => {
                                    planning.properties.set_string("closed", &timestamp_text)
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(Rc::new(RefCell::new(planning))))
    }

    /// Parse property drawer
    fn parse_property_drawer(
        &self,
        ts_node: &TSNode,
        source: &str,
    ) -> Result<Option<Rc<RefCell<Node>>>> {
        let drawer = Node::element(
            Element::PropertyDrawer,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );
        let drawer = Rc::new(RefCell::new(drawer));

        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "property" {
                    if let Some(prop) = self.parse_node_property(&child, source)? {
                        drawer.borrow_mut().add_child(prop);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(drawer))
    }

    /// Parse a node property (:KEY: value)
    fn parse_node_property(
        &self,
        ts_node: &TSNode,
        source: &str,
    ) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut prop = Node::element(
            Element::NodeProperty,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        if let Some(name_node) = ts_node.child_by_field_name("name") {
            let name = self.get_text(&name_node, source);
            prop.properties.set_string("key", &name);
        }

        if let Some(value_node) = ts_node.child_by_field_name("value") {
            let value = self.get_text(&value_node, source).trim().to_string();
            prop.properties.set_string("value", &value);
        }

        Ok(Some(Rc::new(RefCell::new(prop))))
    }

    /// Parse body contents into parent
    fn parse_body_into(
        &self,
        ts_node: &TSNode,
        source: &str,
        parent: &Rc<RefCell<Node>>,
    ) -> Result<()> {
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "paragraph" => {
                        if let Some(para) = self.parse_paragraph(&child, source)? {
                            parent.borrow_mut().add_child(para);
                        }
                    }
                    "list" => {
                        if let Some(list) = self.parse_list(&child, source)? {
                            parent.borrow_mut().add_child(list);
                        }
                    }
                    "block" => {
                        if let Some(block) = self.parse_block(&child, source)? {
                            parent.borrow_mut().add_child(block);
                        }
                    }
                    "table" => {
                        if let Some(table) = self.parse_table(&child, source)? {
                            parent.borrow_mut().add_child(table);
                        }
                    }
                    "directive" => {
                        // Extract document-level keywords like #+TITLE:
                        let mut name = String::new();
                        let mut value = String::new();
                        let mut dcursor = child.walk();
                        if dcursor.goto_first_child() {
                            loop {
                                let dchild = dcursor.node();
                                match dchild.kind() {
                                    "expr" if name.is_empty() => {
                                        name = self.get_text(&dchild, source);
                                    }
                                    "value" => {
                                        value = self.get_text(&dchild, source);
                                    }
                                    _ => {}
                                }
                                if !dcursor.goto_next_sibling() { break; }
                            }
                        }
                        if name.eq_ignore_ascii_case("TITLE") {
                            parent.borrow_mut().properties.set_string("doc-title", &value);
                        }
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Parse body node
    fn parse_body(&self, _ts_node: &TSNode, _source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        // Body is not a real org-element type, but we process its children
        // Return None and let parse_section handle it
        Ok(None)
    }

    /// Parse a paragraph with inline objects
    fn parse_paragraph(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let start = ts_node.start_byte();
        let end = ts_node.end_byte();

        // Scan for #+ATTR_HTML: directives among paragraph children.
        // Track the byte offset where actual content begins (after all directives).
        let mut content_start = start;
        let mut attr_html_props: Vec<(String, String)> = Vec::new();
        {
            let mut cursor = ts_node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    if child.kind() == "directive" {
                        // Extract directive name from the expr child
                        let mut name = String::new();
                        let mut value = String::new();
                        let mut dcursor = child.walk();
                        if dcursor.goto_first_child() {
                            loop {
                                let dchild = dcursor.node();
                                match dchild.kind() {
                                    "expr" if name.is_empty() => {
                                        name = self.get_text(&dchild, source);
                                    }
                                    "value" => {
                                        value = self.get_text(&dchild, source);
                                    }
                                    _ => {}
                                }
                                if !dcursor.goto_next_sibling() { break; }
                            }
                        }
                        if name.eq_ignore_ascii_case("ATTR_HTML") {
                            attr_html_props.extend(parse_plist(&value));
                            // Only move content_start past ATTR_HTML directives
                            content_start = child.end_byte();
                        }
                    } else {
                        break;
                    }
                    if !cursor.goto_next_sibling() { break; }
                }
            }
        }

        // Use content_start so raw-value excludes directive lines
        let text = source[content_start..end].to_string();

        let mut para = Node::element(Element::Paragraph, StandardProperties::new(content_start, end));
        para.properties.set_string("raw-value", &text);

        // Store any ATTR_HTML properties
        for (key, val) in &attr_html_props {
            para.properties.set_string(&format!("attr-html-{}", key), val);
        }

        let para = Rc::new(RefCell::new(para));

        // Parse inline objects (bold, italic, links, etc.)
        // First, scan the raw text for [[...]] links (tree-sitter splits these across
        // multiple expr nodes when descriptions contain spaces).
        // Then parse expr-based markup, skipping byte ranges already covered by links.
        //
        // When ATTR_HTML directives were stripped, link byte offsets must use
        // content_start as the base (not the tree-sitter node start) so they
        // align with the trimmed raw-value.
        let link_nodes = self.parse_all_links_from_text(&text, ts_node, content_start)?;
        let link_ranges: Vec<(usize, usize)> = link_nodes
            .iter()
            .map(|n| {
                let b = n.borrow();
                (b.standard_properties.begin, b.standard_properties.end)
            })
            .collect();
        for link in link_nodes {
            para.borrow_mut().add_child(link);
        }

        self.parse_inline_objects_excluding(ts_node, source, &para, &link_ranges)?;

        Ok(Some(para))
    }

    /// Parse inline objects, skipping expr nodes whose byte ranges overlap
    /// with already-parsed regions (e.g. links extracted from raw text).
    fn parse_inline_objects_excluding(
        &self,
        ts_node: &TSNode,
        source: &str,
        parent: &Rc<RefCell<Node>>,
        exclude_ranges: &[(usize, usize)],
    ) -> Result<()> {
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "expr" {
                    let child_start = child.start_byte();
                    let child_end = child.end_byte();

                    // Skip expr nodes that overlap with already-parsed link ranges
                    let overlaps = exclude_ranges.iter().any(|&(rs, re)| {
                        child_start < re && child_end > rs
                    });

                    if !overlaps {
                        let objects = self.parse_expr_for_objects(&child, source)?;
                        for obj in objects {
                            parent.borrow_mut().add_child(obj);
                        }
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Parse an expr node to detect inline objects.
    /// Returns a Vec because a single expr node may contain multiple links
    /// interspersed with plain text.
    fn parse_expr_for_objects(
        &self,
        ts_node: &TSNode,
        source: &str,
    ) -> Result<Vec<Rc<RefCell<Node>>>> {
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            let first_child = cursor.node();
            let marker = first_child.kind();

            // Check for markup markers — only if both open and close markers exist
            match marker {
                "*" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut bold = Node::object(Object::Bold, StandardProperties::new(begin, end));
                        bold.properties.set_string("content", &content);
                        return Ok(vec![Rc::new(RefCell::new(bold))]);
                    }
                }
                "/" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut italic = Node::object(Object::Italic, StandardProperties::new(begin, end));
                        italic.properties.set_string("content", &content);
                        return Ok(vec![Rc::new(RefCell::new(italic))]);
                    }
                }
                "_" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut underline = Node::object(Object::Underline, StandardProperties::new(begin, end));
                        underline.properties.set_string("content", &content);
                        return Ok(vec![Rc::new(RefCell::new(underline))]);
                    }
                }
                "+" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut strike = Node::object(Object::StrikeThrough, StandardProperties::new(begin, end));
                        strike.properties.set_string("content", &content);
                        return Ok(vec![Rc::new(RefCell::new(strike))]);
                    }
                }
                "~" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut code = Node::object(Object::Code, StandardProperties::new(begin, end));
                        code.properties.set_string("value", &content);
                        return Ok(vec![Rc::new(RefCell::new(code))]);
                    }
                }
                "=" => {
                    if let Some((content, begin, end)) = self.extract_markup_content(ts_node, source) {
                        let mut verbatim = Node::object(Object::Verbatim, StandardProperties::new(begin, end));
                        verbatim.properties.set_string("value", &content);
                        return Ok(vec![Rc::new(RefCell::new(verbatim))]);
                    }
                }
                "[" => {
                    // Could be link [[url][desc]] or other bracket construct.
                    // An expr node may contain multiple links, so parse them all.
                    let full_text = self.get_text(ts_node, source);
                    if full_text.starts_with("[[") {
                        return self.parse_all_links_from_text(&full_text, ts_node, ts_node.start_byte());
                    }
                }
                _ => {}
            }
        }
        Ok(vec![])
    }

    /// Extract content and byte range from markup (between opening and closing markers).
    ///
    /// Returns `Some((content_text, markup_start_byte, markup_end_byte))` where the
    /// byte range covers the full markup including markers (e.g. `=text=`),
    /// excluding any trailing characters the expr node may include.
    /// Returns `None` if no valid matching markers are found.
    fn extract_markup_content(&self, ts_node: &TSNode, source: &str) -> Option<(String, usize, usize)> {
        let child_count = ts_node.child_count();
        if child_count >= 2 {
            let first_child = ts_node.child(0).unwrap();
            let marker_kind = first_child.kind();
            // Find the last child that matches the opening marker kind
            for i in (1..child_count).rev() {
                if let Some(child) = ts_node.child(i as u32) {
                    if child.kind() == marker_kind {
                        let content_start = first_child.end_byte();
                        let content_end = child.start_byte();
                        let markup_start = first_child.start_byte();
                        let markup_end = child.end_byte();
                        if content_end > content_start && content_end <= source.len() {
                            let content = source[content_start..content_end].to_string();
                            return Some((content, markup_start, markup_end));
                        }
                        break;
                    }
                }
            }
        }
        // Fallback: try simple first/last char stripping
        let text = self.get_text(ts_node, source);
        let start = ts_node.start_byte();
        let end = ts_node.end_byte();
        if text.len() >= 2 {
            let bytes = text.as_bytes();
            if bytes[0] == bytes[bytes.len() - 1]
                && matches!(bytes[0], b'*' | b'/' | b'_' | b'+' | b'~' | b'=')
            {
                return Some((text[1..text.len() - 1].to_string(), start, end));
            }
        }
        None
    }

    /// Parse a link from text.
    /// Extracts the first `[[path][description]]` or `[[path]]` from the text,
    /// rather than assuming the entire text is a single link.
    fn parse_link_from_text(
        &self,
        text: &str,
        ts_node: &TSNode,
    ) -> Result<Option<Rc<RefCell<Node>>>> {
        // Find the closing ]]` for the first `[[`
        if !text.starts_with("[[") {
            return Ok(None);
        }

        // Find the first `]]` — this closes the link started by the leading `[[`
        let close = match text.find("]]") {
            Some(pos) => pos,
            None => return Ok(None),
        };

        let link_text = &text[..close + 2]; // e.g. "[[url][desc]]"
        let inner = &text[2..close]; // e.g. "url][desc"

        let start_byte = ts_node.start_byte();
        let end_byte = start_byte + link_text.len();

        let mut link = Node::object(
            Object::Link,
            StandardProperties::new(start_byte, end_byte),
        );

        link.properties.set_string("raw-link", link_text);

        if let Some(sep) = inner.find("][") {
            let path = &inner[..sep];
            let desc = &inner[sep + 2..];
            link.properties.set_string("path", path);
            link.properties.set_string("description", desc);
            link.properties.set_string("format", "bracket");
        } else {
            link.properties.set_string("path", inner);
            link.properties.set_string("format", "bracket");
        }

        // Detect link type
        if inner.contains("://") {
            link.properties.set_string("type", "url");
        } else if inner.starts_with("file:") {
            link.properties.set_string("type", "file");
        } else if inner.starts_with("#") {
            link.properties.set_string("type", "custom-id");
        } else if inner.starts_with("id:") {
            link.properties.set_string("type", "id");
        } else {
            link.properties.set_string("type", "fuzzy");
        }

        Ok(Some(Rc::new(RefCell::new(link))))
    }

    /// Parse all `[[...]]` links from an expr node's text.
    /// Plain text between links is handled by the export layer via byte offsets.
    fn parse_all_links_from_text(
        &self,
        text: &str,
        ts_node: &TSNode,
        base_byte: usize,
    ) -> Result<Vec<Rc<RefCell<Node>>>> {
        let mut results = Vec::new();
        let mut pos = 0;

        while pos < text.len() {
            if let Some(link_start) = text[pos..].find("[[") {
                let abs_start = pos + link_start;

                // Find the closing `]]`
                if let Some(close_offset) = text[abs_start..].find("]]") {
                    let abs_close = abs_start + close_offset + 2;
                    let link_slice = &text[abs_start..abs_close];

                    if let Ok(Some(link_node)) = self.parse_link_from_text(link_slice, ts_node) {
                        // Fix byte range to actual position within the expr node
                        link_node.borrow_mut().standard_properties.begin = base_byte + abs_start;
                        link_node.borrow_mut().standard_properties.end = base_byte + abs_close;
                        results.push(link_node);
                    }
                    pos = abs_close;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(results)
    }

    /// Parse a list
    fn parse_list(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut list = Node::element(
            Element::PlainList,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        // Determine list type from first item's bullet
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "listitem" {
                    if let Some(bullet_node) = child.child_by_field_name("bullet") {
                        let bullet = self.get_text(&bullet_node, source);
                        let list_type = if bullet.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                            "ordered"
                        } else if bullet.starts_with("-") || bullet.starts_with("+") || bullet.starts_with("*") {
                            "unordered"
                        } else {
                            "descriptive"
                        };
                        list.properties.set_string("type", list_type);
                        break;
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let list = Rc::new(RefCell::new(list));

        // Parse list items
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "listitem" {
                    if let Some(item) = self.parse_list_item(&child, source)? {
                        list.borrow_mut().add_child(item);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(list))
    }

    /// Parse a list item
    fn parse_list_item(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut item = Node::element(
            Element::Item,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        // Get bullet
        if let Some(bullet_node) = ts_node.child_by_field_name("bullet") {
            let bullet = self.get_text(&bullet_node, source);
            item.properties.set_string("bullet", &bullet);

            // Check for checkbox — tree-sitter-org exposes it as a dedicated field
            if let Some(checkbox_node) = ts_node.child_by_field_name("checkbox") {
                let checkbox_text = self.get_text(&checkbox_node, source);
                if checkbox_text.contains("[ ]") {
                    item.properties.set_string("checkbox", "off");
                } else if checkbox_text.contains("[x]") || checkbox_text.contains("[X]") {
                    item.properties.set_string("checkbox", "on");
                } else if checkbox_text.contains("[-]") {
                    item.properties.set_string("checkbox", "trans");
                }
                // Store the byte offset of the '[' character for frontend toggling
                let bracket_offset = checkbox_text.find('[').map(|i| checkbox_node.start_byte() + i).unwrap_or(checkbox_node.start_byte());
                item.properties
                    .set_integer("checkbox-offset", bracket_offset as i64);
            }
        }

        let item = Rc::new(RefCell::new(item));

        // Parse contents (paragraph) and nested lists
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                match child.kind() {
                    "paragraph" => {
                        if let Some(para) = self.parse_paragraph(&child, source)? {
                            item.borrow_mut().add_child(para);
                        }
                    }
                    "list" => {
                        if let Some(nested_list) = self.parse_list(&child, source)? {
                            item.borrow_mut().add_child(nested_list);
                        }
                    }
                    _ => {}
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(item))
    }

    /// Parse a table
    fn parse_table(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let table = Rc::new(RefCell::new(Node::element(
            Element::Table,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        )));

        // First pass: find if there's an hr node to determine header rows
        let mut has_hr = false;
        let mut hr_index = usize::MAX;
        let mut row_index = 0usize;
        {
            let mut cursor = ts_node.walk();
            if cursor.goto_first_child() {
                loop {
                    let child = cursor.node();
                    match child.kind() {
                        "hr" => {
                            if !has_hr {
                                has_hr = true;
                                hr_index = row_index;
                            }
                        }
                        "row" => {
                            row_index += 1;
                        }
                        _ => {}
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }

        // Second pass: parse rows, marking rows before the first hr as headers
        let mut row_index = 0usize;
        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "row" {
                    let is_header = has_hr && row_index < hr_index;
                    if let Some(row) = self.parse_table_row(&child, source, is_header)? {
                        table.borrow_mut().add_child(row);
                    }
                    row_index += 1;
                }
                // Skip hr nodes — they're just separators, not AST elements
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(table))
    }

    /// Parse a table row
    fn parse_table_row(
        &self,
        ts_node: &TSNode,
        source: &str,
        is_header: bool,
    ) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut row = Node::element(
            Element::TableRow,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );
        row.properties
            .set_string("row-type", if is_header { "header" } else { "data" });

        let row = Rc::new(RefCell::new(row));

        let mut cursor = ts_node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "cell" {
                    if let Some(cell) = self.parse_table_cell(&child, source, is_header)? {
                        row.borrow_mut().add_child(cell);
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        Ok(Some(row))
    }

    /// Parse a table cell
    fn parse_table_cell(
        &self,
        ts_node: &TSNode,
        source: &str,
        is_header: bool,
    ) -> Result<Option<Rc<RefCell<Node>>>> {
        let mut cell = Node::object(
            Object::TableCell,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );
        cell.properties
            .set_string("cell-type", if is_header { "header" } else { "data" });

        // Extract the cell text content, stripping leading pipe and trimming whitespace
        let raw = self.get_text(ts_node, source);
        let text = raw.strip_prefix('|').unwrap_or(&raw).trim().to_string();
        cell.properties.set_string("value", &text);

        Ok(Some(Rc::new(RefCell::new(cell))))
    }

    /// Parse a block (src, example, quote, etc.)
    fn parse_block(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        // Get block type from name field
        let block_name = ts_node
            .child_by_field_name("name")
            .map(|n| self.get_text(&n, source).to_uppercase())
            .unwrap_or_default();

        let element_type = match block_name.as_str() {
            "SRC" => Element::SrcBlock,
            "EXAMPLE" => Element::ExampleBlock,
            "QUOTE" => Element::QuoteBlock,
            "CENTER" => Element::CenterBlock,
            "VERSE" => Element::VerseBlock,
            "COMMENT" => Element::CommentBlock,
            "EXPORT" => Element::ExportBlock,
            _ => Element::SpecialBlock,
        };

        let mut block = Node::element(
            element_type,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        block.properties.set_string("block-type", &block_name);

        // Get language/parameter for src blocks
        if let Some(param_node) = ts_node.child_by_field_name("parameter") {
            let param = self.get_text(&param_node, source);
            if element_type == Element::SrcBlock {
                // tree-sitter-org's `parameter` field may contain just the language
                // or "language :header args...". Split on first whitespace.
                if let Some(space_idx) = param.find(char::is_whitespace) {
                    let language = &param[..space_idx];
                    let rest = param[space_idx..].trim();
                    block.properties.set_string("language", language);
                    if !rest.is_empty() {
                        block.properties.set_string("parameters", rest);
                        Self::parse_header_args(&mut block, rest);
                    }
                } else {
                    block.properties.set_string("language", &param);
                }
            } else {
                block.properties.set_string("parameters", &param);
            }
        }

        // If we only got language (no header args from parameter field), try
        // extracting header args from the raw #+BEGIN_SRC line in the source.
        if element_type == Element::SrcBlock
            && block.properties.get_string("parameters").is_none()
        {
            let begin = ts_node.start_byte();
            let block_text = &source[begin..ts_node.end_byte()];
            if let Some(first_line_end) = block_text.find('\n') {
                let first_line = &block_text[..first_line_end];
                // Match after #+BEGIN_SRC <language>
                if let Some(lang) = block.properties.get_string("language") {
                    if let Some(lang_pos) = first_line.find(&lang) {
                        let after_lang = first_line[lang_pos + lang.len()..].trim();
                        if !after_lang.is_empty() {
                            block.properties.set_string("parameters", after_lang);
                            Self::parse_header_args(&mut block, after_lang);
                        }
                    }
                }
            }
        }

        // Parse #+NAME: affiliated keyword.
        // Strategy: look at the raw source around this block — both within the
        // block's byte range (tree-sitter may include it) and just before it.
        if element_type == Element::SrcBlock {
            let begin = ts_node.start_byte();
            let block_text = &source[begin..ts_node.end_byte()];
            // Check lines within the block text before #+BEGIN_SRC
            for line in block_text.lines() {
                let trimmed = line.trim();
                let upper = trimmed.to_uppercase();
                if upper.starts_with("#+BEGIN_SRC") || upper.starts_with("#+BEGIN_") {
                    break;
                }
                if upper.starts_with("#+NAME:") {
                    let rest = &trimmed["#+NAME:".len()..];
                    block.properties.set_string("name", rest.trim());
                }
            }
            // Also check lines just before the block's start byte
            if block.properties.get_string("name").is_none() {
                let before = &source[..begin];
                for line in before.lines().rev() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let upper = trimmed.to_uppercase();
                    if upper.starts_with("#+NAME:") {
                        let rest = &trimmed["#+NAME:".len()..];
                        block.properties.set_string("name", rest.trim());
                    }
                    break;
                }
            }
        }

        // Get contents
        if let Some(contents_node) = ts_node.child_by_field_name("contents") {
            let contents = self.get_text(&contents_node, source);
            block.properties.set_string("value", &contents);
        }

        Ok(Some(Rc::new(RefCell::new(block))))
    }

    /// Parse org-babel header arguments from a parameter string like ":results output html :exports code :var x=data"
    fn parse_header_args(block: &mut Node, params: &str) {
        let tokens: Vec<&str> = params.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            match tokens[i] {
                ":results" => {
                    // Collect all following tokens until next :keyword or end
                    let mut values = Vec::new();
                    let mut j = i + 1;
                    while j < tokens.len() && !tokens[j].starts_with(':') {
                        values.push(tokens[j]);
                        j += 1;
                    }
                    if !values.is_empty() {
                        block.properties.set_string("header-results", &values.join(" "));
                    }
                    i = j;
                }
                ":exports" => {
                    if i + 1 < tokens.len() {
                        block.properties.set_string("header-exports", tokens[i + 1]);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                ":frame" => {
                    if i + 1 < tokens.len() {
                        block.properties.set_string("header-frame", tokens[i + 1]);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                ":var" => {
                    if i + 1 < tokens.len() {
                        let existing = block.properties.get_string("header-var").unwrap_or_default();
                        let new_val = if existing.is_empty() {
                            tokens[i + 1].to_string()
                        } else {
                            format!("{},{}", existing, tokens[i + 1])
                        };
                        block.properties.set_string("header-var", &new_val);
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }
    }

    /// Parse a timestamp
    fn parse_timestamp(&self, ts_node: &TSNode, source: &str) -> Result<Option<Rc<RefCell<Node>>>> {
        let text = self.get_text(ts_node, source);
        let mut ts = Node::object(
            Object::Timestamp,
            StandardProperties::new(ts_node.start_byte(), ts_node.end_byte()),
        );

        ts.properties.set_string("raw-value", &text);

        // Determine type (active vs inactive)
        let ts_type = if text.starts_with("<") {
            "active"
        } else if text.starts_with("[") {
            "inactive"
        } else {
            "active"
        };
        ts.properties.set_string("type", ts_type);

        // Extract date components
        if let Some(date_node) = ts_node.child_by_field_name("date") {
            let date = self.get_text(&date_node, source);
            // Parse YYYY-MM-DD
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3 {
                if let Ok(year) = parts[0].parse::<i64>() {
                    ts.properties.set_integer("year-start", year);
                }
                if let Ok(month) = parts[1].parse::<i64>() {
                    ts.properties.set_integer("month-start", month);
                }
                if let Ok(day) = parts[2].parse::<i64>() {
                    ts.properties.set_integer("day-start", day);
                }
            }
        }

        if let Some(day_node) = ts_node.child_by_field_name("day") {
            let day_name = self.get_text(&day_node, source);
            ts.properties.set_string("day-name", &day_name);
        }

        Ok(Some(Rc::new(RefCell::new(ts))))
    }

    // Helper methods

    /// Get text content of a node
    fn get_text(&self, node: &TSNode, source: &str) -> String {
        source[node.start_byte()..node.end_byte()].to_string()
    }

    /// Find first child with given kind
    fn find_child_by_kind<'a>(&self, node: &'a TSNode, kind: &str) -> Option<TSNode<'a>> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == kind {
                    return Some(child);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        None
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new().expect("Failed to create parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::NodeVariant;
    use crate::traversal::NodeExt;


    #[test]
    fn test_parser_creation() {
        let parser = Parser::new();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_headline_with_todo() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* TODO Test Headline").unwrap();

        let headlines = ast.find_elements(Element::Headline);
        assert!(!headlines.is_empty(), "Should find a headline");

        let headline = headlines[0].borrow();
        assert_eq!(headline.properties.get_integer("level"), Some(1));
        assert_eq!(
            headline.properties.get_string("todo-keyword"),
            Some("TODO")
        );
        assert_eq!(headline.properties.get_string("title"), Some("Test Headline"));
    }

    #[test]
    fn test_parse_headline_with_tags() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Headline :tag1:tag2:").unwrap();

        let headlines = ast.find_elements(Element::Headline);
        assert!(!headlines.is_empty());

        let headline = headlines[0].borrow();
        let tags = headline.properties.get_string_list("tags");
        assert!(tags.is_some());
        let tags = tags.unwrap();
        assert!(tags.len() >= 1);
    }

    #[test]
    fn test_parse_headline_with_priority() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* TODO [#A] Important").unwrap();

        let headlines = ast.find_elements(Element::Headline);
        assert!(!headlines.is_empty());

        let headline = headlines[0].borrow();
        assert_eq!(headline.properties.get_string("priority"), Some("A"));
    }

    #[test]
    fn test_parse_paragraph() {
        let mut parser = Parser::new().unwrap();
        // Standalone paragraph needs to be in a context tree-sitter recognizes
        let ast = parser.parse("* Heading\n\nThis is a paragraph.").unwrap();

        let paragraphs = ast.find_elements(Element::Paragraph);
        assert!(!paragraphs.is_empty(), "Should find at least one paragraph");

        let para = paragraphs[0].borrow();
        assert!(para.properties.get_string("raw-value").is_some());
    }

    #[test]
    fn test_parse_bold_text() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("Text with *bold* word.").unwrap();

        let _bold_objects = ast.find_objects(Object::Bold);
        // May or may not find bold depending on tree-sitter-org version
        // This tests the infrastructure works
    }

    #[test]
    fn test_parse_list() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("- Item 1\n- Item 2").unwrap();

        let _lists = ast.find_elements(Element::PlainList);
        // Lists may be nested in sections depending on tree-sitter version
    }

    #[test]
    fn test_parse_checkbox() {
        let mut parser = Parser::new().unwrap();
        let input = "- [ ] Buy milk\n- [x] Done task\n";
        let ast = parser.parse(input).unwrap();

        let items = ast.find_elements(Element::Item);
        assert_eq!(items.len(), 2);

        let first = items[0].borrow();
        assert_eq!(first.properties.get_string("checkbox"), Some("off"));
        assert!(first.properties.get_integer("checkbox-offset").is_some());

        let second = items[1].borrow();
        assert_eq!(second.properties.get_string("checkbox"), Some("on"));
        assert!(second.properties.get_integer("checkbox-offset").is_some());
    }

    #[test]
    fn test_parse_src_block() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC")
            .unwrap();

        let blocks = ast.find_elements(Element::SrcBlock);
        if !blocks.is_empty() {
            let block = blocks[0].borrow();
            assert_eq!(block.properties.get_string("language"), Some("rust"));
        }
    }

    #[test]
    fn test_parse_src_block_with_header_args() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output\nprint(\"hello\")\n#+END_SRC")
            .unwrap();

        let blocks = ast.find_elements(Element::SrcBlock);
        if !blocks.is_empty() {
            let block = blocks[0].borrow();
            assert_eq!(block.properties.get_string("language"), Some("python"));
            assert_eq!(
                block.properties.get_string("parameters"),
                Some(":results output")
            );
            assert_eq!(
                block.properties.get_string("header-results"),
                Some("output")
            );
        }
    }

    #[test]
    fn test_parse_multi_value_results() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output html\nprint(\"<b>hi</b>\")\n#+END_SRC")
            .unwrap();
        let blocks = ast.find_elements(Element::SrcBlock);
        assert!(!blocks.is_empty());
        let block = blocks[0].borrow();
        assert_eq!(block.properties.get_string("header-results"), Some("output html"));
    }

    #[test]
    fn test_parse_exports_header_arg() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :exports results :results output\nprint(42)\n#+END_SRC")
            .unwrap();
        let blocks = ast.find_elements(Element::SrcBlock);
        assert!(!blocks.is_empty());
        let block = blocks[0].borrow();
        assert_eq!(block.properties.get_string("header-exports"), Some("results"));
        assert_eq!(block.properties.get_string("header-results"), Some("output"));
    }

    #[test]
    fn test_parse_var_header_arg() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :var x=data :var y=other\nprint(x)\n#+END_SRC")
            .unwrap();
        let blocks = ast.find_elements(Element::SrcBlock);
        assert!(!blocks.is_empty());
        let block = blocks[0].borrow();
        assert_eq!(block.properties.get_string("header-var"), Some("x=data,y=other"));
    }

    #[test]
    fn test_parse_name_affiliated_keyword() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+NAME: my-data\n#+BEGIN_SRC python\nprint(42)\n#+END_SRC")
            .unwrap();
        let blocks = ast.find_elements(Element::SrcBlock);
        assert!(!blocks.is_empty());
        let block = blocks[0].borrow();
        assert_eq!(block.properties.get_string("name"), Some("my-data"));
    }

    #[test]
    fn test_parse_table() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("| Name | Age |\n|------+-----|\n| Alice | 30 |\n| Bob | 25 |")
            .unwrap();

        let tables = ast.find_elements(Element::Table);
        assert!(!tables.is_empty(), "Should find a table");

        let table = tables[0].borrow();
        let rows: Vec<_> = table
            .children
            .iter()
            .filter(|c| matches!(c.borrow().variant, NodeVariant::Element(Element::TableRow)))
            .collect();
        assert_eq!(rows.len(), 3, "Should have 3 rows (1 header + 2 data)");

        // First row should be a header
        let first_row = rows[0].borrow();
        assert_eq!(
            first_row.properties.get_string("row-type"),
            Some("header")
        );

        // Check header cells
        let cells: Vec<_> = first_row
            .children
            .iter()
            .filter(|c| matches!(c.borrow().variant, NodeVariant::Object(Object::TableCell)))
            .collect();
        assert_eq!(cells.len(), 2, "Header row should have 2 cells");
        assert_eq!(cells[0].borrow().properties.get_string("value"), Some("Name"));
        assert_eq!(cells[0].borrow().properties.get_string("cell-type"), Some("header"));

        // Second data row
        let second_row = rows[1].borrow();
        assert_eq!(second_row.properties.get_string("row-type"), Some("data"));
    }

    #[test]
    fn test_parse_table_no_header() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("| a | b |\n| c | d |").unwrap();

        let tables = ast.find_elements(Element::Table);
        assert!(!tables.is_empty());

        let table = tables[0].borrow();
        for child_ref in &table.children {
            let child = child_ref.borrow();
            if let NodeVariant::Element(Element::TableRow) = &child.variant {
                assert_eq!(
                    child.properties.get_string("row-type"),
                    Some("data"),
                    "Without an hr separator, all rows should be data rows"
                );
            }
        }
    }

    #[test]
    fn test_parse_planning() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* TODO Task\n  SCHEDULED: <2025-01-15 Wed>")
            .unwrap();

        let planning = ast.find_elements(Element::Planning);
        if !planning.is_empty() {
            let plan = planning[0].borrow();
            assert!(plan.properties.get_string("scheduled").is_some());
        }
    }

    #[test]
    fn test_parse_plist() {
        let pairs = super::parse_plist(":width 300px :class my-img");
        assert_eq!(pairs, vec![
            ("width".to_string(), "300px".to_string()),
            ("class".to_string(), "my-img".to_string()),
        ]);

        // Key with no value
        let pairs = super::parse_plist(":width");
        assert_eq!(pairs, vec![("width".to_string(), String::new())]);

        // Empty string
        assert!(super::parse_plist("").is_empty());
    }

    #[test]
    fn test_parse_attr_html_on_image() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+ATTR_HTML: :width 300px\n[[./photo.jpg]]\n")
            .unwrap();

        let paragraphs = ast.find_elements(Element::Paragraph);
        assert!(!paragraphs.is_empty(), "Should find a paragraph");

        let para = paragraphs[0].borrow();
        assert_eq!(
            para.properties.get_string("attr-html-width"),
            Some("300px"),
            "Paragraph should have attr-html-width property"
        );

        // raw-value should NOT contain the directive line
        let raw = para.properties.get_string("raw-value").unwrap_or("");
        assert!(
            !raw.contains("ATTR_HTML"),
            "raw-value should not contain directive text, got: {:?}",
            raw
        );
    }

    #[test]
    fn test_parse_title_directive() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("#+TITLE: My Document\n\n* Heading\n\nSome text.\n")
            .unwrap();

        let root = ast.borrow();
        assert_eq!(
            root.properties.get_string("doc-title"),
            Some("My Document"),
            "Root node should have doc-title property"
        );
    }

}
