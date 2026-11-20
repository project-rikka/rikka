((host) => {
  "use strict";

  const { post, top: TOP, LETTER, SPACES, INVISIBLE, OPAQUE, KEPT, EDITORS, CODE_SURE, CODE_HINT } = host;
  const UNIT_CHARS = 1500;
  const UNIT_LIMIT = 18000;
  const KEEP_CHARS = 300;
  const BATCH_UNITS = 40;
  const BATCH_CHARS = 6000;
  const IN_FLIGHT = 4;
  const MUTATION_DEBOUNCE_MS = 250;
  const DEPTH = 30;
  const COMMENT_CHARS = 2000;
  const REGION_CHARS = 1000000;
  const CACHE_SIZE = 4000;
  const CHURN_LIMIT = 10;
  const CHURN_MS = 10000;
  const PUMP_DELAY_MS = 150;

  const WAITING = 0;
  const SENT = 1;
  const DONE = 2;
  const DROPPED = 3;
  const DISCARDED = 4;

  const FLOW = 0;
  const ATTRIBUTE = 1;
  const TITLE = 2;
  const COMMENT = 3;

  const OTHER = 0;
  const IGNORE = 1;
  const BREAK = 2;
  const KEEP = 3;
  const OPAQUE_INLINE = 4;
  const INLINE = 5;
  const BLOCK = 6;
  const SKIP = 7;
  const REGION = 8;
  const HOST = 9;

  const LETTERS = /\p{L}/gu;
  const WORDISH = /[\p{L}\p{N}]/u;

  const BLOCK_TAGS = new Set(["address", "article", "aside", "blockquote", "body", "caption", "center", "colgroup", "dd", "details", "dialog", "dir", "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form", "frameset", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup", "hr", "html", "legend", "li", "listing", "main", "menu", "nav", "ol", "optgroup", "option", "p", "plaintext", "pre", "search", "section", "summary", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "ul", "xmp"]);
  const UNCLONED = new Set(["iframe", "frame", "object", "embed", "video", "audio", "canvas", "input", "select", "textarea", "applet"]);
  const PROSE = new Set(["p", "li", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "dd", "dt", "figcaption", "summary", "caption"]);
  const CELLS = new Set(["td", "th"]);
  const FLOWING = new Set(["inline", "inline-block", "contents", "none", "ruby", "ruby-base", "ruby-text"]);
  const PRESERVING = new Set(["pre", "pre-wrap", "pre-line", "break-spaces"]);
  const CODE_BLOCKS = new Set(["div", "p", "li", "tr", "td", "th", "table", "tbody", "thead", "tfoot", "pre", "ol", "ul", "dl", "dd", "dt", "section", "article", "blockquote", "details", "summary", "figure", "header", "footer", "h1", "h2", "h3", "h4", "h5", "h6"]);
  const NON_TEXT_TAGS = new Set([...INVISIBLE, ...OPAQUE, "button", "rikka-translation"]);
  const ATTRIBUTES = ["title", "alt", "placeholder", "aria-label"];
  const OBSERVED_ATTRIBUTES = [...ATTRIBUTES, "value", "data-code-text"];
  const BUTTONISH = /^(?:button|submit|reset)$/i;

  const GUTTERS = ".react-line-numbers, .react-line-number, .blob-num, .diff-line-number";
  const REGION_GUTTERS = ".react-line-numbers, .react-line-number, .blob-num, .diff-line-number, .diff-text-marker, .blob-code-marker, .hunk, .blob-code-hunk, .linenos, .lineno, .line-numbers-rows, .hljs-ln-numbers, .line-number, .line-numbers, .ln-num";
  const DIFF_CELLS = ".blob-code, .blob-code-inner, .diff-text-cell";
  const COMMENT_TOKENS = ".pl-c, .hljs-comment, .token.comment, .tok-comment, .cm-comment, .c, .c1, .cm, .ch, .cs, .cd, .sd, .ace_comment, .sourceCode .co, .sourceCode .do, .syntax--comment";
  const STRING_TOKENS = ".pl-s, .hljs-string, .token.string, .token.triple-quoted-string, .s, .s1, .s2, .sourceCode .st";
  const HIGHLIGHTED = /(?:^|\s)(?:pl-[a-z0-9]+|hljs-[\w-]+|token|tok-[\w-]+|cm-[\w-]+|ace_[\w-]+|syntax--[\w-]+|[knopsc]|[kn][bcdfnrt]|nx|s[12bdx]|mi|mf|c1|err)(?:\s|$)/;
  const NAVIGATION = "nav, [role=navigation], [role=menu], [role=menubar], [role=tablist], [role=toolbar], button, label, select";
  const ICON_CLASS = /(?:^|\s)material-(?:icons|symbols)/;
  const ICON_FONT = /icon|symbol|glyph|awesome|octicon|material/i;
  const ICON_WORD = /^(?:[a-z][a-z0-9_]{1,40}|[\ue000-\uf8ff])$/;
  const LANGUAGE_CLASS = /(?:^|\s)(?:language|lang|highlight-source|highlight-text|highlight)-([\w+#-]+)/i;
  const PAGE_EXTENSIONS = new Set(["html", "htm", "xhtml", "shtml", "php", "asp", "aspx", "jsp", "cgi", "md", "markdown", "mdx", "rst", "txt", "adoc"]);

  const BLOCK_OPEN = /^(?:\/\*|<!--|\{-|--\[=*\[|[rRuUbB]{0,2}(?:"""|''')|=begin|#\||%\{|\(\*)/;
  const DOC_OPEN = /^[rRuUbB]{0,2}(?:"""|''')/;
  const LINE_MARK = /^(?:\/\/[/!]?|#+|--+|;+|%+)/;
  const FENCE = /^(?:```|~~~)/;
  const BULLET = /^(?:[-*+•]\s|\d{1,3}[.)]\s|@\w)/;
  const DIRECTIVE = /^(?:!|-\*-|vim?:|eslint|jshint|jscs:|prettier-ignore|istanbul ignore|c8 ignore|@ts-|tslint:|noqa|type:\s*ignore|pylint:|pyright:|mypy:|fmt:\s|isort:|rubocop:|frozen_string_literal|(?:en)?coding[:=]|swiftlint:|nolint|go:\w|\+build|SPDX-License-Identifier|@flow|@jsx|@license|@preserve|clang-format|cspell:|spell-checker:|biome-ignore|deno-lint|@refresh|<reference|shellcheck|nix-shell|@generated|DO NOT EDIT)/i;
  const LICENSE = /\b(?:SPDX-License-Identifier|Copyright\s+(?:\(c\)|©|\d{4})|All rights reserved|Licensed under|Permission is hereby granted|WITHOUT WARRANT)/i;
  const CODELIKE = /[;{}]\s*$|^[\w$.[\]"']+\s*(?:[-+*/%|&^]|<<|>>)?=(?!=)\s*\S|^[\w$.]+\(.*\)\s*;?$|^(?:import|from|let|const|var|fn|def|func|return|class|struct|use|#include|#define|public|private|static|async|await|export|package)\s+\S.*[;({=]\s*$|^<\/?[a-zA-Z][\w-]*[\s>]|^@\w+\(/;
  const WIDE = /[\u1100-\u115f\u2e80-\u303e\u3041-\u33ff\u3400-\u4dbf\u4e00-\u9fff\ua000-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe30-\ufe4f\uff00-\uff60\uffe0-\uffe6]/;
  const UNSPACED = /[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u0e00-\u0eff\u1000-\u109f\u1780-\u17ff]/g;
  const UNBREAKABLE = /https?:\/\/\S+|`[^`]*`|[A-Za-z0-9_][\w.'’-]*[A-Za-z0-9_]|[\ud800-\udbff][\udc00-\udfff]/g;
  const NO_START = new Set([..."、。，．・：；？！）」』】〕〉》｝］’”ー…‥々ゝゞぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮヵヶ,.!?:;)]}%"]);
  const NO_END = new Set([..."（「『【〔〈《｛［‘“([{"]);

  const FAMILIES = {
    c: { line: ["//"], block: [["/*", "*/"]], quotes: "\"'`" },
    rust: { line: ["//"], block: [["/*", "*/"]], nest: true, quotes: "\"", rust: true },
    python: { line: ["#"], quotes: "\"'", triple: true },
    ruby: { line: ["#"], quotes: "\"'", begin: true },
    hash: { line: ["#"], quotes: "\"'", word: true },
    nix: { line: ["#"], block: [["/*", "*/"]], quotes: "\"", word: true },
    sql: { line: ["--"], block: [["/*", "*/"]], quotes: "'\"" },
    lua: { line: ["--"], quotes: "'\"", lua: true },
    haskell: { line: ["--"], block: [["{-", "-}"]], nest: true, quotes: "\"", dash: true },
    lisp: { line: [";"], block: [["#|", "|#"]], quotes: "\"" },
    ini: { line: [";", "#"], start: true },
    asm: { line: [";", "#", "//"], block: [["/*", "*/"]], quotes: "\"", word: true },
    markup: { block: [["<!--", "-->"]] },
    css: { block: [["/*", "*/"]], quotes: "\"'" },
    scss: { line: ["//"], block: [["/*", "*/"]], quotes: "\"'" },
    tex: { line: ["%"], tex: true },
    percent: { line: ["%"], block: [["%{", "%}"]], quotes: "\"" },
    ml: { block: [["(*", "*)"]], nest: true, quotes: "\"" },
    unknown: { line: ["//", "# "], block: [["/*", "*/"]], start: true },
  };
  const CLOSERS = { "/*": "*/", "<!--": "-->", "{-": "-}", "#|": "|#", "%{": "%}", "(*": "*)", "=begin": "=end" };
  const LANGUAGES = new Map();
  const family = (name, names) => names.split(" ").forEach((alias) => LANGUAGES.set(alias, name));
  family("c", "c h cc cpp cxx hpp hh hxx c++ cs csharp c# java js jsx mjs cjs javascript ts tsx mts cts typescript go golang swift kotlin kt kts scala sc dart groovy gradle php objc objective-c objectivec m mm zig v d proto protobuf solidity sol json5 jsonc glsl hlsl wgsl cuda cu vala hx haxe less styl stylus fsharp fs fsx pascal pas delphi");
  family("rust", "rs rust");
  family("python", "py python python3 py3 pyi pyw gyp bzl bazel starlark build");
  family("ruby", "rb ruby rake gemspec podspec rbi crystal cr");
  family("hash", "sh bash zsh fish ksh shell console shellsession shell-session ps1 powershell pwsh yaml yml toml perl pl pm r rscript make makefile mk cmake dockerfile docker containerfile conf nginx tcl elixir ex exs julia jl coffee coffeescript gitignore properties awk sed graphql gql nim");
  family("nix", "nix");
  family("sql", "sql psql mysql plsql pgsql sqlite tsql postgres postgresql ada adb ads vhdl vhd applescript");
  family("lua", "lua luau");
  family("haskell", "hs haskell elm purescript purs agda idris lhs");
  family("lisp", "lisp cl el elisp emacs-lisp scheme scm rkt racket clj cljs cljc clojure edn fennel fnl");
  family("ini", "ini cfg desktop reg editorconfig gitconfig");
  family("asm", "asm s nasm masm gas x86asm armasm");
  family("markup", "html htm xhtml xml svg md markdown mdx vue svelte astro xaml plist rss atom xsl xslt wsdl");
  family("css", "css");
  family("scss", "scss sass");
  family("tex", "tex latex ltx sty cls bib context");
  family("percent", "matlab octave erl erlang hrl prolog");
  family("ml", "ml mli ocaml sml");

  let run = null;

  const primary = (tag) => {
    const lower = String(tag || "").trim().toLowerCase().replace(/_/g, "-");
    const head = lower.split("-")[0];
    if (head === "zh") return /^zh-(?:tw|hk|mo|hant)/.test(lower) ? "zh-tw" : "zh-cn";
    if (head === "iw") return "he";
    if (head === "in") return "id";
    if (head === "jw") return "jv";
    return head;
  };

  const flows = (element) => {
    if (BLOCK_TAGS.has(element.localName)) return false;
    let display = run.displays.get(element);
    if (display === undefined) {
      display = getComputedStyle(element).display;
      run.displays.set(element, display);
    }
    return FLOWING.has(display);
  };

  const preserving = (element) => {
    const style = getComputedStyle(element);
    return PRESERVING.has(style.whiteSpace) || style.whiteSpaceCollapse === "preserve" || style.whiteSpaceCollapse === "preserve-breaks";
  };

  const classOf = (element) => element.getAttribute("class") || "";

  const isExcluded = (element) => {
    if (element.classList.contains("notranslate")) return true;
    const translate = element.getAttribute("translate");
    if (translate !== null && translate.toLowerCase() === "no") return true;
    if (element.classList.length && element.matches(EDITORS)) return true;
    const editable = element.getAttribute("contenteditable");
    if (editable !== null && editable.toLowerCase() !== "false") return true;
    const lang = element.getAttribute("lang");
    return Boolean(lang) && element !== document.body && element !== document.documentElement && primary(lang) === run.target;
  };

  const iconic = (element) => {
    if (ICON_CLASS.test(classOf(element))) return true;
    if (element.firstElementChild) return false;
    if (!ICON_WORD.test((element.textContent || "").trim())) return false;
    return ICON_FONT.test(getComputedStyle(element).fontFamily);
  };

  const codeKind = (element) => {
    const tag = element.localName;
    if (tag === "pre") return REGION;
    if (tag === "code") return flows(element) ? KEEP : REGION;
    if (KEPT.has(tag)) return flows(element) ? KEEP : SKIP;
    if (tag === "table" && element.querySelector(DIFF_CELLS)) return REGION;
    if (!element.classList.length) return OTHER;
    if (element.matches(CODE_SURE)) return REGION;
    if (element.matches(CODE_HINT) && element.querySelector("pre, code")) return REGION;
    return OTHER;
  };

  const kindOf = (element) => {
    const tag = element.localName;
    if (INVISIBLE.has(tag)) return IGNORE;
    if (tag === "rikka-translation") return SKIP;
    if (tag === "br") return BREAK;
    if (tag === "rt" || tag === "rp") return KEEP;
    if (OPAQUE.has(tag)) return flows(element) ? OPAQUE_INLINE : SKIP;
    if (element.classList.length && element.matches(GUTTERS)) return SKIP;
    const code = codeKind(element);
    if (code !== OTHER) return code;
    if (isExcluded(element)) {
      if (flows(element)) return KEEP;
      const pre = element.querySelector(":scope > pre");
      return pre && (element.textContent || "").trim() === (pre.textContent || "").trim() ? REGION : SKIP;
    }
    if (element.shadowRoot) return HOST;
    if (tag === "button" && !LETTER.test(element.textContent || "")) return OPAQUE_INLINE;
    if (!BLOCK_TAGS.has(tag) && iconic(element)) return OPAQUE_INLINE;
    return flows(element) ? INLINE : BLOCK;
  };

  const letterCount = (text) => {
    const found = text.match(LETTERS);
    return found ? found.length : 0;
  };

  const widths = (text) => {
    const result = new Array(text.length + 1);
    result[0] = 0;
    for (let index = 0; index < text.length; index++) {
      const code = text.charCodeAt(index);
      let add = 1;
      if (code >= 0xd800 && code <= 0xdbff) add = 2;
      else if (code >= 0xdc00 && code <= 0xdfff) add = 0;
      else if (WIDE.test(text[index])) add = 2;
      result[index + 1] = result[index] + add;
    }
    return result;
  };

  const widthOf = (text) => widths(text)[text.length];

  const spaced = (text) => {
    const unspaced = text.match(UNSPACED);
    return (unspaced ? unspaced.length : 0) * 3 < letterCount(text);
  };

  const wordStarts = (text, language) => {
    try {
      return Array.from(new Intl.Segmenter(language, { granularity: "word" }).segment(text), (segment) => segment.index);
    } catch {
      return Array.from(new Intl.Segmenter(undefined, { granularity: "word" }).segment(text), (segment) => segment.index);
    }
  };

  const unbreakableMask = (text, language) => {
    const locked = new Uint8Array(text.length + 1);
    if (typeof Intl.Segmenter === "function") {
      locked.fill(1);
      for (const start of wordStarts(text, language)) locked[start] = 0;
      locked[text.length] = 0;
    }
    for (const match of text.matchAll(UNBREAKABLE)) {
      for (let index = match.index + 1; index < match.index + match[0].length; index++) locked[index] = 1;
    }
    return locked;
  };

  const spaceNear = (text, position, from) => {
    let left = -1;
    for (let index = Math.min(position, text.length - 1); index > from; index--) {
      if (text[index] === " ") {
        left = index;
        break;
      }
    }
    let right = -1;
    for (let index = Math.max(position, from + 1); index < text.length; index++) {
      if (text[index] === " ") {
        right = index;
        break;
      }
    }
    if (left < 0 && right < 0) return text.length;
    if (left < 0) return right;
    if (right < 0) return left;
    return position - left <= right - position ? left : right;
  };

  const breakNear = (text, position, from, locked) => {
    const allowed = (index) => index > from && index < text.length && !locked[index] && !NO_START.has(text[index]) && !NO_END.has(text[index - 1]);
    for (let distance = 0; distance <= text.length; distance++) {
      if (allowed(position - distance)) return position - distance;
      if (allowed(position + distance)) return position + distance;
      if (position - distance <= from && position + distance >= text.length) break;
    }
    return text.length;
  };

  const spread = (target, sizes, language) => {
    const measured = widths(target);
    const full = measured[target.length];
    if (sizes.length === 1 || full <= sizes[0]) return sizes.map((size, line) => (line ? "" : target));
    const total = sizes.reduce((sum, size) => sum + size, 0);
    const isSpaced = spaced(target);
    const locked = isSpaced ? null : unbreakableMask(target, language);
    const chunks = [];
    let from = 0;
    let position = 0;
    let sum = 0;
    for (let line = 0; line < sizes.length - 1; line++) {
      sum += sizes[line];
      const goal = (full * sum) / total;
      position = Math.max(position, from);
      while (position < target.length && measured[position] < goal) position++;
      if (position > from && goal - measured[position - 1] < measured[position] - goal) position--;
      const cut = Math.max(from, isSpaced ? spaceNear(target, position, from) : breakNear(target, position, from, locked));
      chunks.push(target.slice(from, cut).trim());
      from = cut;
    }
    chunks.push(target.slice(from).trim());
    return chunks;
  };

  const lineAllowed = (text, index, marker, spec, blank) => {
    if (spec.start && !blank) return false;
    if (spec.word && !blank && !/\s/.test(text[index - 1])) return false;
    if (spec.tex && text[index - 1] === "\\") return false;
    if (spec.dash) {
      let after = index;
      while (text[after] === "-") after++;
      if (after < text.length && /[!#$%&*+./<=>?@\\^|~:]/.test(text[after])) return false;
    }
    return true;
  };

  const closing = (text, from, open, close, nest) => {
    if (!nest) {
      const end = text.indexOf(close, from);
      return end < 0 ? text.length : end + close.length;
    }
    let depth = 1;
    let index = from;
    while (index < text.length) {
      if (text.startsWith(close, index)) {
        depth--;
        index += close.length;
        if (depth === 0) return index;
      } else if (text.startsWith(open, index)) {
        depth++;
        index += open.length;
      } else {
        index++;
      }
    }
    return text.length;
  };

  const skipString = (text, index, quote) => {
    for (let at = index + 1; at < text.length; at++) {
      const character = text[at];
      if (character === "\\") {
        at++;
        continue;
      }
      if (character === quote) return at + 1;
      if (character === "\n" && quote !== "`") return at;
    }
    return text.length;
  };

  const docstring = (text, start, end) => {
    const lineEnd = text.indexOf("\n", end);
    if (text.slice(end, lineEnd < 0 ? text.length : lineEnd).trim()) return false;
    let before = start - 1;
    while (before >= 0 && /\s/.test(text[before])) before--;
    if (before < 0 || text[before] === ":") return true;
    return !/[=([{,\\+\-*/%&|<>]/.test(text[before]);
  };

  const lex = (text, name) => {
    const spec = FAMILIES[name] || FAMILIES.unknown;
    const found = [];
    const length = text.length;
    let index = 0;
    let blank = true;
    scan: while (index < length) {
      const character = text[index];
      if (character === "\n") {
        blank = true;
        index++;
        continue;
      }
      if (character === " " || character === "\t" || character === "\r") {
        index++;
        continue;
      }
      if (spec.lua && text.startsWith("--[", index)) {
        const long = /^--\[(=*)\[/.exec(text.slice(index, index + 40));
        if (long) {
          const close = `]${long[1]}]`;
          const end = text.indexOf(close, index + long[0].length);
          const stop = end < 0 ? length : end + close.length;
          found.push({ start: index, end: stop });
          index = stop;
          blank = false;
          continue;
        }
      }
      if (spec.block && (!spec.start || blank)) {
        for (const [open, close] of spec.block) {
          if (text.startsWith(open, index)) {
            const end = closing(text, index + open.length, open, close, spec.nest);
            found.push({ start: index, end });
            index = end;
            blank = false;
            continue scan;
          }
        }
      }
      if (spec.begin && blank && (index === 0 || text[index - 1] === "\n") && text.startsWith("=begin", index)) {
        const close = text.indexOf("\n=end", index);
        const lineEnd = close < 0 ? -1 : text.indexOf("\n", close + 1);
        const end = close < 0 || lineEnd < 0 ? length : lineEnd;
        found.push({ start: index, end });
        index = end;
        continue;
      }
      if (spec.line) {
        for (const marker of spec.line) {
          if (text.startsWith(marker, index) && lineAllowed(text, index, marker, spec, blank)) {
            const end = text.indexOf("\n", index);
            const stop = end < 0 ? length : end;
            found.push({ start: index, end: stop });
            index = stop;
            continue scan;
          }
        }
      }
      const identifierBefore = index > 0 && /[\w$]/.test(text[index - 1]);
      if (spec.triple && !identifierBefore && /["'rRuUbBfF]/.test(character)) {
        const triple = /^[rRuUbBfF]{0,2}("""|''')/.exec(text.slice(index, index + 5));
        if (triple) {
          const end = text.indexOf(triple[1], index + triple[0].length);
          const stop = end < 0 ? length : end + 3;
          if (blank && docstring(text, index, stop)) found.push({ start: index, end: stop });
          index = stop;
          blank = false;
          continue;
        }
      }
      if (spec.rust) {
        if ((character === "r" || character === "b") && !identifierBefore) {
          const raw = /^b?r(#*)"/.exec(text.slice(index, index + 260));
          if (raw) {
            const close = `"${raw[1]}`;
            const end = text.indexOf(close, index + raw[0].length);
            index = end < 0 ? length : end + close.length;
            blank = false;
            continue;
          }
        }
        if (character === "'") {
          const literal = /^'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]{1,6}\}|.)|[^\\'\n])'/.exec(text.slice(index, index + 14));
          index += literal ? literal[0].length : 1;
          blank = false;
          continue;
        }
      }
      if (spec.quotes && spec.quotes.includes(character)) {
        index = skipString(text, index, character);
        blank = false;
        continue;
      }
      blank = false;
      index++;
    }
    return found;
  };

  const standaloneAt = (text, position) => !text.slice(text.lastIndexOf("\n", position - 1) + 1, position).trim();

  const closerOf = (opener) => {
    if (opener.startsWith("--[")) return `]${opener.slice(3, -1)}]`;
    const quotes = DOC_OPEN.exec(opener);
    if (quotes) return opener.slice(-3);
    return CLOSERS[opener];
  };

  const fromMarks = (text, marks) => {
    const found = [];
    const rowOf = rowsOf(text);
    let open = null;
    for (const mark of marks) {
      let start = mark.start;
      while (start < mark.end && /\s/.test(text[start])) start++;
      if (start >= mark.end) continue;
      const head = text.slice(start, Math.min(mark.end, start + 12));
      if (open && (open.doc !== mark.doc || rowOf(start) > open.row + 1)) {
        found.push(open);
        open = null;
      }
      if (open) {
        open.end = mark.end;
        open.row = rowOf(mark.end);
        if (text.slice(start, mark.end).includes(open.close)) {
          found.push(open);
          open = null;
        }
        continue;
      }
      if (mark.doc && !(DOC_OPEN.test(head) && standaloneAt(text, start))) continue;
      const opener = BLOCK_OPEN.exec(head);
      if (opener) {
        const close = closerOf(opener[0]);
        const comment = { start, end: mark.end, close, doc: mark.doc, row: rowOf(mark.end) };
        if (text.slice(start + opener[0].length, mark.end).includes(close)) found.push(comment);
        else open = comment;
        continue;
      }
      found.push({ start, end: mark.end });
    }
    if (open) found.push(open);
    return found;
  };

  const lineContent = (text, start, end) => {
    let from = start;
    while (from < end && (text[from] === " " || text[from] === "\t")) from++;
    let to = end;
    while (to > from && /\s/.test(text[to - 1])) to--;
    return { start: from, end: to, indent: from - start };
  };

  const blockLines = (text, comment, opener) => {
    const close = closerOf(opener);
    let bodyEnd = comment.end;
    while (bodyEnd > comment.start && /\s/.test(text[bodyEnd - 1])) bodyEnd--;
    if (close && bodyEnd - comment.start >= opener.length + close.length && text.startsWith(close, bodyEnd - close.length)) bodyEnd -= close.length;
    const lines = [];
    let position = comment.start + opener.length;
    let first = true;
    while (position <= bodyEnd) {
      let lineEnd = text.indexOf("\n", position);
      if (lineEnd < 0 || lineEnd > bodyEnd) lineEnd = bodyEnd;
      let from = position;
      while (from < lineEnd && (text[from] === " " || text[from] === "\t")) from++;
      if (first) {
        while (from < lineEnd && (text[from] === "*" || text[from] === "!" || text[from] === "|")) from++;
      } else if (text[from] === "*" && text[from + 1] !== "/") {
        from++;
      }
      let to = lineEnd;
      while (to > from && /\s/.test(text[to - 1])) to--;
      if (lineEnd === bodyEnd) {
        while (to > from && text[to - 1] === "*") to--;
        while (to > from && /\s/.test(text[to - 1])) to--;
      }
      const content = lineContent(text, from, to);
      lines.push(content);
      first = false;
      if (lineEnd === bodyEnd) break;
      position = lineEnd + 1;
    }
    return lines;
  };

  const acceptable = (text, lines) => {
    const joined = lines.map((line) => text.slice(line.start, line.end)).join(" ");
    return letterCount(joined) >= 2 && !LICENSE.test(joined);
  };

  const capped = (text, lines) => {
    const parts = [];
    let part = [];
    let size = 0;
    for (const line of lines) {
      const length = line.end - line.start + 1;
      if (part.length && size + length > COMMENT_CHARS) {
        parts.push(part);
        part = [];
        size = 0;
      }
      part.push(line);
      size += length;
    }
    if (part.length) parts.push(part);
    return parts;
  };

  const split = (text, lines) => {
    const found = [];
    const filled = lines.filter((line) => line.end > line.start);
    const base = filled.length ? Math.min(...filled.map((line) => line.indent)) : 0;
    let current = [];
    let fenced = false;
    const end = () => {
      if (current.length && acceptable(text, current)) found.push(...capped(text, current));
      current = [];
    };
    for (const line of lines) {
      const content = text.slice(line.start, line.end);
      if (FENCE.test(content)) {
        fenced = !fenced;
        end();
        continue;
      }
      if (fenced || !content || !WORDISH.test(content) || line.indent - base >= 4 || DIRECTIVE.test(content) || CODELIKE.test(content)) {
        end();
        continue;
      }
      if (BULLET.test(content)) end();
      current.push(line);
    }
    end();
    return found;
  };

  const rowsOf = (text) => {
    const starts = [0];
    for (let index = text.indexOf("\n"); index >= 0; index = text.indexOf("\n", index + 1)) starts.push(index + 1);
    return (position) => {
      let low = 0;
      let high = starts.length - 1;
      while (low < high) {
        const middle = (low + high + 1) >> 1;
        if (starts[middle] <= position) low = middle;
        else high = middle - 1;
      }
      return low;
    };
  };

  const paragraphs = (text, comments) => {
    const found = [];
    const rowOf = rowsOf(text);
    let group = null;
    const close = () => {
      if (group) found.push(...split(text, group.lines));
      group = null;
    };
    for (const comment of comments) {
      const lineStart = text.lastIndexOf("\n", comment.start - 1) + 1;
      const standalone = !text.slice(lineStart, comment.start).trim();
      const head = text.slice(comment.start, Math.min(comment.end, comment.start + 12));
      const opener = BLOCK_OPEN.exec(head);
      if (opener) {
        close();
        found.push(...split(text, blockLines(text, comment, opener[0])));
        continue;
      }
      const marker = (LINE_MARK.exec(head) || [""])[0];
      const column = comment.start - lineStart;
      const row = rowOf(comment.start);
      const continues = group && standalone && group.standalone && group.marker === marker && group.column === column && row === group.row + 1;
      if (!continues) {
        close();
        group = { marker, column, standalone, row, lines: [] };
      }
      group.row = row;
      group.lines.push(lineContent(text, comment.start + marker.length, comment.end));
      if (!standalone) close();
    }
    close();
    return found;
  };

  const hintOf = (element) => {
    const named = LANGUAGE_CLASS.exec(classOf(element));
    if (named) return named[1];
    return element.getAttribute("data-lang") || element.getAttribute("data-language") || "";
  };

  const familyNamed = (name) => {
    const lower = String(name || "").toLowerCase();
    return LANGUAGES.get(lower) || LANGUAGES.get(lower.split("-")[0]) || "";
  };

  const familyOf = (root) => {
    for (const element of [root, root.querySelector("code, pre, [data-lang], [data-language]"), root.parentElement]) {
      if (!element) continue;
      const found = familyNamed(hintOf(element));
      if (found) return found;
    }
    const diff = /^Diff for:\s*(\S+)/.exec(root.getAttribute("aria-label") || "");
    const base = (diff ? diff[1] : location.pathname).split("/").pop().toLowerCase();
    const extension = base.includes(".") ? base.slice(base.lastIndexOf(".") + 1) : base;
    if (!extension || PAGE_EXTENSIONS.has(extension)) return "unknown";
    return familyNamed(extension) || "unknown";
  };

  const read = (root) => {
    let text = "";
    const slots = [];
    const marks = [];
    let highlighted = false;
    const separate = () => {
      if (text && text[text.length - 1] !== "\n") text += "\n";
    };
    const slot = (node, attribute, original) => {
      slots.push({ node, attribute, start: text.length, original, written: null, edits: new Map() });
      text += original;
    };
    const visit = (node, inside) => {
      for (let child = node.firstChild; child; child = child.nextSibling) {
        if (child.nodeType === Node.TEXT_NODE) {
          if (child.data) slot(child, false, child.data);
          continue;
        }
        if (child.nodeType !== Node.ELEMENT_NODE) continue;
        const tag = child.localName;
        if (tag === "br") {
          text += "\n";
          continue;
        }
        if (NON_TEXT_TAGS.has(tag) || (child.classList.length && child.matches(REGION_GUTTERS))) continue;
        const isBlock = CODE_BLOCKS.has(tag);
        if (isBlock) separate();
        let mark = null;
        if (child.classList.length) {
          if (!highlighted && HIGHLIGHTED.test(classOf(child))) highlighted = true;
          if (!inside) {
            if (child.matches(COMMENT_TOKENS)) mark = { start: text.length, doc: false };
            else if (child.matches(STRING_TOKENS)) mark = { start: text.length, doc: true };
          }
        }
        if (child.hasAttribute("data-code-text")) slot(child, true, child.getAttribute("data-code-text"));
        visit(child, inside || mark !== null);
        if (mark) {
          mark.end = text.length;
          marks.push(mark);
        }
        if (isBlock) separate();
      }
    };
    visit(root, false);
    return { root, text, slots, marks, highlighted, units: new Set() };
  };

  const prosePre = (root) => {
    if (root.localName !== "pre" || root.hasAttribute("class") || root.querySelector("[class]")) return false;
    const text = root.textContent || "";
    const letters = letterCount(text);
    if (letters < 400) return false;
    const symbols = (text.match(/[{}[\]();=<>$|&*\\#@]/g) || []).length;
    const sentences = (text.match(/[.!?。！？](?:\s|$)/g) || []).length;
    return symbols < letters * 0.02 && sentences >= 4;
  };

  const slotValue = (slot) => (slot.attribute ? slot.node.getAttribute("data-code-text") : slot.node.data);

  const setSlot = (slot, value) => {
    if (slot.attribute) slot.node.setAttribute("data-code-text", value);
    else slot.node.data = value;
  };

  const rewrite = (slot) => {
    const expected = slot.written === null ? slot.original : slot.written;
    if (slotValue(slot) !== expected) return false;
    let value = slot.original;
    const edits = [];
    for (const list of slot.edits.values()) edits.push(...list);
    edits.sort((left, right) => right.from - left.from);
    for (const edit of edits) value = value.slice(0, edit.from) + edit.text + value.slice(edit.to);
    if (value !== slotValue(slot)) setSlot(slot, value);
    slot.written = edits.length ? value : null;
    return true;
  };

  const slotsIn = (slots, start, end) => {
    let low = 0;
    let high = slots.length;
    while (low < high) {
      const middle = (low + high) >> 1;
      if (slots[middle].start + slots[middle].original.length <= start) low = middle + 1;
      else high = middle;
    }
    const found = [];
    for (let index = low; index < slots.length && slots[index].start < end; index++) {
      if (slots[index].original.length) found.push(slots[index]);
    }
    return found;
  };

  const textOf = (output) => output.nodes.join("");

  const plain = (output) => Boolean(output) && Array.isArray(output.nodes) && output.nodes.every((node) => typeof node === "string");

  const fits = (level, outs, seen) => {
    if (!Array.isArray(outs)) return false;
    for (const out of outs) {
      if (typeof out === "string") continue;
      if (!out || typeof out !== "object" || !Number.isInteger(out.i)) return false;
      const item = level.ids.get(out.i);
      if (!item || seen.has(out.i)) return false;
      seen.add(out.i);
      if (out.nodes !== undefined) {
        if (!item.items || !fits(item, out.nodes, seen)) return false;
      }
    }
    return true;
  };

  const accepts = (unit, output) => {
    if (!output || typeof output !== "object") return false;
    if (unit.kind === FLOW) return fits(unit.model, output.nodes, new Set());
    return plain(output);
  };

  const setText = (node, value, record) => {
    if (node.data === value) return;
    record.texts.push({ node, original: node.data, written: value });
    node.data = value;
  };

  const sameOrder = (left, right) => left.length === right.length && left.every((node, index) => node === right[index]);

  const arrange = (level, desired, created, record) => {
    const parent = level.node;
    const nodes = level.range ? level.range.slice() : Array.from(parent.childNodes);
    const fresh = new Set(created);
    const existing = desired.filter((node) => !fresh.has(node));
    const wanted = new Set(existing);
    const current = nodes.filter((node) => wanted.has(node));
    const same = sameOrder(current, existing);
    if (same && !created.length) return;
    const anchor = nodes.length ? nodes[nodes.length - 1].nextSibling : null;
    record.orders.push({ parent, nodes, anchor });
    if (same) {
      desired.forEach((node, index) => {
        if (!fresh.has(node)) return;
        const previous = index ? desired[index - 1] : null;
        parent.insertBefore(node, previous ? previous.nextSibling : existing[0] || anchor);
      });
      return;
    }
    for (const node of desired) parent.insertBefore(node, anchor);
  };

  const place = (level, outs, record) => {
    const order = [];
    const used = new Set();
    let pending = null;
    for (const out of outs) {
      if (typeof out === "string") {
        pending = pending === null ? out : pending + out;
        continue;
      }
      if (pending !== null) order.push(pending);
      pending = null;
      const item = level.ids.get(out.i);
      used.add(item);
      order.push(item);
      if (item.items && out.nodes !== undefined) place(item, out.nodes, record);
    }
    if (pending !== null) order.push(pending);
    const created = [];
    let next = 0;
    const desired = order.map((token) => {
      if (typeof token !== "string") return token.node;
      let node = level.texts[next++];
      if (!node) {
        node = document.createTextNode("");
        created.push(node);
        record.created.push(node);
        run.owners.set(node, record.unit);
        record.unit.nodes.push(node);
      }
      setText(node, token, record);
      return node;
    });
    for (; next < level.texts.length; next++) setText(level.texts[next], "", record);
    for (const item of level.items) if (!item.text && !used.has(item)) desired.push(item.node);
    arrange(level, desired, created, record);
  };

  const stripIds = (copy) => {
    if (copy.nodeType !== Node.ELEMENT_NODE) return copy;
    copy.removeAttribute("id");
    for (const inner of copy.querySelectorAll("[id]")) inner.removeAttribute("id");
    return copy;
  };

  const render = (level, outs) => {
    const fragment = document.createDocumentFragment();
    for (const out of outs) {
      if (typeof out === "string") {
        fragment.append(out);
        continue;
      }
      const item = level.ids.get(out.i);
      if (UNCLONED.has(item.node.localName)) continue;
      if (item.items && out.nodes !== undefined) {
        const copy = stripIds(item.node.cloneNode(false));
        copy.append(render(item, out.nodes));
        fragment.append(copy);
      } else {
        fragment.append(stripIds(item.node.cloneNode(true)));
      }
    }
    return fragment;
  };

  const applyFlow = (unit) => {
    const record = { unit, texts: [], created: [], orders: [], box: null };
    unit.applied = record;
    if (run.display === "bilingual" && unit.prose) {
      const box = document.createElement("rikka-translation");
      box.setAttribute("translate", "no");
      box.setAttribute("class", "notranslate");
      box.setAttribute("lang", run.to);
      box.style.display = "block";
      box.style.marginTop = ".25em";
      box.style.opacity = ".9";
      if (unit.request.pre) box.style.whiteSpace = "pre-wrap";
      box.append(render(unit.model, unit.output.nodes));
      const last = unit.model.range[unit.model.range.length - 1];
      last.parentNode.insertBefore(box, last.nextSibling);
      record.box = box;
      return;
    }
    place(unit.model, unit.output.nodes, record);
  };

  const restoreOrder = (order) => {
    if (!order.nodes.every((node) => node.parentNode === order.parent)) return;
    const members = new Set(order.nodes);
    const current = Array.from(order.parent.childNodes).filter((node) => members.has(node));
    if (sameOrder(current, order.nodes)) return;
    const anchor = order.anchor && order.anchor.parentNode === order.parent && !members.has(order.anchor) ? order.anchor : current[current.length - 1].nextSibling;
    for (const node of order.nodes) order.parent.insertBefore(node, anchor);
  };

  const unapplyFlow = (record) => {
    if (record.box) {
      record.box.remove();
      return;
    }
    const written = new Map(record.texts.map((text) => [text.node, text.written]));
    for (const node of record.created) {
      if (node.parentNode && node.data === written.get(node)) node.remove();
    }
    for (const text of record.texts) {
      if (text.node.data === text.written) text.node.data = text.original;
    }
    for (const order of record.orders.reverse()) restoreOrder(order);
  };

  const applyComment = (unit) => {
    const region = unit.region;
    const target = textOf(unit.output).replace(SPACES, " ").trim();
    const chunks = spread(target, unit.lines.map((line) => widthOf(region.text.slice(line.start, line.end))), run.to);
    const edits = new Map();
    unit.lines.forEach((line, index) => {
      let first = true;
      for (const slot of slotsIn(region.slots, line.start, line.end)) {
        const from = Math.max(line.start, slot.start) - slot.start;
        const to = Math.min(line.end, slot.start + slot.original.length) - slot.start;
        if (!edits.has(slot)) edits.set(slot, []);
        edits.get(slot).push({ from, to, text: first ? chunks[index] : "" });
        first = false;
      }
    });
    unit.applied = { slots: [...edits.keys()] };
    for (const [slot, list] of edits) slot.edits.set(unit, list);
    for (const slot of edits.keys()) {
      if (!rewrite(slot)) run.dirty.add(region.root);
    }
    if (run.dirty.size) schedule();
  };

  const unapplyComment = (unit, record) => {
    for (const slot of record.slots) {
      slot.edits.delete(unit);
      rewrite(slot);
    }
  };

  const apply = (unit) => {
    if (unit.kind === FLOW) applyFlow(unit);
    else if (unit.kind === COMMENT) applyComment(unit);
    else {
      const value = textOf(unit.output);
      if (unit.kind === ATTRIBUTE) unit.element.setAttribute(unit.name, value);
      else unit.node.data = value;
      unit.applied = { written: value };
    }
  };

  const unapply = (unit) => {
    const record = unit.applied;
    if (!record) return;
    unit.applied = null;
    if (unit.kind === FLOW) unapplyFlow(record);
    else if (unit.kind === COMMENT) unapplyComment(unit, record);
    else if (unit.kind === ATTRIBUTE) {
      if (unit.element.getAttribute(unit.name) === record.written) unit.element.setAttribute(unit.name, unit.original);
    } else if (unit.node.data === record.written) {
      unit.node.data = unit.original;
    }
  };

  const rectOf = (unit) => {
    if (unit.slot) {
      if (unit.slot.attribute) return unit.slot.node.getBoundingClientRect();
      const range = document.createRange();
      const node = unit.slot.node;
      const offset = Math.min(Math.max(unit.lines[0].start - unit.slot.start, 0), node.length);
      range.setStart(node, offset);
      range.setEnd(node, Math.min(offset + 1, node.length));
      return range.getBoundingClientRect();
    }
    return unit.root ? unit.root.getBoundingClientRect() : null;
  };

  const rankOf = (unit) => {
    if (!unit.root) return 0;
    const rect = rectOf(unit);
    const height = innerHeight;
    if (rect.bottom >= 0 && rect.top <= height) return 0;
    if (rect.bottom >= -height && rect.top <= 2 * height) return 1;
    return 2;
  };

  const track = (unit) => {
    const current = run;
    if (!unit.root) {
      current.eligible.add(unit);
      return;
    }
    let targets = current.targets.get(unit.root);
    if (!targets) {
      targets = new Set();
      current.targets.set(unit.root, targets);
      current.sight.observe(unit.root);
    }
    targets.add(unit);
    if (current.near.get(unit.root)) current.eligible.add(unit);
  };

  const remember = (current, key, output) => {
    current.cache.delete(key);
    current.cache.set(key, output);
    if (current.cache.size > CACHE_SIZE) current.cache.delete(current.cache.keys().next().value);
  };

  const unitOf = (fields) => {
    const current = run;
    const unit = Object.assign(fields, { seq: current.seq++, state: WAITING, output: null, applied: null });
    unit.key = JSON.stringify(unit.request);
    current.units.add(unit);
    const cached = current.cache.get(unit.key);
    if (cached) {
      unit.output = cached;
      unit.state = DONE;
      apply(unit);
      return unit;
    }
    if (current.failed.has(unit.key)) {
      unit.state = DROPPED;
      return unit;
    }
    track(unit);
    return unit;
  };

  const discardUnit = (unit) => {
    if (unit.state === DISCARDED) return;
    const current = run;
    unit.state = DISCARDED;
    current.units.delete(unit);
    current.eligible.delete(unit);
    if (unit.kind === FLOW) {
      const units = current.byContainer.get(unit.container);
      if (units) units.delete(unit);
      for (const node of unit.nodes) if (current.owners.get(node) === unit) current.owners.delete(node);
    } else if (unit.kind === ATTRIBUTE) {
      const names = current.attributes.get(unit.element);
      if (names && names.get(unit.name) === unit) names.delete(unit.name);
    } else if (unit.kind === COMMENT) {
      unit.region.units.delete(unit);
    } else if (current.title === unit) {
      current.title = null;
    }
    if (unit.root) {
      const targets = current.targets.get(unit.root);
      if (targets) {
        targets.delete(unit);
        if (!targets.size) {
          current.targets.delete(unit.root);
          current.near.delete(unit.root);
          current.sight.unobserve(unit.root);
        }
      }
    }
  };

  const release = (node) => {
    const owner = run.owners.get(node);
    if (!owner) return;
    unapply(owner);
    discardUnit(owner);
  };

  const attributeUnit = (element, name) => {
    const current = run;
    let names = current.attributes.get(element);
    if (names && names.has(name)) return;
    const value = element.getAttribute(name);
    if (!value || value.length > UNIT_LIMIT || !LETTER.test(value)) return;
    if (!names) {
      names = new Map();
      current.attributes.set(element, names);
    }
    names.set(name, unitOf({ kind: ATTRIBUTE, element, name, root: element, original: value, chars: value.length, request: { pre: false, nodes: [value] } }));
  };

  const collect = (element) => {
    for (const name of ATTRIBUTES) if (element.hasAttribute(name)) attributeUnit(element, name);
    if (element.localName === "input" && BUTTONISH.test(element.type) && element.hasAttribute("value")) attributeUnit(element, "value");
  };

  const keepText = (node, kind) => (kind === OPAQUE_INLINE ? "" : (node.textContent || "").replace(SPACES, " ").trim().slice(0, KEEP_CHARS));

  const keepItem = (node, kind) => {
    release(node);
    if (kind === OPAQUE_INLINE) collect(node);
    return { node, keep: keepText(node, kind) };
  };

  const inline = (element, depth) => {
    if (depth > DEPTH) return keepItem(element, KEEP);
    release(element);
    const items = [];
    for (let child = element.firstChild; child; child = child.nextSibling) {
      if (child.nodeType === Node.TEXT_NODE) {
        release(child);
        items.push({ text: child, node: child });
        continue;
      }
      if (child.nodeType !== Node.ELEMENT_NODE) continue;
      const kind = kindOf(child);
      if (kind === IGNORE) continue;
      if (kind === KEEP || kind === OPAQUE_INLINE) items.push(keepItem(child, kind));
      else if (kind === BREAK) items.push(keepItem(child, OPAQUE_INLINE));
      else if (kind === INLINE) {
        const model = inline(child, depth + 1);
        if (!model) return null;
        items.push(model);
      } else {
        return null;
      }
    }
    collect(element);
    return { node: element, items };
  };

  const index = (level, counter, nodes, parts) => {
    level.ids = new Map();
    level.texts = [];
    const request = [];
    for (const item of level.items) {
      nodes.push(item.node);
      if (item.text) {
        level.texts.push(item.text);
        request.push(item.text.data);
        parts.push(item.text.data);
        continue;
      }
      item.id = counter.next++;
      level.ids.set(item.id, item);
      if (item.items) {
        request.push({ i: item.id, nodes: index(item, counter, nodes, parts) });
      } else {
        request.push({ i: item.id, keep: item.keep });
        counter.kept += item.keep.length;
      }
    }
    return request;
  };

  const hostOf = (container) => (container.nodeType === Node.ELEMENT_NODE ? container : container.host);

  const flowUnit = (container, items, first, last) => {
    const current = run;
    const range = [];
    for (let node = first; node; node = node.nextSibling) {
      range.push(node);
      if (node === last) break;
    }
    const model = { node: container, items, range };
    const nodes = [];
    const parts = [];
    const counter = { next: 0, kept: 0 };
    const request = index(model, counter, nodes, parts);
    const text = parts.join("");
    const chars = text.length + counter.kept;
    if (!LETTER.test(text) || chars > UNIT_LIMIT) return;
    const element = hostOf(container);
    const pre = text.includes("\n") && preserving(element);
    const letters = letterCount(text);
    const tag = container.nodeType === Node.ELEMENT_NODE ? container.localName : "";
    const prose = !element.closest(NAVIGATION) && (PROSE.has(tag) ? letters >= 8 : CELLS.has(tag) ? letters >= 30 : letters >= 40);
    const unit = { kind: FLOW, container, root: element, model, nodes, prose, chars, request: { pre, nodes: request } };
    for (const node of nodes) current.owners.set(node, unit);
    let units = current.byContainer.get(container);
    if (!units) {
      units = new Set();
      current.byContainer.set(container, units);
    }
    units.add(unit);
    unitOf(unit);
  };

  const commentUnit = (region, lines) => {
    const text = lines.map((line) => region.text.slice(line.start, line.end)).join(" ");
    const slot = slotsIn(region.slots, lines[0].start, lines[0].end)[0];
    if (!slot) return;
    const anchor = slot.attribute ? slot.node : slot.node.parentElement;
    const ranged = !slot.attribute && (anchor === region.root || anchor.localName === "code" || anchor.localName === "pre");
    const unit = unitOf({ kind: COMMENT, region, lines, root: ranged ? region.root : anchor, slot: ranged ? slot : null, chars: text.length, request: { pre: false, nodes: [text] } });
    region.units.add(unit);
  };

  const processRegion = (root) => {
    const current = run;
    if (current.code === "skip") {
      current.regions.set(root, null);
      return;
    }
    if (prosePre(root)) {
      scan(root, false);
      return;
    }
    const region = read(root);
    current.regions.set(root, region);
    if (region.text.length > REGION_CHARS) return;
    const found = fromMarks(region.text, region.marks);
    const comments = found.length || region.highlighted ? found : lex(region.text, familyOf(root));
    for (const lines of paragraphs(region.text, comments)) commentUnit(region, lines);
  };

  const observe = (root) => {
    const observer = new MutationObserver(enqueueRecords);
    observer.observe(root, { childList: true, characterData: true, subtree: true, attributes: true, attributeFilter: OBSERVED_ATTRIBUTES });
    run.observers.push(observer);
  };

  const scan = (container, again) => {
    const current = run;
    current.scanned.add(container);
    if (container.nodeType === Node.ELEMENT_NODE) collect(container);
    let items = null;
    let chars = 0;
    let first = null;
    let last = null;
    const flush = () => {
      if (items) flowUnit(container, items, first, last);
      items = null;
      chars = 0;
    };
    const add = (item, length) => {
      if (!items) {
        items = [];
        first = item.node;
      }
      items.push(item);
      chars += length;
      last = item.node;
    };
    const descend = (element) => {
      if (!(again && current.scanned.has(element))) scan(element, again);
    };
    for (let node = container.firstChild; node; node = node.nextSibling) {
      if (node.nodeType === Node.TEXT_NODE) {
        release(node);
        if (items && chars + node.data.length > UNIT_CHARS && LETTER.test(node.data)) flush();
        add({ text: node, node }, node.data.length);
        continue;
      }
      if (node.nodeType !== Node.ELEMENT_NODE) continue;
      const kind = kindOf(node);
      if (kind === IGNORE) continue;
      if (kind === KEEP || kind === OPAQUE_INLINE) {
        const item = keepItem(node, kind);
        add(item, item.keep.length);
        continue;
      }
      if (kind === INLINE) {
        const model = inline(node, 1);
        if (model) {
          const length = (node.textContent || "").length;
          if (items && chars + length > UNIT_CHARS) flush();
          add(model, length);
          continue;
        }
        flush();
        descend(node);
        continue;
      }
      flush();
      if (kind === SKIP) {
        if (OPAQUE.has(node.localName)) collect(node);
      } else if (kind === REGION) {
        if (!current.regions.has(node) && !current.scanned.has(node)) processRegion(node);
      } else if (kind === BLOCK) {
        descend(node);
      } else if (kind === HOST) {
        descend(node);
        if (!current.scanned.has(node.shadowRoot)) {
          observe(node.shadowRoot);
          scan(node.shadowRoot, again);
        }
      }
    }
    flush();
  };

  const titleNode = () => {
    const element = document.querySelector("title");
    if (!element || element.childNodes.length !== 1 || element.firstChild.nodeType !== Node.TEXT_NODE) return null;
    return element.firstChild;
  };

  const retitle = () => {
    const current = run;
    const node = titleNode();
    const unit = current.title;
    if (unit) {
      const expected = unit.applied ? unit.applied.written : unit.original;
      if (unit.node === node && node.data === expected) return;
      if (unit.node === node && unit.applied) unapply(unit);
      discardUnit(unit);
    }
    if (!node || !LETTER.test(node.data) || node.data.length > UNIT_LIMIT) return;
    current.title = unitOf({ kind: TITLE, node, root: null, original: node.data, chars: node.data.length, request: { pre: false, nodes: [node.data] } });
  };

  const enqueueRecords = (records) => {
    const current = run;
    if (!current || !records.length) return;
    for (const record of records) current.records.push(record);
    schedule();
  };

  const schedule = () => {
    const current = run;
    if (current && !current.timer) current.timer = setTimeout(processMutations, MUTATION_DEBOUNCE_MS);
  };

  const withoutObserving = (work) => {
    const current = run;
    for (const observer of current.observers) enqueueRecords(observer.takeRecords());
    try {
      work();
    } finally {
      if (run === current) for (const observer of current.observers) observer.takeRecords();
    }
  };

  const placeOf = (node) => {
    let candidate = null;
    let skipped = false;
    for (let current = node.nodeType === Node.ELEMENT_NODE ? node : node.parentNode; current; ) {
      if (current.nodeType === Node.DOCUMENT_FRAGMENT_NODE) {
        if (!candidate) candidate = current;
        current = current.host;
        continue;
      }
      if (current.nodeType !== Node.ELEMENT_NODE) return null;
      if (run.regions.has(current)) return { region: current };
      const tag = current.localName;
      if (tag === "rikka-translation" || INVISIBLE.has(tag) || OPAQUE.has(tag) || isExcluded(current) || (current.classList.length && current.matches(GUTTERS))) skipped = true;
      if (!candidate && (run.scanned.has(current) || !flows(current))) candidate = current;
      if (current === document.body) return skipped || !candidate ? null : { container: candidate };
      current = current.parentNode;
    }
    return null;
  };

  const withinChurnLimit = (target) => {
    const now = Date.now();
    let churn = run.churn.get(target);
    if (!churn || now - churn.since > CHURN_MS) {
      churn = { since: now, count: 0 };
      run.churn.set(target, churn);
    }
    churn.count++;
    return churn.count <= CHURN_LIMIT;
  };

  const rescan = (container) => {
    if (!container.isConnected) return;
    const units = run.byContainer.get(container);
    if (units) {
      for (const unit of [...units]) {
        unapply(unit);
        discardUnit(unit);
      }
    }
    scan(container, true);
  };

  const rebuild = (root) => {
    const region = run.regions.get(root);
    if (region) {
      for (const unit of [...region.units]) {
        unapply(unit);
        discardUnit(unit);
      }
    }
    run.regions.delete(root);
    if (root.isConnected && placeOf(root.parentNode || root)) processRegion(root);
  };

  const attributeChanged = (element, name) => {
    const names = run.attributes.get(element);
    const unit = names && names.get(name);
    const value = element.getAttribute(name);
    if (unit) {
      if (value === (unit.applied ? unit.applied.written : unit.original)) return;
      unit.applied = null;
      discardUnit(unit);
    }
    if (name === "value" && !(element.localName === "input" && BUTTONISH.test(element.type))) return;
    const found = placeOf(element);
    if (found && found.container) attributeUnit(element, name);
  };

  const dropDetachedUnits = () => {
    for (const unit of [...run.units]) {
      if (unit.root && !unit.root.isConnected) discardUnit(unit);
    }
  };

  const processMutations = () => {
    const current = run;
    if (!current) return;
    current.timer = 0;
    withoutObserving(() => {
      const records = current.records;
      current.records = [];
      const parents = new Set();
      const regions = new Set(current.dirty);
      current.dirty.clear();
      for (const record of records) {
        if (record.type === "attributes") {
          if (record.attributeName === "data-code-text") parents.add(record.target);
          else attributeChanged(record.target, record.attributeName);
        } else if (record.type === "characterData") {
          parents.add(record.target.parentNode);
        } else {
          parents.add(record.target);
        }
      }
      dropDetachedUnits();
      const containers = new Set();
      for (const parent of parents) {
        if (!parent || !parent.isConnected) continue;
        const found = placeOf(parent);
        if (!found) continue;
        if (found.region) regions.add(found.region);
        else containers.add(found.container);
      }
      for (const container of containers) if (withinChurnLimit(container)) rescan(container);
      for (const root of regions) if (withinChurnLimit(root)) rebuild(root);
      if (TOP) retitle();
    });
    pump();
  };

  const send = (current, batch) => {
    current.inflight++;
    for (const unit of batch) {
      unit.state = SENT;
      current.eligible.delete(unit);
    }
    const settled = (outputs) => {
      current.inflight--;
      if (run !== current) return;
      withoutObserving(() => {
        batch.forEach((unit, position) => {
          const output = outputs && outputs[position];
          const usable = output && accepts(unit, output);
          if (usable) remember(current, unit.key, output);
          if (unit.state !== SENT) return;
          if (!usable) {
            unit.state = DROPPED;
            current.failed.add(unit.key);
            if (current.failed.size > CACHE_SIZE) current.failed.delete(current.failed.values().next().value);
            return;
          }
          unit.output = output;
          unit.state = DONE;
          apply(unit);
        });
      });
      pump();
    };
    post({ kind: "translate", session: current.session, units: batch.map((unit) => unit.request) }).then(
      (reply) => settled(reply && Array.isArray(reply.units) && reply.units.length === batch.length ? reply.units : null),
      () => settled(null),
    );
  };

  const pump = () => {
    const current = run;
    if (!current) return;
    while (current.inflight < IN_FLIGHT && current.eligible.size) {
      const ready = [];
      for (const unit of current.eligible) {
        if (unit.state !== WAITING) {
          current.eligible.delete(unit);
          continue;
        }
        const rank = rankOf(unit);
        if (rank < 2) ready.push({ rank, unit });
      }
      if (!ready.length) return;
      ready.sort((left, right) => left.rank - right.rank || left.unit.seq - right.unit.seq);
      const batch = [];
      let chars = 0;
      for (const { unit } of ready) {
        if (batch.length === BATCH_UNITS || (batch.length && chars + unit.chars > BATCH_CHARS)) break;
        batch.push(unit);
        chars += unit.chars;
      }
      send(current, batch);
    }
  };

  const onIntersection = (entries) => {
    const current = run;
    if (!current) return;
    for (const entry of entries) {
      current.near.set(entry.target, entry.isIntersecting);
      const targets = current.targets.get(entry.target);
      if (!targets) continue;
      for (const unit of targets) {
        if (entry.isIntersecting && unit.state === WAITING) current.eligible.add(unit);
        else current.eligible.delete(unit);
      }
    }
    pump();
  };

  const schedulePump = () => {
    const current = run;
    if (!current || current.pumpTimer) return;
    current.pumpTimer = setTimeout(() => {
      current.pumpTimer = 0;
      if (run === current) pump();
    }, PUMP_DELAY_MS);
  };

  const start = (session, to, display, code) => {
    run = {
      session,
      to,
      target: primary(to),
      display,
      code,
      seq: 0,
      units: new Set(),
      eligible: new Set(),
      inflight: 0,
      cache: new Map(),
      failed: new Set(),
      owners: new WeakMap(),
      byContainer: new WeakMap(),
      attributes: new WeakMap(),
      regions: new WeakMap(),
      scanned: new WeakSet(),
      targets: new Map(),
      near: new WeakMap(),
      churn: new WeakMap(),
      displays: new WeakMap(),
      dirty: new Set(),
      records: [],
      observers: [],
      title: null,
      timer: 0,
      pumpTimer: 0,
      sight: new IntersectionObserver(onIntersection, { rootMargin: "100% 0px 100% 0px" }),
    };
    observe(document.documentElement);
    addEventListener("scroll", schedulePump, { capture: true, passive: true });
    withoutObserving(() => {
      if (document.body) scan(document.body, false);
      if (TOP) retitle();
    });
    pump();
  };

  const stop = () => {
    const current = run;
    if (!current) return;
    for (const observer of current.observers) observer.disconnect();
    current.sight.disconnect();
    clearTimeout(current.timer);
    clearTimeout(current.pumpTimer);
    removeEventListener("scroll", schedulePump, { capture: true });
    for (const unit of current.units) unapply(unit);
    run = null;
  };

  const redisplay = (display) => {
    const current = run;
    withoutObserving(() => {
      const prose = [...current.units].filter((unit) => unit.kind === FLOW && unit.prose && unit.applied);
      for (const unit of prose) unapply(unit);
      current.display = display;
      for (const unit of prose) apply(unit);
    });
  };

  const handle = (command) => {
    if (!Number.isSafeInteger(command.session) || typeof command.to !== "string" || !command.to) return false;
    const display = command.display === "bilingual" ? "bilingual" : "replace";
    const code = command.code === "skip" ? "skip" : "comments";
    if (run && run.session === command.session) {
      if (run.display !== display) redisplay(display);
      return true;
    }
    stop();
    start(command.session, command.to, display, code);
    return true;
  };

  const shown = () => (run ? { session: run.session, display: run.display } : null);

  return { handle, stop, shown };
})
