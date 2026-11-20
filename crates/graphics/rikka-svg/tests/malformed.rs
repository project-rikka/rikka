use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write;

use rikka_svg::{Error, Svg};

const SIDES: [u32; 2] = [16, 256];
const ALLOCATION_LIMIT: usize = 48 << 20;
const BOMB_EXPANSION: usize = 1 << 30;
const OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">"#;
const CLOSE: &str = "</svg>";

struct Peak;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static HIGHEST: Cell<usize> = const { Cell::new(0) };
}

fn grew(bytes: usize) {
    let _ = LIVE.try_with(|live| {
        let now = live.get() + bytes;
        live.set(now);
        let _ = HIGHEST.try_with(|highest| highest.set(highest.get().max(now)));
    });
}

fn shrank(bytes: usize) {
    let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(bytes)));
}

unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrank(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        shrank(layout.size());
        grew(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Peak = Peak;

fn peak_allocation(work: impl FnOnce()) -> usize {
    let start = LIVE.with(Cell::get);
    HIGHEST.with(|highest| highest.set(start));
    work();
    HIGHEST.with(Cell::get) - start
}

fn document(body: &str) -> String {
    format!("{OPEN}{body}{CLOSE}")
}

fn nested(depth: usize, open: &str, close: &str, inner: &str) -> String {
    format!("{}{inner}{}", open.repeat(depth), close.repeat(depth))
}

fn hostile_documents() -> Vec<(&'static str, String)> {
    let square = r##"<rect width="16" height="16" fill="#3584e4"/>"##;
    let mut fan_out = String::from(r#"<defs><g id="u0"><rect width="1" height="1"/></g>"#);
    for level in 1..10 {
        let uses = format!(r##"<use href="#u{}"/>"##, level - 1).repeat(10);
        fan_out.push_str(&format!(r#"<g id="u{level}">{uses}</g>"#));
    }
    fan_out.push_str(r##"</defs><use href="#u9"/>"##);
    let mut deep_uses = String::from("<defs>");
    for level in 0..40 {
        let inner = format!(r##"<use href="#d{}"/>"##, level + 1);
        let chain = nested(400, "<g>", "</g>", &inner);
        deep_uses.push_str(&format!(r#"<g id="d{level}">{chain}</g>"#));
    }
    deep_uses.push_str(&format!(
        r##"<g id="d40">{square}</g></defs><use href="#d0"/>"##
    ));
    let mut chained_masks = String::from("<defs>");
    for level in 0..2000 {
        chained_masks.push_str(&format!(
            r##"<mask id="m{level}" mask="url(#m{})"><rect width="16" height="16" fill="#fff"/></mask>"##,
            level + 1
        ));
    }
    let mut reused_masks = chained_masks.clone();
    chained_masks.push_str(&format!(r##"</defs><g mask="url(#m0)">{square}</g>"##));
    reused_masks.push_str("</defs>");
    for level in (0..2000).rev().step_by(4) {
        reused_masks.push_str(&format!(r##"<g mask="url(#m{level})">{square}</g>"##));
    }
    let deep_mask = nested(
        480,
        r#"<g transform="translate(0.01)">"#,
        "</g>",
        r##"<rect width="16" height="16" fill="#fff"/>"##,
    );
    let deep_reuse = format!(
        r##"<mask id="deep">{deep_mask}</mask><g mask="url(#deep)">{square}</g>{}"##,
        nested(
            480,
            r#"<g transform="translate(0.01)">"#,
            "</g>",
            &format!(r##"<g mask="url(#deep)">{square}</g>"##)
        )
    );
    let mut chained_patterns = String::from("<defs>");
    for level in 0..200 {
        chained_patterns.push_str(&format!(
            r##"<pattern id="p{level}" width="1e6" height="1e6" patternUnits="userSpaceOnUse"><rect width="1e6" height="1e6" fill="url(#p{})"/></pattern>"##,
            level + 1
        ));
    }
    chained_patterns.push_str(r##"</defs><rect width="16" height="16" fill="url(#p0)"/>"##);
    let mut filter_results = String::from(r#"<filter id="f">"#);
    for level in 0..5000 {
        filter_results.push_str(&format!(r#"<feOffset dx="1" result="r{level}"/>"#));
    }
    filter_results.push_str(&format!(r#"</filter><g filter="url(#f)">{square}</g>"#));
    let segments = "l1 1".repeat(1_200_000);
    let vertices = "l0.001 0.001".repeat(20_000);
    let heavy_marker = format!(
        r##"<marker id="heavy" markerUnits="userSpaceOnUse" overflow="visible">{}</marker>"##,
        r#"<path d="M0 0h1v1z"/>"#.repeat(200)
    );
    let mut chained_markers = String::new();
    for level in 0..300 {
        chained_markers.push_str(&format!(
            r##"<marker id="k{level}" overflow="visible"><path d="M0 0L1 1L2 0" stroke="#000" marker-mid="url(#k{})"/></marker>"##,
            level + 1
        ));
    }
    let mut chained_images = String::from("<defs>");
    for level in 0..300 {
        chained_images.push_str(&format!(
            r##"<filter id="i{level}"><feImage href="#e{}"/></filter><g id="e{level}" filter="url(#i{level})"><rect width="4" height="4"/></g>"##,
            level + 1
        ));
    }
    chained_images.push_str(r##"</defs><use href="#e0"/>"##);
    let siblings = r#"<rect width="1" height="1"/>"#.repeat(20_000);
    let mut deep_media = String::new();
    for _ in 0..200 {
        deep_media.push_str("@media (min-width: 1px) { ");
    }
    deep_media.push_str("rect { fill: red }");
    deep_media.push_str(&" }".repeat(200));
    vec![
        (
            "text floods",
            document(&format!(
                r#"<text font-family="JetBrains Mono" x="0" y="8">{}</text>"#,
                "x".repeat(300_000)
            )),
        ),
        (
            "nested text spans",
            document(&format!(
                r#"<text y="8">{}</text>"#,
                nested(600, r#"<tspan dy="0.1" baseline-shift="super">"#, "</tspan>", "deep")
            )),
        ),
        (
            "extreme text values",
            document(
                r##"<text x="3e38 -3e38 NaN" y="1e30" dx="3e38" rotate="3e38 NaN" font-size="3e38" letter-spacing="3e38" word-spacing="-3e38">a b c</text>
                <text font-size="1e-38" textLength="3e38" lengthAdjust="spacingAndGlyphs">tiny</text>
                <text font-size="-5" font-weight="1e38" font-stretch="1e30%" baseline-shift="3e38" dominant-baseline="central">neg</text>
                <path id="p" d="M0 0C3e38 3e38 -3e38 3e38 16 16"/><text><textPath href="#p" startOffset="1e38%">along</textPath><textPath href="#missing">gone</textPath><textPath href="#t">self</textPath></text>
                <text id="t"><textPath href="#t">loop</textPath></text>"##,
            ),
        ),
        (
            "colour glyph floods",
            document(&format!(
                r#"<text font-family="Noto Color Emoji" y="8" text-decoration="underline line-through">{}</text>
                <text font-size="3e38" text-decoration="overline">🦊🌻</text>
                <text font-size="1e6" x="-5e5" y="5e5" rotate="45">🦊</text>
                <text font-size="0.0001">🎉</text>"#,
                "🦊🌻🎉✨".repeat(1_500)
            )),
        ),
        (
            "decorations and fitted lengths",
            document(&format!(
                r#"<text y="8" font-variant="small-caps all-small-caps titling-caps" textLength="NaN">{}</text>
                <text textLength="3e38">wide</text><text textLength="-4" lengthAdjust="spacingAndGlyphs">neg</text>
                <text textLength="0" lengthAdjust="spacingAndGlyphs" text-decoration="underline">zero</text>"#,
                nested(
                    500,
                    r#"<tspan text-decoration="underline overline line-through" textLength="1e30" lengthAdjust="spacingAndGlyphs" dx="1">"#,
                    "</tspan>",
                    "deep"
                )
            )),
        ),
        ("markers on many vertices", document(&format!(
            r##"<defs>{heavy_marker}</defs><path d="M0 0{vertices}" stroke="#000" marker-mid="url(#heavy)"/>"##
        ))),
        (
            "recursive markers",
            document(&format!(
                r##"<defs><marker id="self" overflow="visible"><path d="M0 0L4 4L8 0" stroke="#000" marker-mid="url(#self)"/></marker>
                <marker id="a" overflow="visible"><path d="M0 0L4 4L8 0" stroke="#000" marker-start="url(#b)"/></marker>
                <marker id="b" overflow="visible"><path d="M0 0L4 4L8 0" stroke="#000" marker-start="url(#a)"/></marker>
                <marker id="huge" viewBox="0 0 1e-38 1e-38" markerWidth="3e38" markerHeight="3e38" refX="3e38" orient="3e38deg"><rect width="1" height="1"/></marker>
                {chained_markers}</defs>
                <path d="M0 0L8 8L16 0" stroke="#000" marker-mid="url(#self)" marker-start="url(#a)" marker-end="url(#huge)"/>
                <path d="M0 8L8 16L16 8" stroke="#000" marker-mid="url(#k0)"/>"##
            )),
        ),
        (
            "extreme new filter primitives",
            document(&format!(
                r#"<filter id="f" x="-1" y="-1" width="3" height="3">
                    <feTurbulence baseFrequency="1e30 -1" numOctaves="1e9" seed="-3e38" stitchTiles="stitch"/>
                    <feConvolveMatrix order="1000" kernelMatrix="1"/><feConvolveMatrix order="32" kernelMatrix="{kernel}" targetX="1e9"/>
                    <feConvolveMatrix order="3" kernelMatrix="1 1 1 1 1 1 1 1 1" divisor="1e-38" bias="3e38" edgeMode="wrap"/>
                    <feDisplacementMap in2="SourceGraphic" scale="3e38" xChannelSelector="A"/>
                    <feDiffuseLighting surfaceScale="3e38" diffuseConstant="-3e38"><fePointLight x="3e38" y="NaN" z="-3e38"/></feDiffuseLighting>
                    <feSpecularLighting specularExponent="128" surfaceScale="1e30"><feSpotLight specularExponent="3e38" limitingConeAngle="NaN" pointsAtZ="3e38"/></feSpecularLighting>
                    <feSpecularLighting specularExponent="1000"><feDistantLight/></feSpecularLighting>
                    <feImage href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='3e38' height='3e38'/%3E" width="3e38" height="3e38"/>
                </filter>
                <g filter="url(#f)">{square}</g>"#,
                kernel = "1 ".repeat(1024)
            )),
        ),
        ("chained element images", document(&chained_images)),
        (
            "selector explosion",
            document(&format!(
                r#"<style>* ~ * ~ * ~ * ~ rect:not(:first-child) {{ fill: red }} [a] [b] [c] rect:nth-child(2n+1) {{ stroke: blue }}</style><g>{siblings}</g>"#
            )),
        ),
        (
            "deep media queries",
            document(&format!(r#"<style>{deep_media}</style>{square}"#)),
        ),
        (
            "nested translucent groups",
            document(&nested(500, r#"<g opacity="0.9">"#, "</g>", square)),
        ),
        (
            "nested filtered groups",
            document(&format!(
                r#"<filter id="f"><feGaussianBlur stdDeviation="2"/></filter>{}"#,
                nested(300, r#"<g filter="url(#f)">"#, "</g>", square)
            )),
        ),
        (
            "nesting beyond the parser depth",
            document(&nested(5000, "<g>", "</g>", square)),
        ),
        (
            "use cycles",
            document(
                r##"<use id="a" href="#a"/><g id="g"><use href="#g"/></g>
                <use id="b" href="#c"/><use id="c" href="#b"/>
                <symbol id="s"><use href="#s"/></symbol><use href="#s"/>"##,
            ),
        ),
        ("use fan-out", document(&fan_out)),
        ("use chain through deep groups", document(&deep_uses)),
        (
            "self-referencing paint servers",
            document(
                r##"<linearGradient id="a" href="#b"/><linearGradient id="b" href="#a"/>
                <radialGradient id="r" href="#r"><stop offset="0" stop-color="#f00"/></radialGradient>
                <pattern id="p" href="#p" width="4" height="4" patternUnits="userSpaceOnUse">
                    <rect width="4" height="4" fill="url(#p)"/><rect width="2" height="2" fill="url(#q)"/>
                </pattern>
                <pattern id="q" width="2" height="2" patternUnits="userSpaceOnUse">
                    <rect width="2" height="2" fill="url(#p)"/>
                </pattern>
                <rect width="8" height="8" fill="url(#a)"/><rect x="8" width="8" height="8" fill="url(#r)"/>
                <rect y="8" width="16" height="8" fill="url(#p)"/>"##,
            ),
        ),
        (
            "self-referencing clips, masks and filters",
            document(
                r##"<clipPath id="c" clip-path="url(#c)"><rect width="8" height="8" clip-path="url(#c)"/><use href="#user"/></clipPath>
                <mask id="m" mask="url(#m)"><rect width="16" height="16" fill="#fff" mask="url(#m)"/><use href="#user"/></mask>
                <filter id="f"><feImage href="#user"/><feTile/></filter>
                <g id="user" clip-path="url(#c)" mask="url(#m)" filter="url(#f)"><rect width="16" height="16"/></g>"##,
            ),
        ),
        ("chained masks", document(&chained_masks)),
        ("chained masks reused from below", document(&reused_masks)),
        ("deep mask content reused deep", document(&deep_reuse)),
        ("chained giant patterns", document(&chained_patterns)),
        ("many named filter results", document(&filter_results)),
        (
            "giant document size",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="3e38" height="3e38"><rect width="3e38" height="3e38"/></svg>"##.to_owned(),
        ),
        (
            "giant finite document size",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1e30" height="1e30"><rect width="1e30" height="1e30"/><circle r="1"/></svg>"##.to_owned(),
        ),
        (
            "giant view box",
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-3e38 -3e38 3e38 3e38"><circle r="3e38"/></svg>"##.to_owned(),
        ),
        (
            "tiny view box",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1e-38" height="1e-38" viewBox="0 0 1e-38 1e-38"><rect width="1" height="1"/></svg>"##.to_owned(),
        ),
        (
            "giant filter regions",
            document(&format!(
                r#"<filter id="f" filterUnits="userSpaceOnUse" x="-3e38" y="-3e38" width="3e38" height="3e38">
                    <feGaussianBlur stdDeviation="3e38"/><feOffset dx="3e38" dy="-3e38"/>
                    <feMorphology radius="3e38"/><feMorphology operator="dilate" radius="3e38 1"/>
                    <feDropShadow dx="-3e38" dy="3e38" stdDeviation="3e38"/><feTile/>
                </filter>
                <filter id="b" x="-1e30" y="-1e30" width="3e30" height="3e30" primitiveUnits="objectBoundingBox">
                    <feGaussianBlur stdDeviation="1e30"/><feOffset dx="1e30"/>
                    <feFlood x="-1e30" width="3e30" flood-color="red"/>
                </filter>
                <g filter="url(#f)">{square}</g><g filter="url(#b)" transform="scale(3e38)">{square}</g>"#
            )),
        ),
        (
            "giant pattern tiles and masks",
            document(
                r##"<pattern id="p" width="1e7" height="1e7" patternUnits="userSpaceOnUse"><rect width="1e7" height="1e7" fill="#0f0"/></pattern>
                <pattern id="t" width="1" height="1e9" patternUnits="userSpaceOnUse" patternTransform="scale(1e30)"><rect width="1" height="1"/></pattern>
                <mask id="m" maskUnits="userSpaceOnUse" x="-3e38" y="-3e38" width="3e38" height="3e38"><rect width="3e38" height="3e38" fill="#fff"/></mask>
                <rect width="16" height="16" fill="url(#p)" mask="url(#m)"/><rect width="16" height="16" fill="url(#t)"/>"##,
            ),
        ),
        (
            "huge coordinates",
            document(
                r##"<path d="M0 0L3e38 3e38L-3e38 0Z"/><path d="M1e308 0L0 1e308Z"/>
                <circle r="3e38"/><ellipse rx="1e-38" ry="3e38"/><rect width="1e999" height="-1e999"/>
                <rect width="3e38" height="3e38" rx="3e38"/><text x="3e38">overflow</text>"##,
            ),
        ),
        (
            "unparsable numbers",
            document(
                r##"<path d="M NaN 0 L inf 1 L -infinity 2"/>
                <polyline points="0,0 3e38,3e38 -3e38,1e-38 NaN" stroke="#000"/>"##,
            ),
        ),
        (
            "degenerate arcs and curves",
            document(
                r##"<path d="M0 0 A3e38 3e38 0 1 1 1 1 Z"/><path d="M0 0 A1e-38 1e-38 0 1 1 3e38 0Z"/>
                <path d="M0 0 C3e38 -3e38 -3e38 3e38 16 16" stroke="#000"/>"##,
            ),
        ),
        (
            "huge strokes",
            document(
                r##"<circle r="3e38" stroke="#000" stroke-width="3e38"/>
                <polyline points="0,0 16,0 0,1e-30" stroke="#000" stroke-miterlimit="3e38"/>
                <g transform="scale(1e-38)"><rect width="3e38" height="3e38" stroke="#000" stroke-width="1e38"/></g>"##,
            ),
        ),
        (
            "subnormal dashes",
            document(
                r##"<rect width="3e38" height="3e38" stroke="#000" stroke-dasharray="1e-38 1e-38"/>"##,
            ),
        ),
        (
            "huge transforms",
            document(
                r##"<g transform="scale(3e38)"><rect width="1" height="1"/></g>
                <g transform="matrix(3e38 0 0 3e38 3e38 3e38)"><rect width="1" height="1" stroke="#000"/></g>
                <g transform="rotate(3e38) skewX(90) skewY(-90) translate(3e38)"><rect width="1" height="1"/></g>"##,
            ),
        ),
        (
            "huge relative lengths",
            document(
                r##"<g opacity="3e38" style="stroke-width: 3e38px; font-size: 3e38em">
                <rect width="2em" height="1e38ex" stroke="#000"/></g>"##,
            ),
        ),
        (
            "dashes finer than a pixel",
            document(
                r##"<path d="M0 0H1e6" stroke="#000" stroke-dasharray="0.000001"/>
                <g transform="scale(1e-6)"><circle cx="8e6" cy="8e6" r="8e6" stroke="#000" stroke-width="1e6" stroke-dasharray="1"/></g>"##,
            ),
        ),
        ("huge path data", document(&format!(r#"<path d="M0 0{segments}"/>"#))),
    ]
}

#[test]
fn hostile_documents_stay_within_bounds() {
    let mut failures = Vec::new();
    for (name, markup) in hostile_documents() {
        for side in SIDES {
            let mut outcome = String::new();
            let peak = peak_allocation(|| {
                outcome = match Svg::from_bytes(markup.as_bytes())
                    .and_then(|svg| svg.render(side, side))
                {
                    Ok(raster) => {
                        assert_eq!(raster.pixels().len(), (side * side * 4) as usize, "{name}");
                        "rendered".to_owned()
                    }
                    Err(error) => error.to_string(),
                };
            });
            if peak > ALLOCATION_LIMIT {
                failures.push(format!(
                    "{name} at {side}: {outcome}, peak {} MiB",
                    peak >> 20
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn compressed_documents_inflate_within_bounds() {
    let padding = vec![b' '; 1 << 20];
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(OPEN.as_bytes()).expect("gzip header");
    for _ in 0..BOMB_EXPANSION / padding.len() {
        encoder.write_all(&padding).expect("gzip body");
    }
    encoder.write_all(CLOSE.as_bytes()).expect("gzip trailer");
    let bomb = encoder.finish().expect("gzip stream");

    let mut outcome = None;
    let peak = peak_allocation(|| outcome = Some(Svg::from_bytes(&bomb)));
    assert!(matches!(outcome, Some(Err(Error::TooComplex))));
    assert!(
        peak < BOMB_EXPANSION / 4,
        "inflating a {} KiB bomb peaked at {} MiB",
        bomb.len() >> 10,
        peak >> 20
    );
}
