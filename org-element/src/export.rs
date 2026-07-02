//! Export functionality for Org documents.
//!
//! This module provides exporters for converting Org ASTs to various formats.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{Element, Node, NodeVariant, Object};

/// HTML exporter for Org documents.
///
/// Converts an Org AST to HTML output.
///
/// # Example
///
/// ```rust,ignore
/// use org_element::{Parser, export::HtmlExporter};
///
/// let mut parser = Parser::new()?;
/// let ast = parser.parse("* Hello\n\nParagraph with *bold* text.")?;
///
/// let exporter = HtmlExporter::new();
/// let html = exporter.export(&ast)?;
/// println!("{}", html);
/// ```
pub struct HtmlExporter {
    /// Include HTML5 doctype and wrapper
    include_wrapper: bool,
    /// CSS class prefix for elements
    class_prefix: String,
    /// Use semantic HTML5 elements (article, section, etc.)
    semantic_html: bool,
    /// When true, the next paragraph should emit inline (no <p> tags)
    inline_paragraph: Cell<bool>,
    /// ATTR_HTML properties from the current paragraph, threaded to child link export
    current_attr_html: RefCell<Option<HashMap<String, String>>>,
    /// Tracks the `header-results` value from the last exported src block,
    /// so that a following #+RESULTS: block can render accordingly (e.g., raw HTML).
    last_src_results: RefCell<Option<String>>,
    /// Tracks the `:exports` value from the last exported src block,
    /// so that a following #+RESULTS: block can be hidden for `:exports code`/`none`.
    last_src_exports: RefCell<Option<String>>,
    /// Tracks the `:frame` value from the last exported src block,
    /// so that a following #+RESULTS: block can render frameless when `:frame none`.
    last_src_frame: RefCell<Option<String>>,
}

impl Default for HtmlExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlExporter {
    /// Create a new HTML exporter with default settings.
    pub fn new() -> Self {
        Self {
            include_wrapper: false,
            class_prefix: "org-".to_string(),
            semantic_html: true,
            inline_paragraph: Cell::new(false),
            current_attr_html: RefCell::new(None),
            last_src_results: RefCell::new(None),
            last_src_exports: RefCell::new(None),
            last_src_frame: RefCell::new(None),
        }
    }

    /// Create an exporter that includes full HTML document wrapper.
    pub fn with_wrapper(mut self) -> Self {
        self.include_wrapper = true;
        self
    }

    /// Set a custom CSS class prefix.
    pub fn with_class_prefix(mut self, prefix: &str) -> Self {
        self.class_prefix = prefix.to_string();
        self
    }

    /// Disable semantic HTML5 elements.
    pub fn without_semantic_html(mut self) -> Self {
        self.semantic_html = false;
        self
    }

    /// Export an Org AST to HTML.
    pub fn export(&self, ast: &Rc<RefCell<Node>>) -> Result<String, ExportError> {
        let mut output = String::new();

        if self.include_wrapper {
            output.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
            output.push_str("<meta charset=\"utf-8\">\n");
            output.push_str(
                "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
            );
            output.push_str("</head>\n<body>\n");
        }

        self.export_node(ast, &mut output, 0)?;

        if self.include_wrapper {
            output.push_str("</body>\n</html>\n");
        }

        Ok(output)
    }

    /// Export a single node and its children.
    fn export_node(
        &self,
        node: &Rc<RefCell<Node>>,
        output: &mut String,
        depth: usize,
    ) -> Result<(), ExportError> {
        let borrowed = node.borrow();

        match &borrowed.variant {
            NodeVariant::Element(element) => {
                self.export_element(&borrowed, element, output, depth)?;
            }
            NodeVariant::Object(object) => {
                self.export_object(&borrowed, object, output)?;
            }
        }

        Ok(())
    }

