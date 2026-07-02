//! Debug tool to see what tree-sitter-org produces

use tree_sitter::Parser;

fn main() {
    let mut parser = Parser::new();
    let language = org_element::language();
    parser.set_language(&language).unwrap();

    let source = r#"* TODO Test Headline :tag1:tag2:
  SCHEDULED: <2025-01-15 Wed>
  :PROPERTIES:
  :CUSTOM_ID: test
  :END:

  This is a paragraph with *bold* and /italic/ text.

  - First item
  - Second item
    - Nested item

** DONE Subheading [#A]

   #+BEGIN_SRC rust
   fn main() {
       println!("Hello!");
   }
   #+END_SRC
"#;

    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    println!("Source length: {} bytes", source.len());
    println!("Root: {} ({} children)", root.kind(), root.child_count());
    println!("\n=== Full Tree Structure ===\n");
    print_tree(&root, source, 0);
}

fn print_tree(node: &tree_sitter::Node, source: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let start = node.start_byte();
    let end = node.end_byte();

    // Get a snippet of the text
    let text_preview = if node.child_count() == 0 {
        let text = &source[start..end.min(start + 40)];
        let text = text.replace('\n', "↵").replace('\t', "→");
        format!(
            " \"{}\"",
            if text.len() > 37 {
                format!("{}...", &text[..37])
            } else {
                text
            }
        )
    } else {
        String::new()
    };

    // Get field name if this node is a field
    let field_info = node
        .parent()
        .and_then(|parent| {
            for i in 0..parent.child_count() {
                if let Some(child) = parent.child(i as u32) {
                    if child.id() == node.id() {
                        if let Some(name) = parent.field_name_for_child(i as u32) {
                            return Some(format!(" [field: {}]", name));
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_default();

    println!(
        "{}{} ({}..{}){}{}",
        indent,
        node.kind(),
        start,
        end,
        field_info,
        text_preview
    );

    // Print named children with their field names
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            print_tree(&child, source, depth + 1);
        }
    }
}
