; Tree-sitter highlights query for Org mode.
;
; Captures map to standard highlight names; the consuming app turns each into a
; CSS class (`tok-<capture>`, dots → dashes) and themes it.

; --- Headlines ---
(headline (stars) @markup.heading.marker)
(headline (item) @markup.heading)

; --- Lists ---
(listitem (bullet) @markup.list)
(checkbox) @markup.list.checked

; --- Keyword lines + comments ---
(directive) @keyword.directive
(comment) @comment

; --- Block bodies ---
(block (contents) @markup.raw.block)

; --- Property drawers ---
(property_drawer ":properties:" @property)
(property_drawer ":end:" @property)
(property name: (expr) @property.name)

; --- Planning + timestamps ---
(entry name: (entry_name) @keyword.plan)
(timestamp) @constant.timestamp

; --- Tables ---
(cell) @markup.table.cell

; --- Tags ---
(tag) @label

; --- Drawers + dynamic blocks (structural, like blocks) ---
(drawer) @markup.raw.block
(dynamic_block (contents) @markup.raw.block)

; --- Footnote definitions + LaTeX ---
(fndef (description) @markup.link.label)
(latex_env) @markup.raw
(formula) @markup.math

; --- Horizontal rule ---
(hr) @punctuation.special

; NOTE: inline emphasis (*bold*, /italic/, ~code~, …) has NO node in the
; tree-sitter-org grammar, so it cannot be captured here. It is filled into the
; gaps this query leaves by the byte scanner in highlight.rs (see fill_inline_gaps).
