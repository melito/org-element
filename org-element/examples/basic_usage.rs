//! Basic usage example for org-element-core.
//!
//! This demonstrates parsing Org documents and extracting real properties.

use org_element::traversal::NodeExt;
use org_element::{Element, Object, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Sample Org document
    let org_text = r#"
* TODO Write Documentation :urgent:docs:
  SCHEDULED: <2025-01-15 Wed>
  :PROPERTIES:
  :CUSTOM_ID: write-docs
  :EFFORT: 2h
  :END:

  This is a paragraph with *bold* and /italic/ text.

  - First item
  - Second item
    - Nested item

** DONE Subheading [#A]

   Another paragraph here.

   #+BEGIN_SRC rust
   fn main() {
       println!("Hello, Org!");
   }
   #+END_SRC
"#;

    // Parse the document
    let mut parser = Parser::new()?;
    let ast = parser.parse(org_text)?;

    println!("=== Parsing Complete ===\n");

    // Find all headlines and show their properties
    let headlines = ast.find_elements(Element::Headline);
    println!("Headlines Found: {}\n", headlines.len());

    for (i, headline) in headlines.iter().enumerate() {
        let h = headline.borrow();
        println!("Headline {}:", i + 1);

        if let Some(level) = h.properties.get_integer("level") {
            println!("  Level: {}", level);
        }
        if let Some(todo) = h.properties.get_string("todo-keyword") {
            println!("  TODO: {}", todo);
        }
        if let Some(priority) = h.properties.get_string("priority") {
            println!("  Priority: {}", priority);
        }
        if let Some(title) = h.properties.get_string("title") {
            println!("  Title: {}", title);
        }
        if let Some(tags) = h.properties.get_string_list("tags") {
            if !tags.is_empty() {
                println!("  Tags: {:?}", tags);
            }
        }
        println!();
    }

    // Find planning elements
    let planning = ast.find_elements(Element::Planning);
    if !planning.is_empty() {
        println!("Planning Information:");
        for plan in &planning {
            let p = plan.borrow();
            if let Some(scheduled) = p.properties.get_string("scheduled") {
                println!("  SCHEDULED: {}", scheduled);
            }
            if let Some(deadline) = p.properties.get_string("deadline") {
                println!("  DEADLINE: {}", deadline);
            }
        }
        println!();
    }

    // Find source blocks
    let src_blocks = ast.find_elements(Element::SrcBlock);
    if !src_blocks.is_empty() {
        println!("Source Blocks:");
        for block in &src_blocks {
            let b = block.borrow();
            if let Some(lang) = b.properties.get_string("language") {
                println!("  Language: {}", lang);
            }
            if let Some(value) = b.properties.get_string("value") {
                println!("  Content ({} chars)", value.len());
            }
        }
        println!();
    }

    // Find inline objects (bold, italic)
    let bold_objects = ast.find_objects(Object::Bold);
    let italic_objects = ast.find_objects(Object::Italic);

    println!("Inline Objects:");
    println!("  Bold: {} found", bold_objects.len());
    for bold in &bold_objects {
        let b = bold.borrow();
        if let Some(content) = b.properties.get_string("content") {
            println!("    - \"{}\"", content);
        }
    }
    println!("  Italic: {} found", italic_objects.len());
    for italic in &italic_objects {
        let i = italic.borrow();
        if let Some(content) = i.properties.get_string("content") {
            println!("    - \"{}\"", content);
        }
    }
    println!();

    // Find lists
    let lists = ast.find_elements(Element::PlainList);
    println!("Lists: {} found", lists.len());
    for list in &lists {
        let l = list.borrow();
        if let Some(list_type) = l.properties.get_string("type") {
            println!("  Type: {}", list_type);
        }
    }
    println!();

    // Count items
    let items = ast.find_elements(Element::Item);
    println!("List Items: {} total", items.len());

    // Show full document statistics
    let paragraphs = ast.find_elements(Element::Paragraph);
    let sections = ast.find_elements(Element::Section);

    println!("\n=== Document Statistics ===");
    println!("  Sections: {}", sections.len());
    println!("  Headlines: {}", headlines.len());
    println!("  Paragraphs: {}", paragraphs.len());
    println!("  Lists: {}", lists.len());
    println!("  List Items: {}", items.len());
    println!("  Source Blocks: {}", src_blocks.len());
    println!("  Bold Objects: {}", bold_objects.len());
    println!("  Italic Objects: {}", italic_objects.len());

    Ok(())
}
