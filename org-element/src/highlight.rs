//! Syntax highlighting: classify Org source into flat, non-overlapping
//! `(start_byte, end_byte, capture)` spans for an editor overlay.
//!
//! This is the "classification" half of highlighting (à la Emacs font-lock): it
//! says *what* each byte range is, never how to color it. The consumer maps each
//! capture name to a CSS class / face and applies a theme.
//!
//! ## One highlighter, two sources
//!
//! [`TsHighlighter`] is the single highlighter. Structure comes from a
//! tree-sitter `highlights.scm` query run over the authoritative parse — the
//! same [`Tree`](tree_sitter::Tree) the AST/preview is built from (see
//! [`highlight_tree`](TsHighlighter::highlight_tree)). Inline emphasis
//! (`*bold*`, `/italic/`, …) and timestamps are then filled into the gaps the
//! query left, by a small byte scanner: the tree-sitter-org grammar has no
//! emphasis nodes, so a query alone cannot see them.
//!
//! Note on wasm: `Query::new` recurses deeply and needs a roomy stack. On wasm
//! with the default 1 MiB stack it can trap (`ts_query__analyze_patterns` →
//! "memory access out of bounds") when invoked deep in a caller's call stack.
//! Link the wasm binary with a larger stack (e.g. `-zstack-size=4194304`).

use crate::error::Result;
use tree_sitter::Tree;

/// One classified span of source: `[start, end)` bytes and its capture name
/// (e.g. `"comment"`, `"markup.heading"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlight {
    /// Start byte offset (inclusive) into the source.
    pub start: usize,
    /// End byte offset (exclusive) into the source.
    pub end: usize,
    /// The capture name for this span (e.g. `"comment"`, `"keyword.directive"`).
    pub capture: String,
}

/// Tree-sitter-query-based highlighter — the authoritative parse. Compiles the
/// bundled `highlights.scm` once and reuses it. See the module docs re: wasm
/// stack size.
pub struct TsHighlighter {
    parser: tree_sitter::Parser,
    query: tree_sitter::Query,
}

impl TsHighlighter {
    /// Build with the bundled Org grammar + highlights query. Fails if the query
    /// doesn't compile (or, on a too-small wasm stack, may TRAP — see module docs).
    pub fn new() -> Result<Self> {
        use crate::error::Error;
        let language = crate::parser::language();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| Error::TreeSitterError(e.to_string()))?;
        let query = tree_sitter::Query::new(&language, include_str!("highlights.scm"))
            .map_err(|e| Error::TreeSitterError(format!("highlights query: {e}")))?;
        Ok(Self { parser, query })
    }

    /// Classify `source` into flat, non-overlapping spans, parsing internally.
    ///
    /// Prefer [`highlight_tree`](Self::highlight_tree) when you already have a
    /// [`Tree`] (e.g. the one the preview/AST was built from) — it avoids a
    /// second parse of the same text.
    pub fn highlight(&mut self, source: &str) -> Result<Vec<Highlight>> {
        use crate::error::Error;
        let Some(tree) = self.parser.parse(source, None) else {
            return Err(Error::ParseError {
                position: 0,
                message: "highlight parse failed".into(),
            });
        };
        self.highlight_tree(&tree, source)
    }

    /// Classify `source` into flat, non-overlapping spans from an
    /// already-parsed [`Tree`] (later/narrower captures win over earlier/wider
    /// ones, so specific tokens override containers). Sorted by start byte.
    ///
    /// This is the single-parse entry point: pass the same [`Tree`] used to
    /// build the AST/preview and no re-parse happens. Inline emphasis and
    /// timestamps — which the grammar has no nodes for — are scanned into the
    /// gaps the structural query leaves.
    ///
    /// `source` must be the exact text `tree` was parsed from.
    pub fn highlight_tree(&self, tree: &Tree, source: &str) -> Result<Vec<Highlight>> {
        use tree_sitter::{QueryCursor, StreamingIterator};

        let names = self.query.capture_names();

        // Raw captures with a priority = match order (later patterns win ties).
        struct Raw {
            start: usize,
            end: usize,
            cap: usize,
            priority: usize,
        }
        let mut raws: Vec<Raw> = Vec::new();
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), source.as_bytes());
        let mut order = 0usize;
        while let Some(m) = matches.next() {
            for cap in m.captures {
                let node = cap.node;
                raws.push(Raw {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    cap: cap.index as usize,
                    priority: order,
                });
                order += 1;
            }
        }

        // Per-byte owner: narrower span wins; equal width → later match wins.
        let n = source.len();
        let mut owner: Vec<Option<usize>> = vec![None; n];
        for (i, r) in raws.iter().enumerate() {
            let width = r.end.saturating_sub(r.start);
            for b in r.start..r.end.min(n) {
                let take = match owner[b] {
                    None => true,
                    Some(j) => {
                        let ow = raws[j].end.saturating_sub(raws[j].start);
                        width < ow || (width == ow && r.priority >= raws[j].priority)
                    }
                };
                if take {
                    owner[b] = Some(i);
                }
            }
        }

        // Coalesce contiguous bytes with the same owning capture into spans.
        let mut out: Vec<Highlight> = Vec::new();
        let mut b = 0;
        while b < n {
            let Some(idx) = owner[b] else {
                b += 1;
                continue;
            };
            let cap = raws[idx].cap;
            let start = b;
            b += 1;
            while b < n && owner[b] == Some(idx) {
                b += 1;
            }
            out.push(Highlight {
                start,
                end: b,
                capture: names[cap].to_string(),
            });
        }

        // The grammar has no emphasis/timestamp nodes, so the query can't see
        // them. Scan them from the source and drop into the gaps the structural
        // query left untouched (`owner[b] == None`) — never recoloring inside a
        // heading, block body, comment, etc. that the query already owns.
        fill_inline_gaps(&mut out, source, &owner);
        Ok(out)
    }
}

