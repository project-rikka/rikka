use crate::look::{Look, Spacing, Typeface, Width};

const SHEET: &str = r#"*,*::before,*::after{box-sizing:border-box}
html{background:var(--background);color:var(--text);font-family:var(--body);font-size:var(--size);line-height:var(--leading);-webkit-text-size-adjust:100%;text-size-adjust:100%;font-kerning:normal;text-rendering:optimizeLegibility}
body{margin:0;padding:48px 24px 96px}
article{max-width:var(--measure);margin:0 auto;overflow-wrap:break-word}
header{margin:0 0 2em;padding-bottom:1.6em;border-bottom:1px solid var(--rule)}
.site{margin:0 0 .8em;color:var(--secondary);font-size:.8em;font-weight:600;letter-spacing:.06em;text-transform:uppercase}
h1{margin:0;font-size:2em;line-height:1.2;font-weight:700;letter-spacing:-.01em;text-wrap:balance}
.byline{margin:1em 0 0;font-size:.9em;font-weight:600}
.meta{margin:.3em 0 0;color:var(--secondary);font-size:.85em}
.meta>span+span::before{content:"\00b7";margin:0 .5em}
h2,h3,h4,h5,h6{margin:1.8em 0 .6em;line-height:1.3;font-weight:700;text-wrap:balance}
h2{font-size:1.45em}
h3{font-size:1.2em}
h4{font-size:1.05em}
h5,h6{font-size:1em}
p{margin:0 0 1.15em;text-wrap:pretty}
a{color:var(--link);text-decoration-line:underline;text-decoration-thickness:1px;text-underline-offset:.18em}
strong{font-weight:700}
img{display:block;max-width:100%;height:auto;margin:0 auto;border-radius:6px}
figure{margin:2em 0}
figure img+img{margin-top:.75em}
figcaption{margin-top:.6em;color:var(--secondary);font-size:.85em;line-height:1.5}
.lead{margin:0 0 2em}
blockquote{margin:1.6em 0;padding:.1em 0 .1em 1.2em;border-inline-start:3px solid var(--rule)}
blockquote>:last-child,li>:last-child{margin-bottom:0}
ul,ol{margin:0 0 1.15em;padding-inline-start:1.6em}
li{margin:.35em 0}
li>ul,li>ol{margin:.35em 0 0}
pre{margin:1.6em 0;padding:1em 1.2em;overflow-x:auto;background:var(--surface);border-radius:8px;font-size:.85em;line-height:1.55;tab-size:4;white-space:pre;word-break:normal;overflow-wrap:normal;text-align:start}
code{font-family:var(--mono);font-size:.9em}
:not(pre)>code{padding:.1em .35em;background:var(--surface);border-radius:4px}
pre code{font-size:inherit}
.table{margin:1.6em 0;overflow-x:auto}
table{border-collapse:collapse;font-size:.9em;line-height:1.5}
caption{caption-side:top;text-align:start;color:var(--secondary);padding-bottom:.5em}
th,td{padding:.45em .7em;border:1px solid var(--rule);text-align:start;vertical-align:top}
th{background:var(--surface);font-weight:600}
hr{margin:2.5em auto;width:30%;border:0;border-top:1px solid var(--rule)}
sup,sub{font-size:.75em;line-height:0}
rt{font-size:.5em}
del{color:var(--secondary)}
html:not(.ideographic) p,html:not(.ideographic) li{hyphens:auto;-webkit-hyphens:auto}
html:lang(ko) article{word-break:keep-all}
.ideographic article{line-break:strict;word-break:normal;overflow-wrap:anywhere;hanging-punctuation:allow-end}
.ideographic p,.ideographic li{text-align:justify;text-justify:inter-character}
.ideographic h1{font-size:1.7em;line-height:1.4;letter-spacing:.02em;font-feature-settings:"palt"}
.ideographic h2,.ideographic h3,.ideographic h4{line-height:1.5;letter-spacing:.02em;font-feature-settings:"palt"}
.ideographic .site{text-transform:none;letter-spacing:.02em}
.ideographic em{font-style:normal;-webkit-text-emphasis:filled sesame;text-emphasis:filled sesame}
@media (max-width:600px){body{padding:24px 16px 64px}h1{font-size:1.6em}}
"#;

pub(crate) fn sheet(look: &Look, ideographic: bool) -> String {
    let colors = look.palette.colors();
    let body = match look.typeface {
        Typeface::Serif => "serif",
        Typeface::Sans => "sans-serif",
    };
    format!(
        ":root{{--background:{};--text:{};--secondary:{};--link:{};--rule:{};--surface:{};--body:{body};--mono:monospace;--size:{}px;--measure:{}em;--leading:{}}}\n{SHEET}",
        colors.background,
        colors.text,
        colors.secondary,
        colors.link,
        colors.rule,
        colors.surface,
        look.size.pixels(),
        measure(look.width, ideographic),
        leading(look.spacing, ideographic),
    )
}

fn measure(width: Width, ideographic: bool) -> u8 {
    match (width, ideographic) {
        (Width::Narrow, false) => 30,
        (Width::Medium, false) => 35,
        (Width::Wide, false) => 42,
        (Width::Narrow, true) => 28,
        (Width::Medium, true) => 34,
        (Width::Wide, true) => 40,
    }
}

fn leading(spacing: Spacing, ideographic: bool) -> &'static str {
    match (spacing, ideographic) {
        (Spacing::Compact, false) => "1.45",
        (Spacing::Normal, false) => "1.6",
        (Spacing::Relaxed, false) => "1.8",
        (Spacing::Compact, true) => "1.7",
        (Spacing::Normal, true) => "1.85",
        (Spacing::Relaxed, true) => "2.05",
    }
}
