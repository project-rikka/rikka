---
title: HTML reading (rikka-html)
description: rikka-html is the public dependency-free HTML tokenizer, forgiving tree, selector subset, entity decoder and escaper.
about: crates/web/rikka-html
tags: [web, html]
---

- Public, no I/O, only `thiserror`. Users: `browser-translate` (service markup), `browser-reader` and the browser's interstitial (escaping). `to_markdown` and `to_text` turn mail-style HTML bodies into Markdown or plain text.
- `tokens(html)` yields `Token::{Text, Open(Tag), Close}`; names are lowercased, text and attribute values are entity-decoded, comments, doctypes and processing instructions are skipped, and a `<` that starts no tag stays text.
- `script`, `style`, `iframe`, `noembed`, `noframes`, `noscript`, `xmp` are raw text; `textarea` and `title` are raw text with entities decoded.
- `Document::parse` never fails. Nodes are stored in document order with their end index, so descendants are a slice. It applies void elements, implied end tags (`p`, `li`, `dt`/`dd`, `option`, headings, table rows and cells) and scope-limited end tags; it does not insert `html`/`head`/`body`/`tbody`, foster-parent, or run the adoption agency.
- `/>` closes an element only inside `svg`/`math`.
- `Selector::parse` knows type, `*`, `#id`, `.class`, `[a]`, `[a='v']`, `[a*='v']`, `:not(<compound>)`, descendant, child `>` and comma lists; anything else is `Error::Selector`. `Element::select` yields descendants only and matches ancestors outside the element too.
- `decode` knows numeric references (Windows-1252 remapping, U+FFFD for invalid) and the HTML 4 named set; named references need their `;`.
- `Document::to_markdown` (`src/markdown/`) is the workspace's only HTML→Markdown conversion, written for rikka-markdown's parser options: tree → `Block`s/`Phrasing` (styled runs), then rendering; `to_text` renders the same blocks as plain text with links as `text <url>`. It flattens layout tables (a row of single-line cells joins with ` · `), keeps tables with `th`/`thead`, simple cells and 2+ rows as GFM tables (colspan and rowspan padded), drops hidden content (`hidden`, `display:none`, `visibility:hidden`, `mso-hide:all`) and every image (alt text only inside a link), links only http/https/mailto, escapes all text so it reads back verbatim, alternates markers of adjacent lists, and omits emphasis delimiters CommonMark flanking would leave literal. Element depth caps at 256 (deeper content is gathered flat, still skipping hidden parts) and quote/list nesting at 16.
- `Escaped(text)` is the one `Display` escaper for text and attribute values.
