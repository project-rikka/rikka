use std::ops::Range;
use std::sync::Arc;

use discord_core::{Block, ChannelId, Inline, Mention, Picture, Rich, SpaceId, UserId};
use rikka_app::prelude::*;
use rikka_syntax::Language;
use rikka_ui::{Element, FontStyle};
use rikka_widgets::{column, row};

use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::media::{frames, open_link};
use crate::conversation::widgets::animated;
use crate::conversation::{AREA, time};
use crate::pictures;

const BODY: f32 = 15.0;
const LEADING: f32 = 1.4;
const BIG: f32 = 48.0;
const CODE: f32 = 13.0;
const MAX_SPOILERS: u32 = 63;

#[derive(Clone, Debug)]
pub(crate) enum Target {
    Url(String),
    User(UserId),
    Channel(ChannelId),
    Nothing,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TextRun {
    text: String,
    leaves: Vec<(TextSpan, Option<u32>)>,
    fills: Vec<(Range<usize>, Option<Color>, Option<u32>)>,
    links: Vec<(Range<usize>, Target, Option<u32>)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Piece {
    Text(TextRun),
    Emoji {
        picture: Picture,
        side: f32,
        animate: bool,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum Shaped {
    Prose {
        pieces: Vec<Piece>,
        size: f32,
        weight: Weight,
        muted: bool,
    },
    Quote(Vec<Shaped>),
    Code {
        language: Option<String>,
        text: Arc<str>,
        spans: Arc<[TextSpan]>,
    },
    List {
        markers: Vec<String>,
        items: Vec<Vec<Shaped>>,
    },
}

#[derive(Clone, Copy, Default)]
struct Marks {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    link: Option<usize>,
    spoiler: Option<u32>,
}

#[derive(Clone, Copy, Default)]
struct Extra {
    color: Option<Color>,
    fill: Option<Color>,
    mono: bool,
    weight: Option<Weight>,
    size: Option<f32>,
}

pub(crate) struct Shaping<'a> {
    linked: &'a LinkedAccount,
    space: Option<&'a SpaceId>,
    big: bool,
    spoilers: u32,
    targets: Vec<Target>,
    relative: bool,
    animate: bool,
}

impl<'a> Shaping<'a> {
    pub(crate) fn new(
        linked: &'a LinkedAccount,
        space: Option<&'a SpaceId>,
        animate: bool,
    ) -> Shaping<'a> {
        Shaping {
            linked,
            space,
            big: false,
            spoilers: 0,
            targets: Vec::new(),
            relative: false,
            animate,
        }
    }

    pub(crate) fn is_relative(&self) -> bool {
        self.relative
    }

    pub(crate) fn rich(&mut self, rich: &Rich) -> Vec<Shaped> {
        self.big = rich.only_emoji();
        let shaped = rich.0.iter().map(|block| self.block(block)).collect();
        self.big = false;
        shaped
    }

    fn block(&mut self, block: &Block) -> Shaped {
        match block {
            Block::Paragraph(inlines) => self.prose(inlines, BODY, font::REGULAR, false),
            Block::Heading { level, content } => {
                let size = match level {
                    1 => 24.0,
                    2 => 20.0,
                    _ => 17.0,
                };
                self.prose(content, size, font::BOLD, false)
            }
            Block::Subtext(inlines) => self.prose(inlines, 12.0, font::REGULAR, true),
            Block::Quote(blocks) => {
                Shaped::Quote(blocks.iter().map(|inner| self.block(inner)).collect())
            }
            Block::Code { language, text } => {
                let spans = language
                    .as_deref()
                    .and_then(Language::named)
                    .map(|grammar| grammar.spans(text, theme().syntax()))
                    .unwrap_or_default();
                Shaped::Code {
                    language: language.clone(),
                    text: Arc::from(text.trim_end_matches('\n')),
                    spans: spans.into(),
                }
            }
            Block::List { start, items } => Shaped::List {
                markers: (0..items.len())
                    .map(|at| match start {
                        Some(first) => format!("{}.", u64::from(*first) + at as u64),
                        None => "•".to_owned(),
                    })
                    .collect(),
                items: items
                    .iter()
                    .map(|item| item.iter().map(|inner| self.block(inner)).collect())
                    .collect(),
            },
        }
    }

    fn prose(&mut self, inlines: &[Inline], size: f32, weight: Weight, muted: bool) -> Shaped {
        let mut pieces = Vec::new();
        let mut text_run = TextRun::default();
        for inline in inlines {
            self.inline(inline, Marks::default(), &mut text_run, &mut pieces);
        }
        if !text_run.text.is_empty() {
            pieces.push(Piece::Text(text_run));
        }
        Shaped::Prose {
            pieces,
            size: match self.big {
                true => BIG * 0.85,
                false => size,
            },
            weight,
            muted,
        }
    }

    fn inline(
        &mut self,
        inline: &Inline,
        marks: Marks,
        text_run: &mut TextRun,
        pieces: &mut Vec<Piece>,
    ) {
        match inline {
            Inline::Text(caption) => {
                leaf(text_run, caption, marks, Extra::default(), &self.targets)
            }
            Inline::Bold(inner) => self.inner(
                inner,
                Marks {
                    bold: true,
                    ..marks
                },
                text_run,
                pieces,
            ),
            Inline::Italic(inner) => self.inner(
                inner,
                Marks {
                    italic: true,
                    ..marks
                },
                text_run,
                pieces,
            ),
            Inline::Underline(inner) => self.inner(
                inner,
                Marks {
                    underline: true,
                    ..marks
                },
                text_run,
                pieces,
            ),
            Inline::Strike(inner) => self.inner(
                inner,
                Marks {
                    strike: true,
                    ..marks
                },
                text_run,
                pieces,
            ),
            Inline::Spoiler(inner) => {
                let spoiler = self.spoilers.min(MAX_SPOILERS);
                self.spoilers += 1;
                self.inner(
                    inner,
                    Marks {
                        spoiler: Some(spoiler),
                        ..marks
                    },
                    text_run,
                    pieces,
                );
            }
            Inline::Code(caption) => leaf(
                text_run,
                caption,
                marks,
                Extra {
                    fill: Some(palette().surface_variant),
                    mono: true,
                    size: Some(CODE),
                    ..Extra::default()
                },
                &self.targets,
            ),
            Inline::Link { url, label } => {
                self.targets.push(Target::Url(url.clone()));
                let linked = Marks {
                    link: Some(self.targets.len() - 1),
                    ..marks
                };
                match label.is_empty() {
                    true => leaf(
                        text_run,
                        url,
                        linked,
                        Extra {
                            color: Some(palette().accent),
                            ..Extra::default()
                        },
                        &self.targets,
                    ),
                    false => self.inner(label, linked, text_run, pieces),
                }
            }
            Inline::Mention(mention) => self.mention(mention, marks, text_run),
            Inline::Emoji(emoji) => match &emoji.picture {
                Some(picture) if emoji.is_custom() => {
                    if !text_run.text.is_empty() {
                        pieces.push(Piece::Text(std::mem::take(text_run)));
                    }
                    pieces.push(Piece::Emoji {
                        picture: picture.clone(),
                        side: match self.big {
                            true => BIG,
                            false => 22.0,
                        },
                        animate: self.animate && picture.is_animated(),
                    });
                }
                _ => leaf(
                    text_run,
                    &emoji.name,
                    marks,
                    Extra {
                        size: self.big.then_some(BIG * 0.85),
                        ..Extra::default()
                    },
                    &self.targets,
                ),
            },
            Inline::Time { unix, style } => {
                if *style == discord_core::TimeStyle::Relative {
                    self.relative = true;
                }
                leaf(
                    text_run,
                    &time::styled(*unix, *style),
                    marks,
                    Extra {
                        fill: Some(palette().surface_variant),
                        ..Extra::default()
                    },
                    &self.targets,
                );
            }
            Inline::Break => leaf(text_run, "\n", marks, Extra::default(), &self.targets),
        }
    }

    fn inner(
        &mut self,
        inner: &[Inline],
        marks: Marks,
        text_run: &mut TextRun,
        pieces: &mut Vec<Piece>,
    ) {
        for inline in inner {
            self.inline(inline, marks, text_run, pieces);
        }
    }

    fn mention(&mut self, mention: &Mention, marks: Marks, text_run: &mut TextRun) {
        let accent = palette().accent;
        let (label, color, target) = match mention {
            Mention::User(user) => (
                format!(
                    "@{}",
                    self.linked
                        .name_of(self.space, user)
                        .unwrap_or_else(|| tr!("discord.conversation.unknown-user").to_string())
                ),
                self.linked
                    .color_of(self.space, user)
                    .map_or(accent, Color::hex),
                Target::User(*user),
            ),
            Mention::Role(role) => {
                let role_info = self.space.and_then(|space| self.linked.role(space, role));
                (
                    format!(
                        "@{}",
                        role_info.map_or_else(
                            || tr!("discord.conversation.unknown-role").to_string(),
                            |role_info| role_info.name.clone()
                        )
                    ),
                    role_info
                        .and_then(|role_info| role_info.color)
                        .map_or(accent, Color::hex),
                    Target::Nothing,
                )
            }
            Mention::Channel(channel) => (
                format!(
                    "#{}",
                    self.linked.channel(channel).map_or_else(
                        || tr!("discord.conversation.unknown-channel").to_string(),
                        |role_info| role_info.name.clone()
                    )
                ),
                accent,
                Target::Channel(*channel),
            ),
            Mention::Everyone => ("@everyone".to_owned(), accent, Target::Nothing),
            Mention::Here => ("@here".to_owned(), accent, Target::Nothing),
        };
        self.targets.push(target);
        leaf(
            text_run,
            &label,
            Marks {
                link: Some(self.targets.len() - 1),
                ..marks
            },
            Extra {
                color: Some(color),
                fill: Some(color.with_alpha(0.18)),
                weight: Some(font::MEDIUM),
                ..Extra::default()
            },
            &self.targets,
        );
    }
}

fn leaf(text_run: &mut TextRun, caption: &str, marks: Marks, extra: Extra, targets: &[Target]) {
    if caption.is_empty() {
        return;
    }
    let start = text_run.text.len();
    text_run.text.push_str(caption);
    let range = start..text_run.text.len();
    let mut span = TextSpan::new(range.clone());
    if marks.bold {
        span = span.weight(font::BOLD);
    }
    if let Some(weight) = extra.weight {
        span = span.weight(weight);
    }
    if marks.italic {
        span = span.style(FontStyle::Italic);
    }
    if marks.underline {
        span = span.underline(true);
    }
    if marks.strike {
        span = span.strikethrough(true);
    }
    if extra.mono {
        span = span.family(Family::parse("monospace"));
    }
    if let Some(size) = extra.size {
        span = span.font_size(size);
    }
    let link = marks.link.and_then(|at| targets.get(at)).cloned();
    match (extra.color, &link) {
        (Some(color), _) => span = span.color(color),
        (None, Some(Target::Url(_))) => span = span.color(palette().accent),
        _ => {}
    }
    text_run.leaves.push((span, marks.spoiler));
    if extra.fill.is_some() || marks.spoiler.is_some() {
        text_run
            .fills
            .push((range.clone(), extra.fill, marks.spoiler));
    }
    if link.is_some() || marks.spoiler.is_some() {
        text_run
            .links
            .push((range, link.unwrap_or(Target::Nothing), marks.spoiler));
    }
}

#[derive(Clone)]
pub(crate) struct Reveal {
    pub(crate) revealed: u64,
    pub(crate) set: Setter<u64>,
    pub(crate) subject: u64,
    pub(crate) space: Option<SpaceId>,
}

impl Reveal {
    fn shows(&self, spoiler: Option<u32>) -> bool {
        spoiler.is_none_or(|at| self.revealed & (1 << at) != 0)
    }
}

pub(crate) fn rich_view(
    linked: &LinkedAccount,
    space: Option<&SpaceId>,
    rich: &Rich,
    subject: u64,
    animate: bool,
) -> Element {
    let shaped = Shaping::new(linked, space, animate).rich(rich);
    let space = space.copied();
    local_state(0_u64, move |revealed: u64, set: Setter<u64>| {
        render(
            &shaped,
            &Reveal {
                revealed,
                set,
                subject,
                space,
            },
        )
    })
}

pub(crate) fn render(shaped: &[Shaped], reveal: &Reveal) -> Element {
    column_of(shaped.iter().map(|block| render_block(block, reveal)))
        .gap(Space::TIGHT)
        .fill_width()
        .any()
}

fn render_block(block: &Shaped, reveal: &Reveal) -> Element {
    match block {
        Shaped::Prose {
            pieces,
            size,
            weight,
            muted,
        } => prose(pieces, *size, *weight, *muted, reveal),
        Shaped::Quote(blocks) => row! {
            container(spacer())
                .background(palette().outline)
                .radius(2.0)
                .width(4.0)
                .fill_height();
            render(blocks, reveal).grow(1.0).min_width(0.0);
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        Shaped::Code {
            language,
            text: code,
            spans,
        } => {
            let copied = code.clone();
            container(
                column! {
                    row! {
                        text(language.clone().unwrap_or_default()).caption().muted().no_wrap().grow(1.0);
                        button(icon(MaterialSymbol::CONTENT_COPY).extent(Length::px(14.0)))
                            .icon_only()
                            .ghost()
                            .small()
                            .focusless()
                            .on_press(move || {
                                rikka_ui::clipboard::set_text(&copied);
                                skip_view();
                            });
                    }
                    .align_items(Align::Center)
                    .fill_width();
                    rich_text(code.clone())
                        .spans(spans.clone())
                        .family(Family::parse("monospace"))
                        .text_size(CODE)
                        .line_height((CODE * 1.45).round())
                        .selectable(("discord-code", reveal.subject, code.len()))
                        .fill_width();
                }
                .gap(Space::HAIR)
                .fill_width(),
            )
            .padding(Inset::xy(Space::SNUG, Space::TIGHT))
            .background(palette().surface_variant)
            .border(1.0, palette().outline.with_alpha(0.5))
            .radius(Corner::Item)
            .fill_width()
            .any()
        }
        Shaped::List { markers, items } => {
            column_of(markers.iter().zip(items).map(|(marker, item)| {
                row! {
                    text(marker.clone()).body().no_wrap().width(22.0);
                    render(item, reveal).grow(1.0).min_width(0.0);
                }
                .align_items(Align::Start)
                .fill_width()
                .any()
            }))
            .gap(Space::HAIR)
            .fill_width()
            .any()
        }
    }
}

fn prose(pieces: &[Piece], size: f32, weight: Weight, muted: bool, reveal: &Reveal) -> Element {
    let color = match muted {
        true => palette().text_muted,
        false => palette().text,
    };
    let leading = (size * LEADING).round();
    let mut parts: Vec<Element> = pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text(text_run) => text_run_view(text_run, size, weight, color, leading, reveal),
            Piece::Emoji {
                picture,
                side,
                animate,
            } => emoji_picture(picture, *side, *animate),
        })
        .collect();
    match parts.len() {
        1 => match &pieces[0] {
            Piece::Text(_) => parts.remove(0).fill_width().any(),
            Piece::Emoji { .. } => parts.remove(0),
        },
        _ => row_of(parts)
            .wrap_items()
            .align_items(Align::Center)
            .fill_width()
            .any(),
    }
}

fn text_run_view(
    text_run: &TextRun,
    size: f32,
    weight: Weight,
    color: Color,
    leading: f32,
    reveal: &Reveal,
) -> Element {
    let hidden = palette().text_muted;
    let spans: Vec<TextSpan> = text_run
        .leaves
        .iter()
        .map(|(span, spoiler)| match reveal.shows(*spoiler) {
            true => span.clone(),
            false => span.clone().color(hidden),
        })
        .collect();
    let fills: Vec<SpanFill> = text_run
        .fills
        .iter()
        .map(|(range, fill, spoiler)| {
            let color = match (reveal.shows(*spoiler), fill, spoiler) {
                (false, _, _) => hidden,
                (true, Some(fill), _) => *fill,
                (true, None, _) => palette().text.with_alpha(0.08),
            };
            SpanFill::new(range.clone(), color)
        })
        .collect();
    let mut ranges = Vec::new();
    let mut targets = Vec::new();
    for (range, target, spoiler) in &text_run.links {
        match (reveal.shows(*spoiler), spoiler) {
            (false, Some(at)) => {
                ranges.push(range.clone());
                targets.push(Err(*at));
            }
            _ if matches!(target, Target::Nothing) => {}
            _ => {
                ranges.push(range.clone());
                targets.push(Ok(target.clone()));
            }
        }
    }
    let mut run = rich_text(text_run.text.clone())
        .spans(spans)
        .fills(fills)
        .text_size(size)
        .weight(weight)
        .color(color)
        .line_height(leading)
        .selectable(("discord-text", reveal.subject));
    if !ranges.is_empty() {
        let reveal = reveal.clone();
        run = run
            .links(ranges)
            .on_link(
                move |chat: &mut Chat, index: usize| match targets.get(index) {
                    Some(Err(spoiler)) => (reveal.set)(reveal.revealed | (1 << spoiler)),
                    Some(Ok(Target::Url(url))) => open_link(url),
                    Some(Ok(Target::User(user))) => {
                        chat.open_profile(user, reveal.space.as_ref(), Anchor::centered_on(AREA));
                    }
                    Some(Ok(Target::Channel(channel))) => chat.open_channel(channel),
                    Some(Ok(Target::Nothing)) | None => {}
                },
            );
    }
    run.any()
}

fn emoji_picture(picture: &Picture, side: f32, animate: bool) -> Element {
    let face = match animate {
        true => frames(picture, side, Size::new(side, side))
            .map(|frames| animated(frames, ImageFit::Contain)),
        false => {
            pictures::fetched(picture, side).map(|bytes| image(bytes).fit(ImageFit::Contain).any())
        }
    };
    face.unwrap_or_else(|| spacer().any())
        .size(Length::px(side), Length::px(side))
        .any()
}
