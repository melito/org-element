# org-element

AST types and a [tree-sitter](https://tree-sitter.github.io/)-backed parser for
[Org mode](https://orgmode.org/) documents, compatible with Emacs
[`org-element.el`](https://orgmode.org/worg/dev/org-element-api.html).

## Features

- Complete coverage of all 26 Org element types
- Complete coverage of all 24 Org object types
- Incremental parsing via tree-sitter
- Iterator-based tree traversal
- AST modification and manipulation
- Export to HTML

## Example

```rust
use org_element::Parser;
use org_element::traversal::NodeExt;

fn main() -> org_element::Result<()> {
    let org_text = "* TODO Headline\n\nParagraph with *bold* text.";
    let mut parser = Parser::new()?;
    let ast = parser.parse(org_text)?;

    for node in ast.iter_descendants() {
        println!("{:?}", node.borrow().node_type());
    }
    Ok(())
}
```

## Cargo features

- `serde` *(default)* — derive `Serialize`/`Deserialize` for AST types.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