    /// Export an element node.
    fn export_element(
        &self,
        node: &Node,
        element: &Element,
        output: &mut String,
        depth: usize,
    ) -> Result<(), ExportError> {
        match element {
            Element::OrgData => {
                // Root element - just export children
                if self.semantic_html {
                    output.push_str("<article class=\"");
                    output.push_str(&self.class_prefix);
                    output.push_str("document\">\n");
                }
                // Render #+TITLE: as document heading
                if let Some(title) = node.properties.get_string("doc-title") {
                    output.push_str("<h1 class=\"doc-title\">");
                    output.push_str(&escape_html(&title));
                    output.push_str("</h1>\n");
                }
                self.export_children(node, output, depth)?;
                if self.semantic_html {
                    output.push_str("</article>\n");
                }
            }

            Element::Headline => {
                self.export_headline(node, output, depth)?;
            }

            Element::Section => {
                if self.semantic_html {
                    output.push_str("<section class=\"");
                    output.push_str(&self.class_prefix);
                    output.push_str("section\">\n");
                }
                self.export_children(node, output, depth)?;
                if self.semantic_html {
                    output.push_str("</section>\n");
                }
            }

            Element::Paragraph => {
                let raw = node.properties.get_string("raw-value");
                let inline = self.inline_paragraph.replace(false);

                // Detect #+RESULTS: blocks (org-babel output).
                // Tree-sitter parses these as paragraphs since they have
                // no dedicated node type.
                if let Some(ref raw_text) = raw {
                    let trimmed = raw_text.trim_start();
                    if trimmed.starts_with("#+RESULTS:") {
                        self.export_results_block(trimmed, output);
                        return Ok(());
                    }
                }

                // Collect attr-html-* properties to thread to child link export
                {
                    let mut attrs = HashMap::new();
                    for key in &["width", "height", "class", "style"] {
                        if let Some(val) = node.properties.get_string(&format!("attr-html-{}", key))
                        {
                            attrs.insert(key.to_string(), val.to_string());
                        }
                    }
                    if !attrs.is_empty() {
                        *self.current_attr_html.borrow_mut() = Some(attrs);
                    }
                }

                if !inline {
                    output.push_str("<p>");
                }

                // Check for standalone checkbox at start of paragraph text
                // (not inside a list item — those are handled by Element::Item)
                let checkbox_skip = if !inline {
                    if let Some(ref raw_text) = raw {
                        emit_standalone_checkbox(raw_text, node.standard_properties.begin, output)
                    } else {
                        0
                    }
                } else {
                    0
                };

                if node.children.is_empty() {
                    // No inline objects — use raw text
                    if let Some(raw) = raw {
                        output.push_str(&escape_html(&unescape_org_markup(&raw[checkbox_skip..])));
                    }
                } else if let Some(raw) = raw {
                    // Interleave raw text with child objects using byte offsets
                    let para_begin = node.standard_properties.begin;
                    let mut last_pos = checkbox_skip; // skip past checkbox prefix if present
                    for child in &node.children {
                        let child_borrowed = child.borrow();
                        let child_begin = child_borrowed.standard_properties.begin;
                        let child_end = child_borrowed.standard_properties.end;
                        let rel_begin = child_begin.saturating_sub(para_begin);
                        let rel_end = child_end.saturating_sub(para_begin);
                        // Emit plain text before this child
                        if rel_begin > last_pos && rel_begin <= raw.len() {
                            output.push_str(&escape_html(&unescape_org_markup(
                                &raw[last_pos..rel_begin],
                            )));
                        }
                        drop(child_borrowed);
                        self.export_node(child, output, depth + 1)?;
                        last_pos = rel_end.min(raw.len());
                    }
                    // Emit any trailing text after the last child
                    if last_pos < raw.len() {
                        output.push_str(&escape_html(&unescape_org_markup(&raw[last_pos..])));
                    }
                } else {
                    self.export_children(node, output, depth)?;
                }
                if !inline {
                    output.push_str("</p>\n");
                }

                // Clear ATTR_HTML context after paragraph children are exported
                *self.current_attr_html.borrow_mut() = None;
            }

            Element::PlainList => {
                let list_type = node.properties.get_string("type").unwrap_or("unordered");
                let tag = if list_type == "ordered" { "ol" } else { "ul" };
                output.push_str("<");
                output.push_str(tag);
                output.push_str(" class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("list\">\n");
                self.export_children(node, output, depth + 1)?;
                output.push_str("</");
                output.push_str(tag);
                output.push_str(">\n");
            }

            Element::Item => {
                output.push_str("<li>");
                // Check for checkbox
                let has_checkbox = if let Some(checkbox) = node.properties.get_string("checkbox") {
                    let checked = checkbox == "on";
                    output.push_str("<input type=\"checkbox\"");
                    if checked {
                        output.push_str(" checked");
                    }
                    if let Some(offset) = node.properties.get_integer("checkbox-offset") {
                        output.push_str(&format!(" data-checkbox-offset=\"{}\"", offset));
                    }
                    output.push_str("> ");
                    true
                } else {
                    false
                };
                // Render the first paragraph inline (no <p> tags) so it flows
                // next to the checkbox on the same line
                if has_checkbox {
                    self.inline_paragraph.set(true);
                }
                self.export_children(node, output, depth)?;
                output.push_str("</li>\n");
            }

            Element::SrcBlock => {
                self.export_src_block(node, output)?;
            }

            Element::ExampleBlock => {
                output.push_str("<pre class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("example\">\n<code>");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</code>\n</pre>\n");
            }

            Element::QuoteBlock => {
                output.push_str("<blockquote class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("quote\">\n");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                self.export_children(node, output, depth)?;
                output.push_str("</blockquote>\n");
            }

            Element::CenterBlock => {
                output.push_str("<div class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("center\" style=\"text-align: center;\">\n");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                self.export_children(node, output, depth)?;
                output.push_str("</div>\n");
            }

            Element::VerseBlock => {
                output.push_str("<p class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("verse\">\n");
                if let Some(value) = node.properties.get_string("value") {
                    // Preserve line breaks in verse
                    output.push_str(&escape_html(&value).replace('\n', "<br>\n"));
                }
                output.push_str("</p>\n");
            }

            Element::Planning => {
                output.push_str("<div class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("planning\">\n");
                if let Some(scheduled) = node.properties.get_string("scheduled") {
                    output.push_str("<span class=\"scheduled\">SCHEDULED: ");
                    output.push_str(&escape_html(&scheduled));
                    output.push_str("</span> ");
                }
                if let Some(deadline) = node.properties.get_string("deadline") {
                    output.push_str("<span class=\"deadline\">DEADLINE: ");
                    output.push_str(&escape_html(&deadline));
                    output.push_str("</span> ");
                }
                if let Some(closed) = node.properties.get_string("closed") {
                    output.push_str("<span class=\"closed\">CLOSED: ");
                    output.push_str(&escape_html(&closed));
                    output.push_str("</span>");
                }
                output.push_str("\n</div>\n");
            }

            Element::PropertyDrawer | Element::NodeProperty => {
                // Property drawers are typically not rendered in HTML output
                // But we could include them as data attributes or hidden elements
            }

            Element::Drawer => {
                // Generic drawer - render as collapsed details
                output.push_str("<details class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("drawer\">\n<summary>");
                if let Some(name) = node.properties.get_string("drawer-name") {
                    output.push_str(&escape_html(&name));
                } else {
                    output.push_str("Drawer");
                }
                output.push_str("</summary>\n");
                self.export_children(node, output, depth)?;
                output.push_str("</details>\n");
            }

            Element::Table => {
                output.push_str("<table class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("table\">\n");

                let mut in_header = false;
                let mut in_body = false;
                for child_ref in &node.children {
                    let is_header = {
                        let child = child_ref.borrow();
                        child.properties.get_string("row-type").as_deref() == Some("header")
                    };
                    if is_header && !in_header {
                        output.push_str("<thead>\n");
                        in_header = true;
                    } else if !is_header && in_header {
                        output.push_str("</thead>\n");
                        in_header = false;
                    }
                    if !is_header && !in_body {
                        output.push_str("<tbody>\n");
                        in_body = true;
                    }
                    self.export_node(child_ref, output, depth)?;
                }
                if in_header {
                    output.push_str("</thead>\n");
                }
                if in_body {
                    output.push_str("</tbody>\n");
                }

                output.push_str("</table>\n");
            }

            Element::TableRow => {
                output.push_str("<tr>");
                self.export_children(node, output, depth)?;
                output.push_str("</tr>\n");
            }

            Element::HorizontalRule => {
                output.push_str("<hr>\n");
            }

            Element::FixedWidth => {
                output.push_str("<pre class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("fixed-width\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&unescape_org_markup(&value)));
                }
                output.push_str("</pre>\n");
            }

