//! HTML export example for org-element-core.
//!
//! This demonstrates exporting Org documents to HTML.

use org_element::{HtmlExporter, Parser};

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

    println!("=== Basic HTML Export ===\n");

    // Export to HTML (without wrapper)
    let exporter = HtmlExporter::new();
    let html = exporter.export(&ast)?;
    println!("{}", html);

    println!("\n=== Full HTML Document ===\n");

    // Export with HTML document wrapper
    let exporter_wrapped = HtmlExporter::new().with_wrapper();
    let full_html = exporter_wrapped.export(&ast)?;
    println!("{}", full_html);

    println!("\n=== Custom CSS Prefix ===\n");

    // Export with custom CSS prefix
    let exporter_custom = HtmlExporter::new().with_class_prefix("my-org-");
    let custom_html = exporter_custom.export(&ast)?;
    println!("{}", custom_html);

    Ok(())
}
