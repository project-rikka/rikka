---
title: XML reader (rikka-xml)
description: rikka-xml parses XML into a borrowed element tree, strict with positioned errors or lenient with recovery.
about: crates/foundation/rikka-xml
tags: [xml, foundation]
---

- The workspace's one XML reader: public, depends only on `thiserror`. Users are the fontconfig loader (rikka-text-catalog), rikka-svg, rikka-wayland-scanner, rikka-xdg (recent files, MIME comments) and nixpkgs-search.
- `parse(source)` is strict: it returns the root `Element` or an `Error` whose variants carry a `Position { line, column }`. `parse_lenient(source)` recovers the way a tolerant config reader wants (unclosed and mismatched tags close, bad attributes drop their element, unknown references stay verbatim) and gives `None` only without a root.
- `Element<'a>` borrows the source: `name()` is the local name, `namespace()` the resolved URI, `attribute(name)` an unprefixed attribute, `attribute_in(namespace, name)` a prefixed one (`xml:` is predeclared), `attributes()` every attribute as (namespace, local name, value), `namespaces()` the declarations on that element, `range()` its byte range in the source.
- `children()`, `children_named(name)` and `descendants()` (the element first, then document order) yield elements only; `contents()` yields `Content::Element`/`Content::Text` in document order (mixed content, as SVG text needs). Whitespace-only text inside the root is kept; `text()` is the element's own character data, CDATA included, joined and trimmed.
- Predefined and numeric references are decoded; DTDs are skipped, so entities they declare are unknown. Nesting is limited to `MAX_DEPTH`; both modes give up past it.
