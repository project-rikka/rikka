use rikka_text::Wrap;
use rikka_ui::text::TextRun;

#[derive(Clone, Debug, Default)]
pub struct FittedLine {
    pub text: String,
    pub width: f32,
}

#[must_use]
pub fn fitted_line(run: &TextRun, width: f32) -> FittedLine {
    if width <= 0.0 || run.content.is_empty() {
        return FittedLine::default();
    }
    let run = run.clone().no_wrap().ellipsized();
    let laid = rikka_text::layout(run.as_shape(), Some(width));
    FittedLine {
        text: laid.visible_text().as_str().to_owned(),
        width: laid.widest(),
    }
}

#[must_use]
pub fn fitted_lines(run: &TextRun, width: f32, lines: usize) -> Vec<FittedLine> {
    if width <= 0.0 || lines == 0 || run.content.is_empty() {
        return Vec::new();
    }
    let mut wrapping = run.clone();
    wrapping.wrap = Wrap::Glyph;
    wrapping.ellipsize = false;
    let laid = rikka_text::layout(wrapping.as_shape(), Some(width));
    let text = run.content.as_str();
    let mut fitted = Vec::with_capacity(lines);
    for row in laid.rows() {
        let range = row.text_range();
        if fitted.len() + 1 == lines && range.end < text.len() {
            let rest = TextRun {
                spans: Default::default(),
                ..run.clone().content(&text[range.start..])
            };
            fitted.push(fitted_line(&rest, width));
            break;
        }
        fitted.push(FittedLine {
            text: text[range].to_owned(),
            width: row.width(),
        });
        if fitted.len() == lines {
            break;
        }
    }
    fitted
}