/// Scan inline emphasis (`*b*`, `/i/`, `_u_`, `+s+`, `~c~`, `=v=`) and Org
/// timestamps, appending any that fall entirely within bytes the structural
/// query did not claim, then re-sort the combined spans by start byte.
///
/// `owner[b].is_some()` means byte `b` is already classified by the query; we
/// only add an inline span when *every* byte it covers is unclaimed, preserving
/// the flat, non-overlapping invariant the overlay renderer needs.
fn fill_inline_gaps(out: &mut Vec<Highlight>, source: &str, owner: &[Option<usize>]) {
    let free = |lo: usize, hi: usize| owner[lo..hi.min(owner.len())].iter().all(Option::is_none);

    let mut inline = Vec::new();
    highlight_emphasis(&mut inline, source, 0, source.len());
    highlight_timestamps(&mut inline, source, 0, source.len());

    for h in inline {
        if free(h.start, h.end) {
            out.push(h);
        }
    }
    out.sort_by_key(|h| (h.start, h.end));
}

/// Push a span, skipping empties.
fn push(out: &mut Vec<Highlight>, start: usize, end: usize, cap: &str) {
    if end > start {
        out.push(Highlight {
            start,
            end,
            capture: cap.to_string(),
        });
    }
}

/// Color balanced inline emphasis (`*b*`, `/i/`, `_u_`, `+s+`, `~c~`, `=v=`).
/// We color the whole `MARKERcontentMARKER` span — simple and alignment-safe.
fn highlight_emphasis(out: &mut Vec<Highlight>, source: &str, start: usize, end: usize) {
    let seg = &source[start..end];
    let bytes = seg.as_bytes();
    let markers: &[(u8, &str)] = &[
        (b'*', "markup.strong"),
        (b'/', "markup.italic"),
        (b'_', "markup.underline"),
        (b'+', "markup.strikethrough"),
        (b'~', "markup.raw"),
        (b'=', "markup.raw"),
    ];
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some((_, cap)) = markers.iter().find(|(m, _)| *m == c) {
            // Opening marker must be preceded by a boundary and followed by a
            // non-space; find a matching closing marker on the same segment.
            let pre_ok = i == 0 || bytes[i - 1].is_ascii_whitespace() || is_pre_punct(bytes[i - 1]);
            if pre_ok && i + 1 < bytes.len() && !bytes[i + 1].is_ascii_whitespace() {
                if let Some(close) = find_close(bytes, i + 1, c) {
                    push(out, start + i, start + close + 1, cap);
                    i = close + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn is_pre_punct(b: u8) -> bool {
    matches!(b, b'(' | b'{' | b'[' | b'\'' | b'"' | b'-')
}

/// Find the closing marker `m` for an emphasis run starting after an opener,
/// requiring the char before it to be non-space.
fn find_close(bytes: &[u8], from: usize, m: u8) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() {
        if bytes[j] == m && !bytes[j - 1].is_ascii_whitespace() {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Color `<...>` and `[...]` Org timestamps in `[start, end)`.
fn highlight_timestamps(out: &mut Vec<Highlight>, source: &str, start: usize, end: usize) {
    let seg = &source[start..end];
    let bytes = seg.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let open = bytes[i];
        let close = match open {
            b'<' => b'>',
            b'[' => b']',
            _ => {
                i += 1;
                continue;
            }
        };
        // A timestamp starts with a digit year after the bracket.
        if let Some(rest) = seg.get(i + 1..) {
            if rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                if let Some(off) = rest.find(close as char) {
                    let ts_end = i + 1 + off + 1;
                    // Require it to look date-ish (contains a '-').
                    if seg[i..ts_end].contains('-') {
                        push(out, start + i, start + ts_end, "constant.timestamp");
                        i = ts_end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(src: &str) -> Vec<(String, String)> {
        let mut h = TsHighlighter::new().unwrap();
        h.highlight(src)
            .unwrap()
            .into_iter()
            .map(|s| (src[s.start..s.end].to_string(), s.capture))
            .collect()
    }

    #[test]
    fn spans_are_flat_and_sorted() {
        let mut h = TsHighlighter::new().unwrap();
        let src = "* Head\n#+TITLE: x\n# comment\n- item *bold*\n| a | b |\n";
        let spans = h.highlight(src).unwrap();
        for w in spans.windows(2) {
            assert!(w[0].end <= w[1].start, "overlap: {:?}", w);
        }
    }

    #[test]
    fn classifies_headline_marker_and_title() {
        let got = caps("* Getting Started\n");
        assert!(got.iter().any(|(t, c)| t == "*" && c == "markup.heading.marker"));
        assert!(got.iter().any(|(t, c)| t.contains("Getting") && c == "markup.heading"));
    }

    #[test]
    fn classifies_keyword_comment_block() {
        // Blank line before `#` so the grammar parses it as a comment, not part
        // of the directive's affiliated region.
        let got = caps("#+TITLE: Hi\n\n# a note\n#+BEGIN_SRC rust\ncode()\n#+END_SRC\n");
        assert!(got.iter().any(|(t, c)| t.starts_with("#+TITLE") && c == "keyword.directive"));
        assert!(got.iter().any(|(t, c)| t.contains("note") && c == "comment"));
        assert!(got.iter().any(|(t, c)| t.contains("code()") && c == "markup.raw.block"));
    }

    #[test]
    fn classifies_planning_and_timestamp() {
        // Planning lines are only parsed as `entry`s under a headline.
        let got = caps("* Task\nCLOSED: [2025-01-15 Wed]\n");
        assert!(got.iter().any(|(t, c)| t.starts_with("CLOSED") && c == "keyword.plan"));
        assert!(got.iter().any(|(t, c)| t.contains("2025-01-15") && c == "constant.timestamp"));
    }

    #[test]
    fn classifies_emphasis_via_gap_fill() {
        // Emphasis has no grammar node; it's scanned into the query's gaps.
        let got = caps("- a *bold* and /italic/ x\n");
        assert!(got.iter().any(|(t, c)| t == "*bold*" && c == "markup.strong"));
        assert!(got.iter().any(|(t, c)| t == "/italic/" && c == "markup.italic"));
    }

    #[test]
    fn emphasis_does_not_recolor_query_owned_bytes() {
        // A headline's stars AND its whole title are owned by the query
        // (`markup.heading.marker` + `markup.heading`). The emphasis scanner
        // must not carve a bogus `markup.strong` span out of query-owned bytes,
        // so `*bold*` inside a heading title stays part of the heading.
        let got = caps("* Head with *bold*\n");
        assert!(got.iter().any(|(t, c)| t == "*" && c == "markup.heading.marker"));
        assert!(got.iter().any(|(t, c)| t == "Head with *bold*" && c == "markup.heading"));
        assert!(!got.iter().any(|(_, c)| c == "markup.strong"));
    }

    #[test]
    fn empty_source_is_empty() {
        let mut h = TsHighlighter::new().unwrap();
        assert!(h.highlight("").unwrap().is_empty());
    }

    #[test]
    fn highlight_tree_matches_highlight() {
        // The single-parse entry point yields the same spans as the
        // parse-internally path.
        let src = "* Getting Started\n#+TITLE: x\n| a | b |\n";
        let mut h = TsHighlighter::new().unwrap();
        let via_internal = h.highlight(src).unwrap();

        let mut parser = crate::Parser::new().unwrap();
        let tree = parser.parse_tree(src, None).unwrap();
        let via_tree = h.highlight_tree(&tree, src).unwrap();

        assert_eq!(via_internal, via_tree);
        assert!(!via_tree.is_empty());
        assert!(via_tree.iter().any(|s| s.capture.contains("heading")));
        assert!(via_tree.iter().any(|s| s.capture == "keyword.directive"));
    }
}
