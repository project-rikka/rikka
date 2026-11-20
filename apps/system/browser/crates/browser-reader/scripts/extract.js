(() => {
  "use strict";

  const BUDGET_MS = 2500;
  const MAX_ELEMENTS = 200000;
  const MAX_BLOCKS = 6000;
  const MAX_TEXT = 1000000;
  const MAX_NESTING = 6;
  const MAX_DOM_DEPTH = 400;
  const MAX_IMAGES = 600;
  const MAX_ROWS = 800;
  const MAX_COLUMNS = 48;
  const MAX_INLINES = 15000;
  const MAX_CODE = 200000;
  const MIN_MASS = 450;
  const TIGHTNESS = 0.97;
  const IDEAL_IMAGE_WIDTH = 1400;
  const TINY_IMAGE = 48;

  const SKIPPED = 1;
  const FLIPPED = 2;
  const NEGATIVE = 4;
  const POSITIVE = 8;
  const STRONG = 16;
  const DROPPED = 32;
  const BODY_TEXT = 64;
  const BURIED = 128;

  const IDEOGRAPHIC = /[\u3040-\u30ff\u31f0-\u31ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9d]/g;
  const KANA = /[\u3040-\u30ff]/g;
  const HAN = /[\u3400-\u4dbf\u4e00-\u9fff]/g;
  const HANGUL = /[\uac00-\ud7af]/g;
  const SENTENCE = /[.,;:!?\u3001\u3002\uff0c\uff0e\uff01\uff1f\uff1b\uff1a]/g;
  const SEGMENT_BREAK = /([\u3000-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uff00-\uffef])[ \t\r]*\n[ \t\r\n]*(?=[\u3000-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uff00-\uffef])/g;
  const COLLAPSIBLE = /[ \t\n\r\f\u00a0]+/g;
  const INVISIBLE_CHARACTERS = /[\u200b\ufeff\u00ad]/g;

  const INVISIBLE = new Set(["script", "style", "noscript", "template", "link", "meta", "head", "title", "object", "embed", "canvas", "svg", "audio", "video", "map", "area", "input", "select", "textarea", "option", "optgroup", "button", "datalist", "dialog", "slot", "frameset", "frame", "progress", "meter"]);
  const BOILERPLATE_TAGS = new Set(["nav", "aside", "footer", "header", "form", "menu", "fieldset"]);
  const BLOCKS = new Set(["address", "article", "aside", "blockquote", "body", "caption", "center", "dd", "details", "dialog", "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup", "html", "li", "main", "menu", "nav", "ol", "p", "pre", "section", "summary", "table", "tbody", "td", "tfoot", "th", "thead", "tr", "ul"]);
  const ROOTS = new Set(["article", "main", "section", "div", "td", "body", "center", "p", "blockquote", "font"]);
  const LINK_HEAVY_CANDIDATES = new Set(["div", "section", "ul", "ol", "p", "li", "dl", "table", "header", "footer", "aside", "nav"]);
  const NEGATIVE_WORDS = new Set(["comment", "comments", "commentlist", "disqus", "respond", "replies", "reply", "discussion", "sidebar", "side", "widget", "widgets", "related", "recommend", "recommended", "recommendation", "recommendations", "ranking", "rankings", "popular", "trending", "pickup", "share", "sharing", "sharebar", "social", "sns", "follow", "footer", "masthead", "navigation", "nav", "navbar", "menu", "breadcrumb", "breadcrumbs", "pagination", "pager", "ad", "ads", "adv", "advert", "adverts", "advertisement", "advertising", "banner", "promo", "promotion", "sponsor", "sponsored", "newsletter", "subscribe", "subscription", "signup", "paywall", "cookie", "cookies", "consent", "gdpr", "popup", "modal", "overlay", "toolbar", "tags", "taglist", "byline", "dateline", "meta", "outbrain", "taboola", "teads", "zergnet", "hidden", "print", "skip"]);
  const STRONG_WORDS = new Set(["comment", "comments", "commentlist", "disqus", "respond", "replies", "discussion", "outbrain", "taboola", "zergnet", "paywall", "cookie", "consent"]);
  const POSITIVE_WORDS = new Set(["article", "articlebody", "content", "entry", "post", "story", "text", "main", "body", "blog", "prose", "markdown", "honbun", "kiji", "news", "detail", "column", "hentry"]);
  const SCREEN_READER_WORDS = new Set(["sr-only", "visually-hidden", "visuallyhidden", "screen-reader-text", "screen-reader-only", "a11y-hidden", "offscreen", "assistive-text"]);
  const NEGATIVE_ROLES = new Set(["navigation", "complementary", "contentinfo", "banner", "search", "dialog", "alertdialog", "menu", "menubar", "toolbar", "tablist"]);
  const CAPTION_WORDS = new Set(["caption", "figcaption", "credit", "credits", "photo-caption", "image-caption", "wp-caption-text", "imagecaption", "caption-text"]);
  const LAZY_SOURCES = ["data-src", "data-original", "data-lazy-src", "data-lazy", "data-url", "data-hi-res-src", "data-full-src", "data-actualsrc", "data-echo", "data-original-src", "data-image", "data-img-src", "data-lazyload"];
  const LAZY_SRCSETS = ["data-srcset", "data-lazy-srcset", "data-original-srcset"];
  const SUPPORTED_TYPES = /^image\/(?:jpeg|jpg|png|gif|webp|avif|svg\+xml|bmp)$/i;
  const PLACEHOLDER = /(?:^|[\/_.-])(?:blank|spacer|pixel|placeholder|transparent|loading|loader|lazy|lazyload|grey|gray|dummy|1x1|clear)\.(?:gif|png|svg|jpe?g|webp)(?:$|[?#])/i;
  const NOT_LEAD = /(?:logo|default|placeholder|no[-_]?image|noimage|blank|favicon|icon|avatar|sprite)/i;
  const ARTICLE_TYPE = /^(?:Article|NewsArticle|BlogPosting|Report|TechArticle|ScholarlyArticle|OpinionNewsArticle|AnalysisNewsArticle|ReportageNewsArticle|ReviewNewsArticle|BackgroundNewsArticle|LiveBlogPosting|SocialMediaPosting|WebPage|Posting)$/;
  const NO_MARKS = Object.freeze({ key: "" });

  const started = performance.now();
  let ticks = 0;
  let stopped = false;
  let truncated = false;
  let blockCount = 0;
  let textLeft = MAX_TEXT;
  let imageCount = 0;

  const textWeight = (text) => {
    const length = text.length;
    if (length === 0) return 0;
    const rest = text.replace(IDEOGRAPHIC, "").length;
    return rest + (length - rest) * 3;
  };

  const count = (text, pattern) => {
    const found = text.match(pattern);
    return found ? found.length : 0;
  };

  const normalizeSpace = (text) => (text || "").replace(INVISIBLE_CHARACTERS, "").replace(COLLAPSIBLE, " ").trim();

  const comparable = (text) => normalizeSpace(text).toLowerCase().replace(/[\s\p{P}\p{S}]+/gu, "");

  const tick = () => {
    if ((++ticks & 255) === 0 && performance.now() - started > BUDGET_MS) {
      stopped = true;
      truncated = true;
    }
    return stopped;
  };

  const resolve = (raw) => {
    if (!raw) return null;
    const text = String(raw).trim();
    if (!text || /^(?:javascript|vbscript|about|blob):/i.test(text)) return null;
    try {
      const url = new URL(text, document.baseURI);
      if (url.protocol === "http:" || url.protocol === "https:" || url.protocol === "mailto:") return url.href;
      if (url.protocol === "data:" && /^data:image\//i.test(text)) return text;
    } catch (error) {
      return null;
    }
    return null;
  };

  const tokensOf = (element) => {
    const id = element.getAttribute("id") || "";
    const classes = element.getAttribute("class") || "";
    if (!id && !classes) return null;
    const label = `${id} ${classes}`;
    const tokens = new Set();
    for (const word of label.split(/\s+/)) {
      if (!word) continue;
      const lower = word.toLowerCase();
      tokens.add(lower);
      for (const part of word.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase().split(/[\s_\-:]+/)) {
        if (part) tokens.add(part);
      }
    }
    return tokens;
  };

  const visible = (element) => {
    if (typeof element.checkVisibility === "function") return element.checkVisibility();
    const style = getComputedStyle(element);
    return style.display !== "none" && style.visibility !== "hidden";
  };

  const notReadable = (reason) => ({ readable: false, reason });

  const measure = (body) => {
    const all = body.getElementsByTagName("*");
    if (all.length + 1 > MAX_ELEMENTS) return null;
    const elements = [body];
    for (let i = 0; i < all.length; i++) elements.push(all[i]);
    const total = elements.length;
    const index = new Map();
    for (let i = 0; i < total; i++) index.set(elements[i], i);
    const parent = new Int32Array(total);
    const state = new Uint8Array(total);
    parent[0] = -1;
    for (let i = 0; i < total; i++) {
      const element = elements[i];
      if (i > 0) {
        const up = index.get(element.parentElement);
        parent[i] = up === undefined ? 0 : up;
        if (state[parent[i]] & SKIPPED) {
          state[i] = SKIPPED;
          continue;
        }
      }
      const name = element.localName;
      if (INVISIBLE.has(name) || element.hidden || element.getAttribute("aria-hidden") === "true") {
        state[i] = SKIPPED;
        continue;
      }
      let flags = 0;
      const tokens = tokensOf(element);
      if (tokens) {
        for (const token of tokens) {
          if (SCREEN_READER_WORDS.has(token)) flags |= SKIPPED;
          if (NEGATIVE_WORDS.has(token)) flags |= NEGATIVE;
          if (STRONG_WORDS.has(token)) flags |= STRONG | NEGATIVE;
          if (POSITIVE_WORDS.has(token)) flags |= POSITIVE;
        }
      }
      const role = element.getAttribute("role");
      if (role && NEGATIVE_ROLES.has(role)) flags |= NEGATIVE;
      if (role === "main" || role === "article") flags |= POSITIVE;
      if (BOILERPLATE_TAGS.has(name)) flags |= NEGATIVE;
      const itemprop = element.getAttribute("itemprop");
      if (itemprop && /articleBody|text/i.test(itemprop)) flags |= POSITIVE | BODY_TEXT;
      if (!(flags & SKIPPED) && BLOCKS.has(name) && !visible(element)) flags |= SKIPPED;
      state[i] = flags;
    }

    const ownText = new Float64Array(total);
    const ownLinks = new Float64Array(total);
    const ownPunctuation = new Float64Array(total);
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = node.data.trim();
      if (!text) continue;
      const holder = index.get(node.parentElement);
      if (holder === undefined || state[holder] & SKIPPED) continue;
      let element = node.parentElement;
      let linked = false;
      while (element && !BLOCKS.has(element.localName)) {
        if (element.localName === "a") linked = true;
        element = element.parentElement;
      }
      const unit = element ? index.get(element) : 0;
      if (unit === undefined) continue;
      const weight = textWeight(text);
      ownText[unit] += weight;
      if (linked) ownLinks[unit] += weight;
      ownPunctuation[unit] += count(text, SENTENCE);
    }

    const good = new Float64Array(total);
    const bad = new Float64Array(total);
    for (let i = 0; i < total; i++) {
      const weight = ownText[i];
      if (weight === 0) continue;
      const linked = ownLinks[i];
      const plain = weight - linked;
      const name = elements[i].localName;
      let quality = ownPunctuation[i] > 0 ? 1 : 0.5;
      if (plain < 30) quality *= 0.25;
      else if (plain < 80) quality *= 0.6;
      good[i] = /^h[1-6]$/.test(name) ? plain * 0.3 : plain * quality;
      bad[i] = linked + (weight < 30 ? weight * 0.5 : 0);
    }

    const raw = new Float64Array(good);
    for (let i = total - 1; i > 0; i--) {
      if (state[i] & SKIPPED) raw[i] = 0;
      raw[parent[i]] += raw[i];
    }
    const mass = raw[0];

    const text = new Float64Array(ownText);
    const links = new Float64Array(ownLinks);
    for (let i = total - 1; i >= 0; i--) {
      if (state[i] & SKIPPED) {
        good[i] = bad[i] = text[i] = links[i] = 0;
        continue;
      }
      const name = elements[i].localName;
      const flags = state[i];
      const exempt = i === 0 || name === "main" || flags & BODY_TEXT;
      if (flags & NEGATIVE && !exempt && (flags & STRONG || raw[i] < mass * 0.5)) {
        bad[i] += good[i];
        good[i] = 0;
        state[i] |= FLIPPED;
      }
      if (i > 0) {
        const up = parent[i];
        good[up] += good[i];
        bad[up] += bad[i];
        text[up] += text[i];
        links[up] += links[i];
      }
    }
    for (let i = 1; i < total; i++) {
      if (state[parent[i]] & (FLIPPED | BURIED)) state[i] |= BURIED;
    }
    return { elements, index, parent, state, good, bad, text, links };
  };

  const chooseRoot = (measured) => {
    const { elements, state, good, bad } = measured;
    const score = (i) => {
      const element = elements[i];
      const name = element.localName;
      let bonus = 0;
      if (state[i] & BODY_TEXT) bonus += 0.3;
      if (state[i] & POSITIVE) bonus += 0.15;
      if (name === "article") bonus += 0.1;
      if (name === "main") bonus += 0.05;
      return good[i] * (1 + bonus) - bad[i];
    };
    let best = 0;
    let bestScore = -Infinity;
    for (let i = 0; i < elements.length; i++) {
      if (state[i] & (SKIPPED | FLIPPED | BURIED) || !ROOTS.has(elements[i].localName)) continue;
      const value = score(i);
      if (value >= bestScore) {
        best = i;
        bestScore = value;
      }
    }
    for (let descended = true; descended;) {
      descended = false;
      for (const child of elements[best].children) {
        const i = measured.index.get(child);
        if (i === undefined || state[i] & (SKIPPED | FLIPPED) || !ROOTS.has(child.localName)) continue;
        if (good[i] >= good[best] * TIGHTNESS && score(i) >= bestScore - good[best] * 0.1) {
          best = i;
          bestScore = score(i);
          descended = true;
          break;
        }
      }
    }
    return best;
  };

  const metadata = () => {
    const metas = new Map();
    for (const meta of document.querySelectorAll("meta[content]")) {
      const key = (meta.getAttribute("property") || meta.getAttribute("name") || meta.getAttribute("itemprop") || "").trim().toLowerCase();
      if (key && !metas.has(key)) metas.set(key, normalizeSpace(meta.getAttribute("content")));
    }
    return metas;
  };

  const linkedData = () => {
    const typed = (value) => {
      const type = value["@type"];
      const types = Array.isArray(type) ? type : [type];
      return types.some((each) => typeof each === "string" && ARTICLE_TYPE.test(each) && each !== "WebPage");
    };
    let page = null;
    const visit = (value, depth) => {
      if (!value || typeof value !== "object" || depth > 6) return null;
      if (Array.isArray(value)) {
        for (const each of value) {
          const found = visit(each, depth + 1);
          if (found) return found;
        }
        return null;
      }
      if (typed(value)) return value;
      if (!page && value["@type"] === "WebPage") page = value;
      if (value["@graph"]) return visit(value["@graph"], depth + 1);
      if (value.mainEntity) return visit(value.mainEntity, depth + 1);
      return null;
    };
    for (const script of document.querySelectorAll('script[type="application/ld+json"]')) {
      const source = script.textContent || "";
      if (source.length > 1000000) continue;
      try {
        const found = visit(JSON.parse(source), 0);
        if (found) return found;
      } catch (error) {
        continue;
      }
    }
    return page;
  };

  const nameOf = (value) => {
    if (!value) return "";
    if (typeof value === "string") return normalizeSpace(value);
    if (Array.isArray(value)) return value.map(nameOf).filter(Boolean).slice(0, 4).join(", ");
    if (typeof value === "object" && typeof value.name === "string") return normalizeSpace(value.name);
    return "";
  };

  const urlOf = (value) => {
    if (!value) return null;
    if (typeof value === "string") return value;
    if (Array.isArray(value)) return urlOf(value[0]);
    if (typeof value === "object") return urlOf(value.url || value.contentUrl || value["@id"]);
    return null;
  };

  const escapeExpression = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

  const stripSite = (title, site) => {
    const separator = "\\s*(?:\\||\uff5c|-|\u2013|\u2014|\u00b7|\u2022|\u00bb|\u00ab|/|::|:|\uff1a)\\s*";
    if (site) {
      const name = escapeExpression(site);
      for (const pattern of [new RegExp(`^(.+?)${separator}${name}\\s*$`, "i"), new RegExp(`^\\s*${name}${separator}(.+)$`, "i")]) {
        const found = title.match(pattern);
        if (found && textWeight(found[1].trim()) >= 6) return found[1].trim();
      }
    }
    const splitter = /\s+(?:\||-|\u2013|\u2014|\u00b7|\u2022|\u00bb|::)\s+|\s*\uff5c\s*/g;
    let last = null;
    for (let found = splitter.exec(title); found; found = splitter.exec(title)) last = found;
    if (last) {
      const head = title.slice(0, last.index).trim();
      const tail = title.slice(last.index + last[0].length).trim();
      if (textWeight(tail) <= 40 && textWeight(head) >= textWeight(tail) * 2 && textWeight(head) >= 12) return head;
    }
    return title;
  };

  const chooseTitle = (metas, ld, site) => {
    let title = normalizeSpace(ld && typeof ld.headline === "string" ? ld.headline : "") || metas.get("og:title") || metas.get("twitter:title") || normalizeSpace(document.title);
    title = stripSite(title || "", site);
    for (const heading of Array.from(document.querySelectorAll("h1")).slice(0, 6)) {
      if (!visible(heading)) continue;
      const text = normalizeSpace(heading.textContent);
      const plain = comparable(text);
      const wanted = comparable(title);
      if (plain.length >= 4 && wanted && (wanted.includes(plain) || plain.includes(wanted))) return text.slice(0, 500);
    }
    return title.slice(0, 500);
  };

  const chooseByline = (metas, ld, measured) => {
    let byline = nameOf(ld && ld.author);
    if (!byline) {
      for (const key of ["author", "article:author", "byl", "parsely-author", "sailthru.author", "dc.creator", "dcterms.creator"]) {
        const value = metas.get(key);
        if (value && !/^https?:/i.test(value)) {
          byline = value;
          break;
        }
      }
    }
    for (const selector of ['[itemprop~="author"]', '[rel="author"]', ".byline", ".author", ".post-author", ".entry-author", ".article-author", ".writer", ".author-name"]) {
      for (const element of document.querySelectorAll(selector)) {
        const i = measured.index.get(element);
        if (i === undefined || measured.state[i] & SKIPPED) continue;
        const text = normalizeSpace(element.textContent);
        if (!text || textWeight(text) > 120) continue;
        measured.state[i] |= DROPPED;
        if (!byline) {
          const name = element.querySelector('[itemprop~="name"]');
          byline = name ? normalizeSpace(name.textContent) : text;
        }
      }
    }
    return byline ? byline.slice(0, 300) : null;
  };

  const choosePublished = (metas, ld, root) => {
    const candidates = [ld && ld.datePublished, metas.get("article:published_time"), metas.get("datepublished"), metas.get("pubdate"), metas.get("publishdate"), metas.get("publish-date"), metas.get("date"), metas.get("dc.date"), metas.get("dc.date.issued"), metas.get("dcterms.created"), metas.get("sailthru.date"), metas.get("parsely-pub-date")];
    for (const candidate of candidates) {
      if (typeof candidate === "string" && normalizeSpace(candidate)) return normalizeSpace(candidate).slice(0, 64);
    }
    const scope = root.closest("article") || root;
    const time = scope.querySelector("time[datetime]") || document.querySelector("article time[datetime], main time[datetime]");
    if (time && normalizeSpace(time.getAttribute("datetime"))) return normalizeSpace(time.getAttribute("datetime")).slice(0, 64);
    const dated = document.querySelectorAll('time, [itemprop="datePublished"], [class*="date" i], [class*="time" i], [class*="publish" i]');
    for (let i = 0; i < Math.min(dated.length, 40); i++) {
      const text = normalizeSpace(dated[i].textContent);
      if (!text || text.length > 60) continue;
      const found = /(\d{4})\s*(?:年|[./-])\s*(\d{1,2})\s*(?:月|[./-])\s*(\d{1,2})/.exec(text);
      if (!found) continue;
      const [year, month, day] = [Number(found[1]), Number(found[2]), Number(found[3])];
      if (year < 1990 || month < 1 || month > 12 || day < 1 || day > 31) continue;
      return `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    }
    return null;
  };

  const chooseLanguage = (root, metas) => {
    const declared = (root.closest("[lang]") || document.documentElement).getAttribute("lang") || metas.get("content-language") || metas.get("language") || "";
    const normalized = declared.trim().replace(/_/g, "-");
    const sample = (root.textContent || "").slice(0, 4000);
    const letters = sample.replace(/[\s\p{P}\p{S}\p{N}]+/gu, "").length || 1;
    const kana = count(sample, KANA) / letters;
    const hangul = count(sample, HANGUL) / letters;
    const han = count(sample, HAN) / letters;
    const primary = normalized.split("-")[0].toLowerCase();
    let guessed = null;
    if (kana > 0.08) guessed = "ja";
    else if (hangul > 0.3) guessed = "ko";
    else if (han > 0.3) guessed = "zh";
    if (guessed && primary !== guessed && !(guessed === "zh" && primary === "ja")) return guessed;
    return /^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(normalized) ? normalized : guessed;
  };

  const parseSrcset = (text) => {
    const candidates = [];
    if (!text) return candidates;
    let position = 0;
    while (position < text.length) {
      while (position < text.length && /[\s,]/.test(text[position])) position++;
      if (position >= text.length) break;
      const start = position;
      while (position < text.length && !/\s/.test(text[position])) position++;
      let url = text.slice(start, position);
      let descriptor = "";
      if (url.endsWith(",")) {
        url = url.replace(/,+$/, "");
      } else {
        const end = text.indexOf(",", position);
        descriptor = text.slice(position, end === -1 ? text.length : end).trim();
        position = end === -1 ? text.length : end + 1;
      }
      const width = /^(\d+)w$/i.exec(descriptor);
      const density = /^(\d+(?:\.\d+)?)x$/i.exec(descriptor);
      candidates.push({ url, width: width ? Number(width[1]) : 0, density: density ? Number(density[1]) : width ? 0 : 1 });
      if (candidates.length > 64) break;
    }
    return candidates;
  };

  const pickCandidate = (candidates) => {
    const usable = candidates.filter((candidate) => !placeholder(candidate.url));
    if (!usable.length) return null;
    const wide = usable.filter((candidate) => candidate.width > 0).sort((a, b) => a.width - b.width);
    if (wide.length) return (wide.find((candidate) => candidate.width >= IDEAL_IMAGE_WIDTH) || wide[wide.length - 1]).url;
    const dense = usable.filter((candidate) => candidate.density > 0).sort((a, b) => a.density - b.density);
    if (dense.length) {
      const fitting = dense.filter((candidate) => candidate.density <= 2);
      return (fitting.length ? fitting[fitting.length - 1] : dense[0]).url;
    }
    return usable[0].url;
  };

  const placeholder = (url) => {
    if (!url) return true;
    if (/^data:/i.test(url)) return url.length < 400 || !/^data:image\//i.test(url);
    return /^about:/i.test(url) || PLACEHOLDER.test(url);
  };

  const pictureSrcset = (picture) => {
    for (const source of picture.querySelectorAll("source")) {
      const type = source.getAttribute("type");
      if (type && !SUPPORTED_TYPES.test(type)) continue;
      const media = source.getAttribute("media");
      try {
        if (media && !matchMedia(media).matches) continue;
      } catch (error) {
        continue;
      }
      const srcset = LAZY_SRCSETS.map((name) => source.getAttribute(name)).find(Boolean) || source.getAttribute("srcset");
      if (srcset) return srcset;
    }
    return null;
  };

  const dimension = (value) => {
    const number = Number.parseFloat(value);
    return Number.isFinite(number) && number >= 1 && number <= 100000 ? Math.round(number) : null;
  };

  const imageOf = (img, live) => {
    const picture = img.parentElement && img.parentElement.localName === "picture" ? img.parentElement : null;
    const srcset = LAZY_SRCSETS.map((name) => img.getAttribute(name)).find(Boolean) || img.getAttribute("srcset") || (picture && pictureSrcset(picture));
    const lazy = LAZY_SOURCES.map((name) => img.getAttribute(name)).find((value) => value && !placeholder(value.trim()));
    const current = live && img.currentSrc && !placeholder(img.currentSrc) ? img.currentSrc : null;
    const plain = img.getAttribute("src");
    const chosen = pickCandidate(parseSrcset(srcset)) || lazy || current || (!placeholder(plain) ? plain : null);
    const src = resolve(chosen);
    if (!src || /^mailto:/i.test(src)) return null;
    let width = dimension(img.getAttribute("width")) || dimension(img.getAttribute("data-width"));
    let height = dimension(img.getAttribute("height")) || dimension(img.getAttribute("data-height"));
    if ((!width || !height) && live && img.complete && img.naturalWidth > 2 && current) {
      width = img.naturalWidth;
      height = img.naturalHeight;
    }
    if (!width || !height) width = height = null;
    return { src, alt: normalizeSpace(img.getAttribute("alt")).slice(0, 300), width, height };
  };

  const isTinyImage = (img) => {
    const width = dimension(img.getAttribute("width"));
    const height = dimension(img.getAttribute("height"));
    if (width && height) return width < TINY_IMAGE && height < TINY_IMAGE;
    if (lazySource(img)) return false;
    if (img.complete && img.naturalWidth > 0) return img.naturalWidth < TINY_IMAGE && img.naturalHeight < TINY_IMAGE;
    const box = img.getBoundingClientRect();
    return box.width > 0 && box.height > 0 && box.width < TINY_IMAGE && box.height < TINY_IMAGE;
  };

  const lazySource = (img) => LAZY_SOURCES.some((name) => img.hasAttribute(name)) || LAZY_SRCSETS.some((name) => img.hasAttribute(name));

  const fallbackImages = (element) => {
    const found = [];
    for (const noscript of element.querySelectorAll("noscript")) {
      const markup = noscript.textContent || "";
      if (!/<img/i.test(markup) || markup.length > 20000) continue;
      const parsed = new DOMParser().parseFromString(markup, "text/html");
      for (const img of parsed.querySelectorAll("img")) {
        const image = imageOf(img, false);
        if (image) found.push(image);
      }
    }
    return found;
  };

  const run = () => {
    const body = document.body;
    if (!body || body.localName !== "body" || !/html/i.test(document.contentType || "text/html")) return notReadable("document");
    const measured = measure(body);
    if (!measured) return notReadable("large");
    const { elements, index, state, good, text: textMass, links: linkMass } = measured;
    const rootIndex = chooseRoot(measured);
    if (good[rootIndex] < MIN_MASS) return notReadable("short");
    const root = elements[rootIndex];
    const metas = metadata();
    const ld = linkedData();
    const site = normalizeSpace(metas.get("og:site_name") || nameOf(ld && ld.publisher) || metas.get("application-name") || "") || location.hostname.replace(/^www\./, "") || null;
    const title = chooseTitle(metas, ld, site);
    const byline = chooseByline(metas, ld, measured);
    const published = choosePublished(metas, ld, root);
    const lang = chooseLanguage(root, metas);
    let dir = "ltr";
    try {
      dir = getComputedStyle(root).direction === "rtl" ? "rtl" : "ltr";
    } catch (error) {
      dir = "ltr";
    }
    const wantedTitle = comparable(title);
    let emittedText = false;

    class Builder {
      constructor(flat, nesting) {
        this.flat = flat;
        this.nesting = nesting;
        this.blocks = [];
        this.runs = [];
        this.keys = [];
      }

      push(run, key) {
        if (this.runs.length >= MAX_INLINES) {
          truncated = true;
          return;
        }
        this.runs.push(run);
        this.keys.push(key);
      }

      text(raw, marks) {
        let text = raw.replace(INVISIBLE_CHARACTERS, "").replace(SEGMENT_BREAK, "$1").replace(COLLAPSIBLE, " ");
        if (!text) return;
        const last = this.runs[this.runs.length - 1];
        if (text.charCodeAt(0) === 32 && (!last || last.type === "break" || last.text.endsWith(" "))) text = text.slice(1);
        if (!text) return;
        if (text.length > textLeft) {
          stopped = true;
          truncated = true;
          return;
        }
        textLeft -= text.length;
        if (last && last.type === "text" && !last.ruby && this.keys[this.keys.length - 1] === marks.key) {
          last.text += text;
          return;
        }
        const run = { type: "text", text };
        if (marks.em) run.em = true;
        if (marks.strong) run.strong = true;
        if (marks.code) run.code = true;
        if (marks.sup) run.sup = true;
        if (marks.sub) run.sub = true;
        if (marks.del) run.del = true;
        if (marks.href) {
          run.href = marks.href;
          textLeft -= marks.href.length;
        }
        this.push(run, marks.key);
      }

      ruby(base, note, marks) {
        if (!base) return;
        this.text(base, marks);
        const last = this.runs[this.runs.length - 1];
        if (last && last.type === "text" && note && last.text.endsWith(base)) {
          if (last.text.length > base.length) {
            last.text = last.text.slice(0, last.text.length - base.length);
            const split = Object.assign({}, last, { text: base, ruby: note.slice(0, 200) });
            this.push(split, null);
          } else {
            last.ruby = note.slice(0, 200);
            this.keys[this.keys.length - 1] = null;
          }
        }
      }

      lineBreak() {
        const last = this.runs[this.runs.length - 1];
        if (!last) return;
        if (last.type === "break") {
          if (!this.flat) {
            this.runs.pop();
            this.keys.pop();
            this.flush();
          }
          return;
        }
        this.push({ type: "break" }, null);
      }

      take() {
        const runs = this.runs;
        this.runs = [];
        this.keys = [];
        while (runs.length && runs[runs.length - 1].type === "break") runs.pop();
        while (runs.length && runs[0].type === "break") runs.shift();
        for (let i = 0; i < runs.length; i++) {
          const run = runs[i];
          if (run.type !== "text") continue;
          if (i === 0 || runs[i - 1].type === "break") run.text = run.text.replace(/^ +/, "");
          if (i === runs.length - 1 || runs[i + 1].type === "break") run.text = run.text.replace(/ +$/, "");
        }
        const kept = runs.filter((run) => run.type === "break" || run.text);
        return kept.some((run) => run.type === "text" && run.text.trim()) ? kept : null;
      }

      flush() {
        if (this.flat) {
          if (this.runs.length && this.runs[this.runs.length - 1].type !== "break") this.push({ type: "break" }, null);
          return;
        }
        const content = this.take();
        if (content) {
          emittedText = true;
          this.add({ type: "paragraph", content });
        }
      }

      block(block) {
        if (this.flat) return;
        this.flush();
        this.add(block);
      }

      add(block) {
        if (blockCount >= MAX_BLOCKS) {
          stopped = true;
          truncated = true;
          return;
        }
        blockCount++;
        this.blocks.push(block);
      }

      lastBlock() {
        return this.runs.length ? null : this.blocks[this.blocks.length - 1];
      }
    }

    const withMark = (marks, name, value) => {
      const next = Object.assign({}, marks);
      next[name] = value;
      next.key = `${next.em ? 1 : 0}${next.strong ? 1 : 0}${next.code ? 1 : 0}${next.sup ? 1 : 0}${next.sub ? 1 : 0}${next.del ? 1 : 0}|${next.href || ""}`;
      return next;
    };

    const blocked = (element) => {
      const i = index.get(element);
      if (i === undefined) return false;
      const flags = state[i];
      if (flags & (SKIPPED | FLIPPED | DROPPED)) return true;
      if (i !== rootIndex && LINK_HEAVY_CANDIDATES.has(element.localName) && textMass[i] > 0 && linkMass[i] > textMass[i] * 0.5) return true;
      return false;
    };

    const children = (element, out, marks, depth) => {
      for (let node = element.firstChild; node && !stopped; node = node.nextSibling) walk(node, out, marks, depth + 1);
    };

    const container = (element, out, marks, depth) => {
      out.flush();
      children(element, out, marks, depth);
      out.flush();
    };

    const inlineOf = (element, depth) => {
      const out = new Builder(true, 0);
      children(element, out, NO_MARKS, depth);
      return out.take() || [];
    };

    const plainOf = (content) => content.map((run) => (run.type === "text" ? run.text : " ")).join("");

    const heading = (element, out, depth) => {
      if (out.flat) {
        container(element, out, NO_MARKS, depth);
        return;
      }
      const content = inlineOf(element, depth);
      if (!content.length) return;
      const plain = comparable(plainOf(content));
      if (!emittedText && wantedTitle && (plain === wantedTitle || (plain.length > wantedTitle.length * 0.6 && wantedTitle.includes(plain)))) return;
      out.block({ type: "heading", level: Number(element.localName[1]), content });
    };

    const list = (element, out, marks, depth) => {
      if (out.flat || out.nesting >= MAX_NESTING) {
        container(element, out, marks, depth);
        return;
      }
      const items = [];
      for (const child of element.children) {
        if (stopped) break;
        if (blocked(child)) continue;
        const item = new Builder(false, out.nesting + 1);
        if (child.localName === "li") children(child, item, marks, depth + 1);
        else walk(child, item, marks, depth + 1);
        item.flush();
        if (!item.blocks.length) continue;
        if ((child.localName === "ul" || child.localName === "ol") && items.length) items[items.length - 1].push(...item.blocks);
        else items.push(item.blocks);
      }
      if (!items.length) return;
      const ordered = element.localName === "ol";
      const listBlock = { type: "list", ordered, items };
      if (ordered) listBlock.start = Number.isFinite(element.start) ? Math.max(0, element.start) : 1;
      out.block(listBlock);
    };

    const definitions = (element, out, marks, depth) => {
      out.flush();
      for (const child of element.children) {
        if (stopped || blocked(child)) continue;
        if (child.localName === "dt") {
          children(child, out, withMark(marks, "strong", true), depth + 1);
          out.flush();
        } else {
          container(child, out, marks, depth + 1);
        }
      }
    };

    const quote = (element, out, marks, depth) => {
      if (out.flat || out.nesting >= MAX_NESTING) {
        container(element, out, marks, depth);
        return;
      }
      const inner = new Builder(false, out.nesting + 1);
      children(element, inner, NO_MARKS, depth);
      inner.flush();
      if (inner.blocks.length) out.block({ type: "quote", blocks: inner.blocks });
    };

    const preText = (element) => {
      const parts = [];
      let size = 0;
      const visit = (node, depth) => {
        if (size > MAX_CODE || depth > 64) return;
        if (node.nodeType === Node.TEXT_NODE) {
          parts.push(node.data);
          size += node.data.length;
          return;
        }
        if (node.nodeType !== Node.ELEMENT_NODE) return;
        const name = node.localName;
        if (name === "br") {
          parts.push("\n");
          return;
        }
        if (INVISIBLE.has(name)) return;
        const tokens = node === element ? null : tokensOf(node);
        if (tokens && [...tokens].some((token) => /^(?:line-?numbers?(?:-rows)?|linenos?|lineno|gutter|ln-num|hljs-ln-numbers|copy)$/.test(token))) return;
        const block = node !== element && BLOCKS.has(name);
        if (block && parts.length && !parts[parts.length - 1].endsWith("\n")) parts.push("\n");
        for (let child = node.firstChild; child; child = child.nextSibling) visit(child, depth + 1);
        if (block && parts.length && !parts[parts.length - 1].endsWith("\n")) parts.push("\n");
      };
      visit(element, 0);
      return parts.join("").replace(/\u00a0/g, " ").replace(/\r\n?/g, "\n").replace(/^\n+/, "").replace(/\s+$/, "").slice(0, MAX_CODE);
    };

    const languageOf = (element) => {
      const holders = [element, element.querySelector("code")].filter(Boolean);
      for (const holder of holders) {
        const attribute = holder.getAttribute("data-lang") || holder.getAttribute("data-language");
        if (attribute && /^[\w+#.-]{1,32}$/.test(attribute)) return attribute.toLowerCase();
        const found = /(?:^|\s)(?:language|lang|highlight-source|brush:?)[-:]?\s*([\w+#.-]{1,32})/i.exec(holder.getAttribute("class") || "");
        if (found) return found[1].toLowerCase();
      }
      for (let up = element.parentElement, hops = 0; up && hops < 2; up = up.parentElement, hops++) {
        const found = /(?:^|\s)(?:language|highlight-source|highlight)-([\w+#.-]{1,32})/i.exec(up.getAttribute("class") || "");
        if (found) return found[1].toLowerCase();
      }
      return null;
    };

    const code = (element, out, marks, depth) => {
      if (out.flat) {
        children(element, out, withMark(marks, "code", true), depth);
        return;
      }
      if (textLeft <= 0) return;
      const text = preText(element);
      if (!text.trim()) return;
      textLeft -= text.length;
      const block = { type: "code", text };
      const language = languageOf(element);
      if (language) block.language = language;
      out.block(block);
    };

    const addImages = (images, out, caption) => {
      const unique = [];
      const seen = new Set();
      for (const image of images) {
        if (seen.has(image.src) || imageCount >= MAX_IMAGES) continue;
        seen.add(image.src);
        imageCount++;
        unique.push(image);
      }
      if (!unique.length) return false;
      out.block({ type: "figure", images: unique, caption: caption || [] });
      return true;
    };

    const figure = (element, out, marks, depth) => {
      if (out.flat) return;
      const images = [];
      for (const img of element.querySelectorAll("img")) {
        if (blocked(img) || isTinyImage(img)) continue;
        const image = imageOf(img, true);
        if (image) images.push(image);
      }
      if (!images.length) images.push(...fallbackImages(element));
      if (!images.length) {
        container(element, out, marks, depth);
        return;
      }
      const captionElement = element.querySelector("figcaption") || Array.from(element.querySelectorAll("*")).find((child) => isLikelyCaption(child));
      const caption = captionElement && !blocked(captionElement) ? inlineOf(captionElement, depth) : [];
      addImages(images, out, caption);
    };

    const isLikelyCaption = (element) => {
      const tokens = tokensOf(element);
      if (!tokens || ![...tokens].some((token) => CAPTION_WORDS.has(token))) return false;
      return !element.querySelector("img, picture, figure, table, pre") && textWeight(normalizeSpace(element.textContent)) <= 600;
    };

    const image = (element, out, marks) => {
      const img = element.localName === "picture" ? element.querySelector("img") : element;
      if (!img || blocked(img)) return;
      if (isTinyImage(img)) {
        const alt = normalizeSpace(img.getAttribute("alt"));
        if (alt && alt.length <= 4) out.text(alt, marks);
        return;
      }
      if (out.flat) return;
      const found = imageOf(img, true);
      const images = found ? [found] : img.parentElement ? fallbackImages(img.parentElement) : [];
      addImages(images.slice(0, 1), out, []);
    };

    const isDataTable = (table) => {
      const role = table.getAttribute("role");
      if (role === "presentation" || role === "none" || table.getAttribute("datatable") === "0") return false;
      if (table.querySelector("table")) return false;
      if (table.caption || table.tHead || table.querySelector("th") || table.getAttribute("summary")) return true;
      const rows = table.rows.length;
      if (rows < 2) return false;
      let columns = 0;
      for (let i = 0; i < Math.min(rows, 10); i++) columns = Math.max(columns, table.rows[i].cells.length);
      if (columns < 2) return false;
      const i = index.get(table);
      return !(i !== undefined && textMass[i] > 4000 && rows * columns < 10);
    };

    const table = (element, out, marks, depth) => {
      if (out.flat || !isDataTable(element)) {
        container(element, out, marks, depth);
        return;
      }
      const rows = [];
      for (const row of element.rows) {
        if (stopped) break;
        if (rows.length >= MAX_ROWS) {
          truncated = true;
          break;
        }
        if (blocked(row)) continue;
        const cells = [];
        for (const cell of row.cells) {
          if (cells.length >= MAX_COLUMNS) break;
          const entry = { header: cell.localName === "th", content: blocked(cell) ? [] : inlineOf(cell, depth) };
          if (cell.colSpan > 1) entry.colspan = Math.min(cell.colSpan, 64);
          if (cell.rowSpan > 1) entry.rowspan = Math.min(cell.rowSpan, 64);
          cells.push(entry);
        }
        if (cells.length) rows.push(cells);
      }
      if (!rows.some((cells) => cells.some((cell) => cell.content.length))) return;
      out.block({ type: "table", caption: element.caption ? inlineOf(element.caption, depth) : [], rows });
    };

    const embed = (element, out) => {
      const source = resolve(element.getAttribute("src") || element.getAttribute("data-src"));
      if (!source || out.flat) return;
      let target = null;
      const youtube = /^https?:\/\/(?:www\.)?(?:youtube(?:-nocookie)?\.com\/embed\/|youtu\.be\/)([\w-]{6,})/i.exec(source);
      const vimeo = /^https?:\/\/player\.vimeo\.com\/video\/(\d+)/i.exec(source);
      if (youtube) target = `https://www.youtube.com/watch?v=${youtube[1]}`;
      else if (vimeo) target = `https://vimeo.com/${vimeo[1]}`;
      if (!target) return;
      const label = normalizeSpace(element.getAttribute("title")) || target;
      out.block({ type: "paragraph", content: [{ type: "text", text: label.slice(0, 300), href: target }] });
    };

    const math = (element, out, marks) => {
      const tex = element.querySelector('annotation[encoding="application/x-tex"]');
      const text = normalizeSpace(element.getAttribute("alttext") || (tex && tex.textContent) || element.textContent);
      if (text) out.text(text, withMark(marks, "code", true));
    };

    const ruby = (element, out, marks) => {
      let base = "";
      let note = "";
      for (let node = element.firstChild; node; node = node.nextSibling) {
        if (node.nodeType === Node.TEXT_NODE) base += node.data;
        else if (node.nodeType === Node.ELEMENT_NODE) {
          if (node.localName === "rt") note += node.textContent;
          else if (node.localName !== "rp") base += node.textContent;
        }
      }
      out.ruby(normalizeSpace(base), normalizeSpace(note), marks);
    };

    const walk = (node, out, marks, depth) => {
      if (stopped) return;
      if (node.nodeType === Node.TEXT_NODE) {
        out.text(node.data, marks);
        return;
      }
      if (node.nodeType !== Node.ELEMENT_NODE || depth > MAX_DOM_DEPTH || tick()) return;
      const element = node;
      const name = element.localName;
      if (blocked(element)) return;
      if (name === "iframe") {
        embed(element, out);
        return;
      }
      switch (name) {
        case "h1":
        case "h2":
        case "h3":
        case "h4":
        case "h5":
        case "h6":
          heading(element, out, depth);
          return;
        case "br":
          out.lineBreak();
          return;
        case "hr":
          if (out.flat) out.flush();
          else out.block({ type: "rule" });
          return;
        case "ul":
        case "ol":
          list(element, out, marks, depth);
          return;
        case "dl":
          definitions(element, out, marks, depth);
          return;
        case "blockquote":
          quote(element, out, marks, depth);
          return;
        case "pre":
        case "xmp":
        case "listing":
          code(element, out, marks, depth);
          return;
        case "figure":
          figure(element, out, marks, depth);
          return;
        case "img":
        case "picture":
          image(element, out, marks);
          return;
        case "table":
          table(element, out, marks, depth);
          return;
        case "math":
          math(element, out, marks);
          return;
        case "ruby":
          ruby(element, out, marks);
          return;
        case "header":
          if (element !== root && textMass[index.get(element)] < 300) return;
          container(element, out, marks, depth);
          return;
        case "summary":
          out.flush();
          children(element, out, withMark(marks, "strong", true), depth);
          out.flush();
          return;
        case "a": {
          const href = resolve(element.getAttribute("href"));
          children(element, out, href ? withMark(marks, "href", href) : marks, depth);
          return;
        }
        case "em":
        case "i":
        case "cite":
        case "dfn":
        case "var":
          children(element, out, withMark(marks, "em", true), depth);
          return;
        case "strong":
        case "b":
          children(element, out, withMark(marks, "strong", true), depth);
          return;
        case "code":
        case "kbd":
        case "samp":
        case "tt": {
          const content = element.textContent || "";
          if (name === "code" && !out.flat && content.split("\n").filter((line) => line.trim()).length >= 3) code(element, out, marks, depth);
          else children(element, out, withMark(marks, "code", true), depth);
          return;
        }
        case "sup":
          children(element, out, withMark(marks, "sup", true), depth);
          return;
        case "sub":
          children(element, out, withMark(marks, "sub", true), depth);
          return;
        case "del":
        case "s":
        case "strike":
          children(element, out, withMark(marks, "del", true), depth);
          return;
        default:
          break;
      }
      const previous = out.lastBlock();
      if (previous && previous.type === "figure" && !previous.caption.length && isLikelyCaption(element)) {
        previous.caption = inlineOf(element, depth);
        return;
      }
      if (BLOCKS.has(name)) container(element, out, marks, depth);
      else children(element, out, marks, depth);
    };

    const out = new Builder(false, 0);
    if (root.localName === "p" || root.localName === "blockquote") walk(root, out, NO_MARKS, 0);
    else children(root, out, NO_MARKS, 0);
    out.flush();

    const blocks = [];
    for (const block of out.blocks) {
      const previous = blocks[blocks.length - 1];
      if (block.type === "rule" && (!previous || previous.type === "rule")) continue;
      blocks.push(block);
    }
    while (blocks.length && blocks[blocks.length - 1].type === "rule") blocks.pop();
    if (!blocks.some((block) => block.type === "paragraph" || block.type === "list" || block.type === "quote" || block.type === "code" || block.type === "table")) return notReadable("empty");

    const contentImages = new Set();
    const collect = (list) => {
      for (const block of list) {
        if (block.type === "figure") block.images.forEach((each) => contentImages.add(fileStem(each.src)));
        else if (block.type === "quote") collect(block.blocks);
        else if (block.type === "list") block.items.forEach(collect);
      }
    };
    const fileStem = (url) => {
      try {
        const parsed = new URL(url);
        return parsed.pathname.replace(/\/+$/, "").split("/").pop().replace(/[-_@](?:\d+x\d+|\d+w|scaled|large|medium|small|thumb)\b/gi, "").replace(/\.[a-z0-9]+$/i, "").toLowerCase();
      } catch (error) {
        return url;
      }
    };
    collect(blocks);
    let lead = null;
    const leadSource = resolve(urlOf(ld && ld.image) || metas.get("og:image:secure_url") || metas.get("og:image") || metas.get("og:image:url") || metas.get("twitter:image") || metas.get("twitter:image:src"));
    const opensWithFigure = blocks.slice(0, 3).some((block) => block.type === "figure");
    if (leadSource && !/^mailto:/i.test(leadSource) && !NOT_LEAD.test(leadSource) && !opensWithFigure && !contentImages.has(fileStem(leadSource))) {
      const width = dimension(metas.get("og:image:width"));
      const height = dimension(metas.get("og:image:height"));
      lead = { src: leadSource, alt: normalizeSpace(metas.get("og:image:alt") || "").slice(0, 300), width: width && height ? width : null, height: width && height ? height : null };
    }

    return { readable: true, title: title || normalizeSpace(document.title) || site || location.hostname || location.href, byline, site, published, lang, dir, image: lead, blocks, truncated };
  };

  try {
    return run();
  } catch (error) {
    return notReadable("error");
  }
})()