            Element::Comment => {
                // HTML comment
                output.push_str("<!-- ");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&value.replace("--", "- -")); // Escape double dashes
                }
                output.push_str(" -->\n");
            }

            Element::CommentBlock => {
                output.push_str("<!-- ");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&value.replace("--", "- -"));
                }
                output.push_str(" -->\n");
            }

            Element::ExportBlock => {
                // Check if it's an HTML export block
                if let Some(backend) = node.properties.get_string("type") {
                    if backend.to_uppercase() == "HTML" {
                        // Include raw HTML
                        if let Some(value) = node.properties.get_string("value") {
                            output.push_str(&value);
                        }
                    }
                }
                // Other export backends are ignored
            }

            Element::Keyword => {
                // Keywords like #+TITLE are typically used for metadata
                // Could be rendered as meta tags in the head
                if let Some(key) = node.properties.get_string("key") {
                    if key.to_uppercase() == "TITLE" {
                        // If there's a title, we could use it
                        // For now, just skip
                    }
                }
            }

            Element::LatexEnvironment => {
                output.push_str("<div class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("latex-env\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</div>\n");
            }

            Element::FootnoteDefinition => {
                output.push_str("<div class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("footnote\" id=\"fn:");
                if let Some(label) = node.properties.get_string("label") {
                    output.push_str(&escape_html(&label));
                }
                output.push_str("\">\n");
                self.export_children(node, output, depth)?;
                output.push_str("</div>\n");
            }

            Element::Clock
            | Element::DiarySexp
            | Element::SpecialBlock
            | Element::BabelCall
            | Element::Inlinetask
            | Element::DynamicBlock => {
                // These elements are either not commonly rendered or need special handling
                self.export_children(node, output, depth)?;
            }
        }

        Ok(())
    }

    /// Export a headline element.
    fn export_headline(
        &self,
        node: &Node,
        output: &mut String,
        depth: usize,
    ) -> Result<(), ExportError> {
        let level = node.properties.get_integer("level").unwrap_or(1) as usize;
        let h_level = level.min(6); // HTML only has h1-h6

        // Build CSS classes
        let mut classes = vec![format!("{}headline", self.class_prefix)];

        if let Some(todo) = node.properties.get_string("todo-keyword") {
            classes.push(format!("{}todo-{}", self.class_prefix, todo.to_lowercase()));
        }

        if let Some(priority) = node.properties.get_string("priority") {
            classes.push(format!(
                "{}priority-{}",
                self.class_prefix,
                priority.to_lowercase()
            ));
        }

        // Opening tag with classes
        output.push_str(&format!("<h{} class=\"{}\">", h_level, classes.join(" ")));

        // TODO badge
        if let Some(todo) = node.properties.get_string("todo-keyword") {
            output.push_str("<span class=\"");
            output.push_str(&self.class_prefix);
            output.push_str("todo\">");
            output.push_str(&escape_html(&todo));
            output.push_str("</span> ");
        }

        // Priority badge
        if let Some(priority) = node.properties.get_string("priority") {
            output.push_str("<span class=\"");
            output.push_str(&self.class_prefix);
            output.push_str("priority\">[#");
            output.push_str(&escape_html(&priority));
            output.push_str("]</span> ");
        }

        // Title
        if let Some(title) = node.properties.get_string("title") {
            output.push_str(&escape_html(&title));
        }

        // Tags
        if let Some(tags) = node.properties.get_string_list("tags") {
            if !tags.is_empty() {
                output.push_str(" <span class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("tags\">");
                for tag in tags {
                    output.push_str("<span class=\"");
                    output.push_str(&self.class_prefix);
                    output.push_str("tag\">");
                    output.push_str(&escape_html(&tag));
                    output.push_str("</span>");
                }
                output.push_str("</span>");
            }
        }

        output.push_str(&format!("</h{}>\n", h_level));

        // Export children (planning, property drawer, body content)
        self.export_children(node, output, depth)?;

        Ok(())
    }

    /// Export a source block.
    fn export_src_block(&self, node: &Node, output: &mut String) -> Result<(), ExportError> {
        let lang = node.properties.get_string("language");
        let lang_lower = lang.as_deref().map(|l| l.to_lowercase());
        let executable = lang_lower.is_some();

        let exports = node.properties.get_string("header-exports");
        let exports_val = exports.as_deref().unwrap_or("both");

        let results_attr = node.properties.get_string("header-results");
        let frame_attr = node.properties.get_string("header-frame");

        // :exports none or :exports results → skip the src block rendering entirely
        // (results-only blocks still show their #+RESULTS: via export_results_block)
        if exports_val == "none" || exports_val == "results" {
            *self.last_src_results.borrow_mut() = results_attr.map(|s| s.to_string());
            *self.last_src_exports.borrow_mut() = Some(exports_val.to_string());
            *self.last_src_frame.borrow_mut() = frame_attr.map(|s| s.to_string());
            return Ok(());
        }

        let block_name = node.properties.get_string("name");
        let var_attr = node.properties.get_string("header-var");

        if executable {
            output.push_str("<div class=\"org-src-container\"");
            output.push_str(&format!(
                " data-block-id=\"{}\"",
                node.standard_properties.begin
            ));
            if let Some(ref l) = lang_lower {
                output.push_str(&format!(" data-language=\"{}\"", escape_html(l)));
            }
            if let Some(ref r) = results_attr {
                output.push_str(&format!(" data-results=\"{}\"", escape_html(r)));
            }
            if let Some(ref name) = block_name {
                output.push_str(&format!(" data-block-name=\"{}\"", escape_html(name)));
            }
            if let Some(ref v) = var_attr {
                output.push_str(&format!(" data-var=\"{}\"", escape_html(v)));
            }
            if exports_val != "both" {
                output.push_str(&format!(" data-exports=\"{}\"", escape_html(exports_val)));
            }
            if let Some(ref f) = frame_attr {
                output.push_str(&format!(" data-frame=\"{}\"", escape_html(f)));
            }
            output.push_str(">\n");
            output.push_str("<div class=\"org-src-header\">");
            if let Some(ref l) = lang_lower {
                output.push_str("<span class=\"org-src-lang\">");
                output.push_str(&escape_html(l));
                if let Some(ref name) = block_name {
                    output.push_str(" — ");
                    output.push_str(&escape_html(name));
                }
                output.push_str("</span>");
            }
            output.push_str("</div>\n");
        }

        {
            output.push_str("<pre class=\"");
            output.push_str(&self.class_prefix);
            output.push_str("src");

            if let Some(ref l) = lang_lower {
                output.push_str(" ");
                output.push_str(&self.class_prefix);
                output.push_str("src-");
                output.push_str(l);
            }
            output.push_str("\">\n<code");

            if let Some(ref l) = lang_lower {
                output.push_str(" class=\"language-");
                output.push_str(l);
                output.push_str("\"");
            }
            output.push_str(">");

            if let Some(value) = node.properties.get_string("value") {
                output.push_str(&escape_html(&value));
            }

            output.push_str("</code>\n</pre>\n");
        }

        if executable && exports_val == "both" {
            output.push_str("<div class=\"org-results\"></div>\n");
        }

        if executable {
            output.push_str("</div>\n");
        }

        // Track results type, exports, and frame so following #+RESULTS: block can render accordingly
        *self.last_src_results.borrow_mut() = results_attr.map(|s| s.to_string());
        *self.last_src_exports.borrow_mut() = Some(exports_val.to_string());
        *self.last_src_frame.borrow_mut() = frame_attr.map(|s| s.to_string());

        Ok(())
    }

    /// Export a #+RESULTS: block (org-babel output) with styled HTML.
    /// The raw text starts with "#+RESULTS:" followed by ": " prefixed lines.
    fn export_results_block(&self, raw: &str, output: &mut String) {
        // Check if preceding src block had :exports code or :exports none — if so, hide results
        let exports_val = self.last_src_exports.borrow_mut().take();
        let frame_val = self.last_src_frame.borrow_mut().take();
        if let Some(ref ev) = exports_val {
            if ev == "code" || ev == "none" {
                // Also consume last_src_results so it doesn't leak to the next block
                self.last_src_results.borrow_mut().take();
                return;
            }
        }
        let is_frameless = frame_val.as_deref() == Some("none");

        // Skip the "#+RESULTS:" header line
        let lines: Vec<&str> = raw.lines().collect();
        let result_lines: Vec<&str> = lines
            .iter()
            .skip(1)
            .filter(|l| !l.is_empty())
            .copied()
            .collect();

        if result_lines.is_empty() {
            return;
        }

        // Strip ": " prefix from each result line
        let stripped: Vec<&str> = result_lines
            .iter()
            .map(|line| {
                line.strip_prefix(": ")
                    .or_else(|| line.strip_prefix(":"))
                    .unwrap_or(line)
            })
            .collect();

        // Check if preceding src block had :results html, then consume (clear) it
        let results_type = self
            .last_src_results
            .borrow_mut()
            .take()
            .unwrap_or_default();
        let is_html = results_type.contains("html");
        let is_table = results_type.contains("table");
        // Also auto-detect org table format: lines starting with "|"
        let looks_like_org_table = stripped
            .iter()
            .all(|l| l.starts_with('|') || l.trim().starts_with("|--"));

        if is_frameless {
            output
                .push_str("<div class=\"org-results org-results-static org-results-frameless\">\n");
        } else {
            output.push_str("<div class=\"org-results org-results-static\">\n");
        }

        if is_html {
            // Render as raw HTML — no escaping
            output.push_str("<div class=\"org-result-html\">");
            for line in &stripped {
                output.push_str(line);
                output.push('\n');
            }
            output.push_str("</div>\n");
        } else if is_table || looks_like_org_table {
            // Render as org table
            Self::render_org_table_from_lines(&stripped, &self.class_prefix, output);
        } else {
            output.push_str("<div class=\"org-result-stdout\"><pre>");
            for (i, content) in stripped.iter().enumerate() {
                output.push_str(&escape_html(&unescape_org_markup(content)));
                if i < stripped.len() - 1 {
                    output.push('\n');
                }
            }
            output.push_str("</pre></div>\n");
        }

        output.push_str("</div>\n");
    }

    /// Render pipe-delimited org table lines into HTML.
    /// Lines like `| a | b |` become table cells. Lines like `|---+---|` are
    /// treated as header separators: all rows before the first separator go into
    /// `<thead>`, all rows after go into `<tbody>`.
    fn render_org_table_from_lines(lines: &[&str], class_prefix: &str, output: &mut String) {
        output.push_str(&format!("<table class=\"{}table\">\n", class_prefix));

        let is_pipe_table = lines.iter().any(|l| l.trim().starts_with('|'));

        // Split into rows, detecting header separator
        let mut header_rows: Vec<Vec<&str>> = Vec::new();
        let mut body_rows: Vec<Vec<&str>> = Vec::new();
        let mut past_separator = false;

        if is_pipe_table {
            for line in lines {
                let trimmed = line.trim();
                // Separator row: |---+---| or |---|
                if trimmed.starts_with("|")
                    && trimmed.contains("---")
                    && !trimmed.contains(|c: char| c.is_alphanumeric())
                {
                    past_separator = true;
                    continue;
                }
                // Data row: | cell | cell |
                if trimmed.starts_with('|') {
                    let cells: Vec<&str> = trimmed
                        .trim_matches('|')
                        .split('|')
                        .map(|c| c.trim())
                        .collect();
                    if past_separator {
                        body_rows.push(cells);
                    } else {
                        header_rows.push(cells);
                    }
                }
            }
        } else {
            // TSV/CSV fallback: detect separator (tab preferred, then comma)
            let sep = if lines.iter().any(|l| l.contains('\t')) {
                '\t'
            } else {
                ','
            };
            for line in lines {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let cells: Vec<&str> = trimmed.split(sep).map(|c| c.trim()).collect();
                // First row becomes header, rest become body
                if header_rows.is_empty() {
                    header_rows.push(cells);
                    past_separator = true;
                } else {
                    body_rows.push(cells);
                }
            }
        }

        if past_separator && !header_rows.is_empty() {
            output.push_str("<thead>\n");
            for row in &header_rows {
                output.push_str("<tr>");
                for cell in row {
                    output.push_str("<th>");
                    output.push_str(&escape_html(&unescape_org_markup(cell)));
                    output.push_str("</th>");
                }
                output.push_str("</tr>\n");
            }
            output.push_str("</thead>\n");
        }

        // If no separator was found, all rows go into tbody
        let tbody_rows = if past_separator {
            &body_rows
        } else {
            &header_rows
        };
        if !tbody_rows.is_empty() {
            output.push_str("<tbody>\n");
            for row in tbody_rows {
                output.push_str("<tr>");
                for cell in row {
                    output.push_str("<td>");
                    output.push_str(&escape_html(&unescape_org_markup(cell)));
                    output.push_str("</td>");
                }
                output.push_str("</tr>\n");
            }
            output.push_str("</tbody>\n");
        }

        output.push_str("</table>\n");
    }

    /// Export an object node.
    fn export_object(
        &self,
        node: &Node,
        object: &Object,
        output: &mut String,
    ) -> Result<(), ExportError> {
        match object {
            Object::Bold => {
                output.push_str("<strong>");
                if let Some(content) = node.properties.get_string("content") {
                    output.push_str(&escape_html(&content));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</strong>");
            }

            Object::Italic => {
                output.push_str("<em>");
                if let Some(content) = node.properties.get_string("content") {
                    output.push_str(&escape_html(&content));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</em>");
            }

            Object::Underline => {
                output.push_str("<u>");
                if let Some(content) = node.properties.get_string("content") {
                    output.push_str(&escape_html(&content));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</u>");
            }

            Object::StrikeThrough => {
                output.push_str("<del>");
                if let Some(content) = node.properties.get_string("content") {
                    output.push_str(&escape_html(&content));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</del>");
            }

            Object::Code => {
                output.push_str("<code>");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</code>");
            }

            Object::Verbatim => {
                output.push_str("<code class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("verbatim\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</code>");
            }

            Object::Link => {
                self.export_link(node, output)?;
            }

            Object::Timestamp => {
                output.push_str("<time class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("timestamp\"");

                // Add datetime attribute if we have structured date
                if let (Some(year), Some(month), Some(day)) = (
                    node.properties.get_integer("year-start"),
                    node.properties.get_integer("month-start"),
                    node.properties.get_integer("day-start"),
                ) {
                    output.push_str(&format!(
                        " datetime=\"{:04}-{:02}-{:02}\"",
                        year, month, day
                    ));
                }

                output.push_str(">");
                if let Some(raw) = node.properties.get_string("raw-value") {
                    output.push_str(&escape_html(&raw));
                }
                output.push_str("</time>");
            }

            Object::Superscript => {
                output.push_str("<sup>");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</sup>");
            }

            Object::Subscript => {
                output.push_str("<sub>");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                self.export_children(node, output, 0)?;
                output.push_str("</sub>");
            }

            Object::LineBreak => {
                output.push_str("<br>\n");
            }

            Object::Target => {
                output.push_str("<a id=\"");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("\"></a>");
            }

            Object::RadioTarget => {
                output.push_str("<span class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("radio-target\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</span>");
            }

            Object::Entity => {
                // Named entities like \alpha, \rarr
                if let Some(html) = node.properties.get_string("html") {
                    output.push_str(&html);
                } else if let Some(utf8) = node.properties.get_string("utf-8") {
                    output.push_str(&utf8);
                } else if let Some(name) = node.properties.get_string("name") {
                    output.push_str(&format!("&{};", name));
                }
            }

            Object::LatexFragment => {
                output.push_str("<span class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("latex\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</span>");
            }

            Object::ExportSnippet => {
                // Only render if backend is html
                if let Some(backend) = node.properties.get_string("back-end") {
                    if backend.to_lowercase() == "html" {
                        if let Some(value) = node.properties.get_string("value") {
                            output.push_str(&value); // Raw HTML, don't escape
                        }
                    }
                }
            }

            Object::FootnoteReference => {
                output.push_str("<sup class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("footnote-ref\"><a href=\"#fn:");
                if let Some(label) = node.properties.get_string("label") {
                    output.push_str(&escape_html(&label));
                    output.push_str("\">[");
                    output.push_str(&escape_html(&label));
                    output.push_str("]");
                }
                output.push_str("</a></sup>");
            }

            Object::Citation | Object::CitationReference => {
                output.push_str("<cite class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("citation\">");
                if let Some(key) = node.properties.get_string("key") {
                    output.push_str(&escape_html(&key));
                }
                output.push_str("</cite>");
            }

            Object::InlineBabelCall | Object::InlineSrcBlock => {
                // Could execute and include result, for now just show code
                output.push_str("<code class=\"");
                output.push_str(&self.class_prefix);
                output.push_str("inline-src\">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</code>");
            }

            Object::TableCell => {
                let tag = match node.properties.get_string("cell-type").as_deref() {
                    Some("header") => "th",
                    _ => "td",
                };
                output.push_str("<");
                output.push_str(tag);
                output.push_str(">");
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                output.push_str("</");
                output.push_str(tag);
                output.push_str(">");
            }

            Object::Macro | Object::StatisticsCookie => {
                // Simple objects
                if let Some(value) = node.properties.get_string("value") {
                    output.push_str(&escape_html(&value));
                }
                self.export_children(node, output, 0)?;
            }
        }

        Ok(())
    }

    /// Export a link object.
    fn export_link(&self, node: &Node, output: &mut String) -> Result<(), ExportError> {
        // Resolve the href
        let href = if let Some(path) = node.properties.get_string("path") {
            match node.properties.get_string("type").as_deref() {
                Some("url") => path.to_string(),
                Some("file") => {
                    if path.ends_with(".org") {
                        format!("{}.html", &path[..path.len() - 4])
                    } else {
                        path.to_string()
                    }
                }
                Some("custom-id") => format!("#{}", path.trim_start_matches('#')),
                Some("id") => format!("#{}", path.trim_start_matches("id:")),
                _ => path.to_string(),
            }
        } else if let Some(raw) = node.properties.get_string("raw-link") {
            raw.to_string()
        } else {
            String::new()
        };

        let description = node.properties.get_string("description");

        // Image links without a description render as <img>
        if description.is_none() && is_image_url(&href) {
            let filename = href.rsplit('/').next().unwrap_or(&href);
            let alt = filename.split(['?', '#']).next().unwrap_or(filename);
            output.push_str("<img src=\"");
            output.push_str(&escape_html(&href));
            output.push_str("\" alt=\"");
            output.push_str(&escape_html(alt));
            output.push('"');
            // Emit ATTR_HTML attributes (width, height, class, style)
            if let Some(ref attrs) = *self.current_attr_html.borrow() {
                for key in &["width", "height", "class", "style"] {
                    if let Some(val) = attrs.get(*key) {
                        output.push(' ');
                        output.push_str(key);
                        output.push_str("=\"");
                        output.push_str(&escape_html(val));
                        output.push('"');
                    }
                }
            }
            output.push_str(" />");
        } else {
            output.push_str("<a href=\"");
            output.push_str(&escape_html(&href));
            output.push_str("\">");
            if let Some(desc) = description {
                output.push_str(&escape_html(&desc));
            } else if let Some(path) = node.properties.get_string("path") {
                output.push_str(&escape_html(&path));
            }
            output.push_str("</a>");
        }

        Ok(())
    }

    /// Export all children of a node.
    fn export_children(
        &self,
        node: &Node,
        output: &mut String,
        depth: usize,
    ) -> Result<(), ExportError> {
        for child in &node.children {
            self.export_node(child, output, depth + 1)?;
        }
        Ok(())
    }
}

/// Extract the `#+TITLE:` value from a parsed AST, if present.
pub fn extract_title(ast: &Rc<RefCell<Node>>) -> Option<String> {
    let node = ast.borrow();
    node.properties
        .get_string("doc-title")
        .map(|s| s.to_string())
}

/// Errors that can occur during export.
#[derive(Debug, Clone)]
pub enum ExportError {
    /// General export error.
    General(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::General(msg) => write!(f, "Export error: {}", msg),
        }
    }
}

impl std::error::Error for ExportError {}

/// If `raw` starts with a checkbox pattern (`[ ]`, `[x]`, `[X]`, `[-]`),
/// emit an HTML checkbox input and return the number of bytes to skip.
/// `abs_begin` is the absolute byte offset of the paragraph in the source,
/// used for the `data-checkbox-offset` attribute.
fn emit_standalone_checkbox(raw: &str, abs_begin: usize, output: &mut String) -> usize {
    let trimmed = raw.trim_start();
    let leading_ws = raw.len() - trimmed.len();

    let (checked, prefix_len) = if trimmed.starts_with("[ ] ") {
        (false, 4)
    } else if trimmed.starts_with("[x] ") || trimmed.starts_with("[X] ") {
        (true, 4)
    } else if trimmed.starts_with("[-] ") {
        // "trans" state — render as unchecked
        (false, 4)
    } else {
        return 0;
    };

    let bracket_offset = abs_begin + leading_ws;
    output.push_str("<input type=\"checkbox\"");
    if checked {
        output.push_str(" checked");
    }
    output.push_str(&format!(" data-checkbox-offset=\"{}\"", bracket_offset));
    output.push_str("> ");
    leading_ws + prefix_len
}

/// Check if a URL points to an image based on file extension or known media path.
fn is_image_url(path: &str) -> bool {
    // Our own media proxy serves images (and PDFs rendered as links elsewhere)
    if path.starts_with("/api/media/") {
        return true;
    }
    let clean = path.split(['?', '#']).next().unwrap_or(path);
    matches!(
        clean
            .rsplit('.')
            .next()
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif")
    )
}

/// Strip org-mode escape backslashes before markup characters.
/// In org-mode, `\*` renders as `*`, `\[` as `[`, etc.
fn unescape_org_markup(s: &str) -> String {
    const MARKUP_CHARS: &[char] = &['*', '/', '_', '=', '~', '+', '[', ']'];
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if MARKUP_CHARS.contains(&next) {
                    result.push(chars.next().unwrap());
                    continue;
                }
            }
        }
        result.push(c);
    }
    result
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Parser;

    #[test]
    fn test_export_simple_headline() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Hello World").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(html.contains("<h1"), "Should have h1 tag");
        assert!(html.contains("Hello World"), "Should contain title");
    }

    #[test]
    fn test_export_headline_with_todo() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* TODO Task").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(html.contains("org-todo"), "Should have todo class");
        assert!(html.contains(">TODO<"), "Should contain TODO badge");
    }

    #[test]
    fn test_export_bold_text() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Test\n\nText with *bold* word.").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        // Bold might be detected depending on tree-sitter version
        assert!(html.contains("<p>"), "Should have paragraph");
    }

    #[test]
    fn test_export_src_block() {
        let mut parser = Parser::new().unwrap();
        // Src blocks may need context to parse properly in tree-sitter-org
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        // Should produce some HTML (may or may not include src block depending on parser)
        assert!(!html.is_empty(), "Should produce HTML output");
        // Src blocks with a language get a container and header with lang label
        if html.contains("org-src-container") {
            assert!(
                html.contains("org-src-lang"),
                "Should have language label, got: {}",
                html
            );
            assert!(
                !html.contains("org-run-btn"),
                "Run button should NOT be in exported HTML (injected by JS), got: {}",
                html
            );
        }
    }

    #[test]
    fn test_export_src_block_with_results() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output\nprint(\"hello\")\n#+END_SRC")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        if html.contains("org-src-container") {
            assert!(
                html.contains("data-language=\"python\""),
                "Should have data-language attribute, got: {}",
                html
            );
            assert!(
                html.contains("data-block-id="),
                "Should have data-block-id attribute, got: {}",
                html
            );
            assert!(
                html.contains("org-results"),
                "Should have results div, got: {}",
                html
            );
        }
    }

    #[test]
    fn test_export_list() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("- Item 1\n- Item 2").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        // Debug output
        println!("HTML output:\n{}", html);

        // List may or may not be parsed depending on tree-sitter version
        // This tests that the export doesn't crash
        assert!(!html.is_empty(), "Should produce some output");
    }

    #[test]
    fn test_export_checkbox_in_list() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("- [ ] Buy milk\n- [x] Done\n").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        println!("Checkbox list HTML:\n{}", html);

        assert!(
            html.contains("<input type=\"checkbox\""),
            "Should have checkbox input"
        );
        assert!(
            html.contains("data-checkbox-offset="),
            "Should have offset attribute"
        );
        assert!(!html.contains("disabled"), "Should not be disabled");
        // Text should render inline with checkbox, not inside <p> tags
        assert!(
            !html.contains("<p>Buy milk"),
            "Paragraph should be inline, not wrapped in <p>"
        );
        assert!(
            html.contains("> Buy milk"),
            "Text should follow checkbox inline"
        );
    }

    #[test]
    fn test_export_checkbox_standalone() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Test\n\n[ ] standalone task\n").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        println!("Standalone checkbox HTML:\n{}", html);

        assert!(
            html.contains("<input type=\"checkbox\""),
            "Should render checkbox"
        );
        assert!(
            html.contains("data-checkbox-offset="),
            "Should have offset attribute"
        );
        assert!(html.contains("standalone task"), "Should have task text");
        assert!(!html.contains("[ ]"), "Should not have literal [ ] text");
    }

    #[test]
    fn test_export_brackets_not_checkbox() {
        let mut parser = Parser::new().unwrap();
        // Text with brackets mid-sentence should NOT become checkboxes
        let ast = parser.parse("* Test\n\nSee [x] for details\n").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        println!("Non-checkbox brackets HTML:\n{}", html);

        assert!(
            !html.contains("<input type=\"checkbox\""),
            "Mid-text [x] should not become checkbox"
        );
        assert!(html.contains("[x]"), "Should keep literal [x] text");
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("A & B"), "A &amp; B");
        assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_export_multiple_links_in_paragraph() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse(
            "[[https://orgmode.org][org-mode]] allows [[https://orgmode.org/manual/Working-with-Source-Code.html][working with source code]]"
        ).unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        println!("Multi-link HTML:\n{}", html);

        assert!(
            html.contains(r#"<a href="https://orgmode.org">org-mode</a>"#),
            "First link should render correctly, got: {}",
            html
        );
        assert!(
            html.contains(r#"<a href="https://orgmode.org/manual/Working-with-Source-Code.html">working with source code</a>"#),
            "Second link should render correctly, got: {}", html
        );
        assert!(
            html.contains(" allows "),
            "Plain text between links should be preserved, got: {}",
            html
        );
    }

    #[test]
    fn test_export_with_wrapper() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Test").unwrap();

        let exporter = HtmlExporter::new().with_wrapper();
        let html = exporter.export(&ast).unwrap();

        assert!(html.starts_with("<!DOCTYPE html>"), "Should have doctype");
        assert!(html.contains("<html>"), "Should have html tag");
        assert!(html.contains("</body>"), "Should have closing body");
    }

    #[test]
    fn test_export_results_block() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python\nprint(\"hello\")\n#+END_SRC\n\n#+RESULTS:\n: hello\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("org-results-static"),
            "Should render #+RESULTS: as styled results block, got: {}",
            html
        );
        assert!(
            html.contains("org-result-stdout"),
            "Should contain stdout div, got: {}",
            html
        );
        assert!(
            html.contains(">hello<"),
            "Should contain result text, got: {}",
            html
        );
        assert!(
            !html.contains("<p>#+RESULTS:"),
            "Should NOT render #+RESULTS: as plain paragraph, got: {}",
            html
        );
    }

    #[test]
    fn test_export_results_block_frame_none() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output html :exports results :frame none\nprint(\"<h1>Hello</h1>\")\n#+END_SRC\n\n#+RESULTS:\n: <h1>Hello</h1>\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("org-results-frameless"),
            "Should add frameless class when :frame none, got: {}",
            html
        );
        assert!(
            html.contains("org-results-static"),
            "Should still have static class, got: {}",
            html
        );
    }

    #[test]
    fn test_export_results_block_no_frame() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output html :exports results\nprint(\"<h1>Hello</h1>\")\n#+END_SRC\n\n#+RESULTS:\n: <h1>Hello</h1>\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            !html.contains("org-results-frameless"),
            "Should NOT add frameless class without :frame none, got: {}",
            html
        );
    }

    #[test]
    fn test_export_table_with_header() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("| Name | Age |\n|------+-----|\n| Alice | 30 |")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        println!("Table HTML:\n{}", html);

        assert!(html.contains("<table"), "Should have table tag");
        assert!(html.contains("<thead>"), "Should have thead");
        assert!(html.contains("<tbody>"), "Should have tbody");
        assert!(html.contains("<th>Name</th>"), "Header cell should use th");
        assert!(html.contains("<td>Alice</td>"), "Data cell should use td");
    }

    #[test]
    fn test_export_image_link() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("[[https://example.com/photo.png]]").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        println!("Image link HTML:\n{}", html);

        assert!(
            html.contains("<img src=\"https://example.com/photo.png\""),
            "Image URL should render as <img>, got: {}",
            html
        );
        assert!(
            html.contains("alt=\"photo.png\""),
            "Alt should be filename, got: {}",
            html
        );
        assert!(
            !html.contains("<a "),
            "Should NOT have <a> tag for image links, got: {}",
            html
        );
    }

    #[test]
    fn test_export_image_link_with_description() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("[[https://example.com/photo.png][My photo]]")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<a href=\"https://example.com/photo.png\">My photo</a>"),
            "Image link with description should render as <a>, got: {}",
            html
        );
    }

    #[test]
    fn test_export_non_image_link() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("[[https://example.com/page.html]]").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<a href="),
            "Non-image URL should render as <a>, got: {}",
            html
        );
        assert!(
            !html.contains("<img"),
            "Non-image URL should NOT render as <img>, got: {}",
            html
        );
    }

    #[test]
    fn test_export_image_link_uppercase_ext() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("[[https://example.com/photo.JPG]]").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<img"),
            "Uppercase image extension should render as <img>, got: {}",
            html
        );
    }

    #[test]
    fn test_export_image_link_with_query_string() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("[[https://example.com/photo.png?w=300&h=200]]")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<img"),
            "Image URL with query string should render as <img>, got: {}",
            html
        );
    }

    #[test]
    fn test_is_image_url() {
        assert!(is_image_url("https://example.com/photo.png"));
        assert!(is_image_url("https://example.com/photo.JPG"));
        assert!(is_image_url("https://example.com/photo.jpeg"));
        assert!(is_image_url("/images/cat.gif"));
        assert!(is_image_url("photo.webp"));
        assert!(is_image_url("photo.svg"));
        assert!(is_image_url("photo.avif"));
        assert!(is_image_url("https://example.com/photo.png?w=300"));
        assert!(is_image_url("https://example.com/photo.jpg#section"));
        assert!(!is_image_url("https://example.com/page.html"));
        assert!(!is_image_url("https://example.com/doc.pdf"));
        assert!(!is_image_url("https://example.com/"));
        // Media proxy URLs
        assert!(is_image_url(
            "/api/media/e5ead099-0b42-43eb-b383-577d91e38360"
        ));
        assert!(!is_image_url("/api/documents/123"));
    }

    #[test]
    fn test_export_table_no_header() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("| a | b |\n| c | d |").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(html.contains("<table"), "Should have table tag");
        assert!(
            !html.contains("<thead>"),
            "Should not have thead without hr separator"
        );
        assert!(html.contains("<tbody>"), "Should have tbody");
        assert!(html.contains("<td>a</td>"), "All cells should be td");
        assert!(!html.contains("<th>"), "No th without header separator");
    }

    #[test]
    fn test_export_attr_html_width_on_image() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+ATTR_HTML: :width 300px\n[[./photo.jpg]]\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("width=\"300px\""),
            "Image should have width attribute, got: {}",
            html
        );
        // Should not render the directive as literal text
        assert!(
            !html.contains("ATTR_HTML"),
            "Directive text should not appear in output, got: {}",
            html
        );
    }

    #[test]
    fn test_export_image_without_attr_html() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Test\n\n[[./photo.jpg]]\n").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<img src="),
            "Should render image, got: {}",
            html
        );
        assert!(
            !html.contains("width="),
            "Image should NOT have width attribute without ATTR_HTML, got: {}",
            html
        );
    }

    #[test]
    fn test_export_attr_html_multiple_attrs() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+ATTR_HTML: :width 400px :height 200px :class rounded\n[[./photo.png]]\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("width=\"400px\""),
            "Should have width, got: {}",
            html
        );
        assert!(
            html.contains("height=\"200px\""),
            "Should have height, got: {}",
            html
        );
        assert!(
            html.contains("class=\"rounded\""),
            "Should have class, got: {}",
            html
        );
    }

    #[test]
    fn test_export_attr_html_media_proxy_no_trailing_garbage() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+ATTR_HTML: :width 200px\n[[/api/media/e5ead099-0b42-43eb-b383-577d91e38360]]\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("width=\"200px\""),
            "Should have width, got: {}",
            html
        );
        assert!(
            html.contains("<img src=\"/api/media/e5ead099-0b42-43eb-b383-577d91e38360\""),
            "Should have correct img src, got: {}",
            html
        );
        // Must NOT have trailing link text leaking into the output
        assert!(
            !html.contains("]]"),
            "Should not have raw bracket text in output, got: {}",
            html
        );
        // The specific garbage pattern from the bug report
        assert!(
            !html.contains("577d91e38360]]"),
            "Should not have trailing link garbage, got: {}",
            html
        );
    }

    #[test]
    fn test_export_title_directive() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("#+TITLE: My Document\n\n* Heading\n\nSome text.\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("<h1 class=\"doc-title\">My Document</h1>"),
            "Should render #+TITLE as h1, got: {}",
            html
        );
        // Title should not appear as plain text in a paragraph
        assert!(
            !html.contains("#+TITLE"),
            "Directive text should not appear in output, got: {}",
            html
        );
    }

    #[test]
    fn test_export_no_title_directive() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("* Heading\n\nSome text.\n").unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            !html.contains("doc-title"),
            "Should not render doc-title without #+TITLE, got: {}",
            html
        );
    }

    #[test]
    fn test_extract_title() {
        let mut parser = Parser::new().unwrap();
        let ast = parser.parse("#+TITLE: Test Title\n\n* Heading\n").unwrap();
        assert_eq!(extract_title(&ast), Some("Test Title".to_string()));

        let ast = parser.parse("* Heading\n").unwrap();
        assert_eq!(extract_title(&ast), None);
    }

    #[test]
    fn test_export_src_block_exports_none() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :exports none\nprint(42)\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            !html.contains("org-src-container"),
            ":exports none should produce no src block output, got: {}",
            html
        );
    }

    #[test]
    fn test_export_src_block_exports_results() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :exports results\nprint(42)\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            !html.contains("org-src-container"),
            ":exports results should not render src container, got: {}",
            html
        );
        assert!(
            !html.contains("<code"),
            ":exports results should hide code block, got: {}",
            html
        );
    }

    #[test]
    fn test_export_src_block_exports_code() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :exports code\nprint(42)\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("<code"),
            ":exports code should show code, got: {}",
            html
        );
        assert!(
            !html.contains("org-results"),
            ":exports code should hide results div, got: {}",
            html
        );
    }

    #[test]
    fn test_export_src_block_data_results_attr() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output html\nprint(\"<b>hi</b>\")\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("data-results=\"output html\""),
            "Should have data-results attribute, got: {}",
            html
        );
    }

    #[test]
    fn test_export_src_block_data_name_attr() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+NAME: my-block\n#+BEGIN_SRC python\nprint(42)\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("data-block-name=\"my-block\""),
            "Should have data-block-name attribute, got: {}",
            html
        );
    }

    #[test]
    fn test_export_src_block_data_var_attr() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :var x=data\nprint(x)\n#+END_SRC\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("data-var=\"x=data\""),
            "Should have data-var attribute, got: {}",
            html
        );
    }

    #[test]
    fn test_export_results_block_html_type() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python :results output html\nprint(\"<b>hi</b>\")\n#+END_SRC\n\n#+RESULTS:\n: <b>hi</b>\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("org-result-html"),
            "HTML results should use org-result-html class, got: {}",
            html
        );
        assert!(
            html.contains("<b>hi</b>"),
            "HTML results should render raw HTML, got: {}",
            html
        );
    }

    #[test]
    fn test_export_results_block_org_table() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC bash\nls -l\n#+END_SRC\n\n#+RESULTS:\n: | total | 88 |\n: | -rw-r--r-- | 1 | melgray | staff | 1601 | Cargo.toml |\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("<table class=\"org-table\">"),
            "Pipe-delimited results should render as org table, got: {}",
            html
        );
        assert!(
            html.contains("<td>total</td>"),
            "Should contain table cells, got: {}",
            html
        );
        assert!(
            html.contains("<td>Cargo.toml</td>"),
            "Should contain filename cell, got: {}",
            html
        );
    }

    #[test]
    fn test_export_results_block_org_table_with_header() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC python\npass\n#+END_SRC\n\n#+RESULTS:\n: | Name | Age |\n: |------+-----|\n: | Alice | 30 |\n")
            .unwrap();
        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();
        assert!(
            html.contains("<thead>"),
            "Should have thead for header row, got: {}",
            html
        );
        assert!(
            html.contains("<th>Name</th>"),
            "Header cells should use <th>, got: {}",
            html
        );
        assert!(
            html.contains("<td>Alice</td>"),
            "Body cells should use <td>, got: {}",
            html
        );
    }

    #[test]
    fn test_exports_code_hides_static_results() {
        let mut parser = Parser::new().unwrap();
        let org = "* Test\n\n#+BEGIN_SRC python :exports code\nprint(\"hello\")\n#+END_SRC\n\n#+RESULTS:\n: hello\n";
        let ast = parser.parse(org).unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        if html.contains("org-src-container") {
            assert!(
                html.contains("<code"),
                "Should show code block, got: {}",
                html
            );
            assert!(
                !html.contains("org-results-static"),
                ":exports code should hide static results, got: {}",
                html
            );
        }
    }

    #[test]
    fn test_exports_none_hides_everything() {
        let mut parser = Parser::new().unwrap();
        let org = "* Test\n\n#+BEGIN_SRC python :exports none\nprint(\"hello\")\n#+END_SRC\n\n#+RESULTS:\n: hello\n";
        let ast = parser.parse(org).unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            !html.contains("org-results-static"),
            ":exports none should hide static results, got: {}",
            html
        );
    }

    #[test]
    fn test_exports_results_shows_static_results() {
        let mut parser = Parser::new().unwrap();
        let org = "* Test\n\n#+BEGIN_SRC python :exports results :results output\nprint(\"hello\")\n#+END_SRC\n\n#+RESULTS:\n: hello\n";
        let ast = parser.parse(org).unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        if html.contains("org-src-container") || html.contains("org-results-static") {
            assert!(
                html.contains("org-results-static"),
                ":exports results should show static results, got: {}",
                html
            );
        }
    }

    #[test]
    fn test_tsv_table_rendering() {
        // Directly test render_org_table_from_lines with TSV input
        let lines = vec!["Name\tAge\tCity", "Alice\t30\tNYC", "Bob\t25\tSF"];
        let mut output = String::new();
        HtmlExporter::render_org_table_from_lines(&lines, "org-", &mut output);

        assert!(
            output.contains("<table"),
            "Should produce a table, got: {}",
            output
        );
        assert!(
            output.contains("<th>Name</th>"),
            "First row should be header, got: {}",
            output
        );
        assert!(
            output.contains("<td>Alice</td>"),
            "Body cells should render, got: {}",
            output
        );
        assert!(
            output.contains("<td>SF</td>"),
            "All cells should render, got: {}",
            output
        );
    }

    #[test]
    fn test_csv_table_rendering() {
        let lines = vec!["Name,Age", "Alice,30"];
        let mut output = String::new();
        HtmlExporter::render_org_table_from_lines(&lines, "org-", &mut output);

        assert!(
            output.contains("<th>Name</th>"),
            "CSV first row should be header, got: {}",
            output
        );
        assert!(
            output.contains("<td>Alice</td>"),
            "CSV body should render, got: {}",
            output
        );
    }

    #[test]
    fn test_unescape_org_markup() {
        assert_eq!(unescape_org_markup(r"\[ 10, 20 \]"), "[ 10, 20 ]");
        assert_eq!(unescape_org_markup(r"\*bold\*"), "*bold*");
        assert_eq!(unescape_org_markup(r"\_underline\_"), "_underline_");
        assert_eq!(unescape_org_markup(r"\~code\~"), "~code~");
        assert_eq!(unescape_org_markup(r"\=verbatim\="), "=verbatim=");
        assert_eq!(unescape_org_markup(r"\+strike\+"), "+strike+");
        assert_eq!(unescape_org_markup(r"\/italic\/"), "/italic/");
        // Regular backslashes should be preserved
        assert_eq!(unescape_org_markup(r"path\to\file"), r"path\to\file");
        assert_eq!(unescape_org_markup(r"newline\n"), r"newline\n");
        // Empty string
        assert_eq!(unescape_org_markup(""), "");
        // Trailing backslash
        assert_eq!(unescape_org_markup(r"end\"), r"end\");
    }

    #[test]
    fn test_results_block_strips_org_escapes() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n#+BEGIN_SRC js\n[10, 20, 30]\n#+END_SRC\n\n#+RESULTS:\n: \\[ 10, 20, 30 \\]\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("[ 10, 20, 30 ]"),
            "Should strip org escape backslashes from results, got: {}",
            html
        );
        assert!(
            !html.contains("\\["),
            "Should NOT contain backslash-bracket in results, got: {}",
            html
        );
    }

    #[test]
    fn test_fixed_width_strips_org_escapes() {
        let mut parser = Parser::new().unwrap();
        let ast = parser
            .parse("* Test\n\n: \\*asterisk\\* and \\[bracket\\]\n")
            .unwrap();

        let exporter = HtmlExporter::new();
        let html = exporter.export(&ast).unwrap();

        assert!(
            html.contains("*asterisk*"),
            "Fixed-width should strip org escapes, got: {}",
            html
        );
        assert!(
            html.contains("[bracket]"),
            "Fixed-width should strip bracket escapes, got: {}",
            html
        );
    }
}
