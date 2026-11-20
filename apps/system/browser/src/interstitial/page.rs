use rikka_html::Escaped;

use crate::interstitial::Problem;

pub(crate) fn document(problem: &Problem, dark: bool) -> String {
    let title = problem.title();
    let title = Escaped(title.as_ref());
    let (background, ink) = match dark {
        true => ("#1b1b1f", "#e4e1e6"),
        false => ("#fdfbff", "#1b1b1f"),
    };
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\
         <title>{title}</title><style>html{{background:{background};color:{ink};\
         font-family:sans-serif}}</style></head><body><h1 hidden>{title}</h1></body></html>"
    )
}
