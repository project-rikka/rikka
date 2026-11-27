(globalThis[Symbol.for("rikka")] ??= {})["browser-reader.probe"] = () => {
  "use strict";
  const MAX_NODES = 3000;
  const MIN_UNIT = 80;
  const MAX_LINK_SHARE = 0.35;
  const IDEOGRAPHIC = /[\u3040-\u30ff\u31f0-\u31ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uff66-\uff9d]/g;
  const BOILERPLATE = "nav, aside, footer, header, form, menu, dialog, [role=navigation], [role=complementary], [role=contentinfo], [role=banner], [role=dialog], [aria-hidden=true], [hidden]";
  const NOISY = /(?:^|[\s_-])(?:comments?|sidebar|related|recommend(?:ed|ations?)?|ranking|share|social|sns|footer|navigation|nav|menu|breadcrumbs?|ad|ads|advert|promo|sponsored|newsletter|subscribe|cookie|popup|modal)(?:$|[\s_-])/i;

  const textWeight = (text) => {
    const length = text.length;
    if (length === 0) return 0;
    const rest = text.replace(IDEOGRAPHIC, "").length;
    return rest + (length - rest) * 3;
  };

  const visible = (element) => {
    if (typeof element.checkVisibility === "function") return element.checkVisibility();
    return element.getClientRects().length > 0;
  };

  const isBoilerplate = (element, body) => {
    for (let node = element, hops = 0; node && node !== body && hops < 6; node = node.parentElement, hops++) {
      const label = `${node.id || ""} ${node.getAttribute("class") || ""}`;
      if (label.length > 1 && NOISY.test(label)) return true;
    }
    return false;
  };

  const hasArticleSignal = () => {
    const type = document.querySelector('meta[property="og:type"]');
    if (type && /article/i.test(type.getAttribute("content") || "")) return true;
    if (document.querySelector('[itemprop="articleBody"], [property="articleBody"], article')) return true;
    for (const script of document.querySelectorAll('script[type="application/ld+json"]')) {
      if (/"@type"\s*:\s*\[?\s*"(?:News|Blog|Tech|Scholarly|Report|Opinion|Analysis)?Article|"BlogPosting"/.test(script.textContent || "")) return true;
    }
    return false;
  };

  const LOOSE_BREAKERS = new Set(["div", "p", "pre", "ul", "ol", "dl", "table", "section", "article", "blockquote", "figure", "h1", "h2", "h3", "h4", "h5", "h6", "header", "footer", "nav", "aside", "form"]);

  const chunksOf = (unit) => {
    if (unit.localName === "p" || unit.localName === "pre" || unit.localName === "td") {
      if (unit.localName === "td" && unit.querySelector("p, pre, td")) return [];
      let linked = 0;
      for (const link of unit.getElementsByTagName("a")) linked += textWeight((link.textContent || "").trim());
      return [{ weight: textWeight((unit.textContent || "").trim()), linked }];
    }
    const chunks = [];
    let weight = 0;
    let linked = 0;
    const close = () => {
      chunks.push({ weight, linked });
      weight = 0;
      linked = 0;
    };
    for (let node = unit.firstChild; node; node = node.nextSibling) {
      if (node.nodeType === Node.TEXT_NODE) {
        weight += textWeight(node.data.trim());
      } else if (node.nodeType === Node.ELEMENT_NODE) {
        const name = node.localName;
        if (name === "br" || LOOSE_BREAKERS.has(name)) {
          close();
          continue;
        }
        const text = textWeight((node.textContent || "").trim());
        weight += text;
        if (name === "a") linked += text;
        else for (const link of node.getElementsByTagName("a")) linked += textWeight((link.textContent || "").trim());
      }
    }
    close();
    return chunks;
  };

  const probe = () => {
    const body = document.body;
    if (!body || !/html/i.test(document.contentType || "text/html")) return { readable: false };
    const units = new Set();
    for (const node of body.querySelectorAll("p, pre, td, div > br, section > br, article > br")) {
      units.add(node.localName === "br" ? node.parentElement : node);
      if (units.size >= MAX_NODES) break;
    }
    let mass = 0;
    let count = 0;
    for (const unit of units) {
      if (!unit || unit.closest(BOILERPLATE) || !visible(unit) || isBoilerplate(unit, body)) continue;
      for (const chunk of chunksOf(unit)) {
        if (chunk.weight < MIN_UNIT || chunk.linked > chunk.weight * MAX_LINK_SHARE) continue;
        mass += Math.min(chunk.weight, 2400);
        count += 1;
      }
    }
    const signal = hasArticleSignal();
    const readable = (count >= 3 && mass >= 800) || (count >= 1 && mass >= 1800) || (signal && mass >= 500);
    return { readable, mass: Math.round(mass), units: count, signal };
  };

  try {
    return probe();
  } catch (error) {
    return { readable: false };
  }
};
