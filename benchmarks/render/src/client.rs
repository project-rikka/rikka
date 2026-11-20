use std::sync::Arc;

use rikka_renderer::{
    GroupClip, GroupId, Node, NodeId, OffscreenTarget, Renderer, Result, Scene, Stroke,
};
use rikka_text::{TextLayout, TextPaint, TextSpec};

use crate::desktop::area;
use rikka_types::{Color, Point, Rect, RoundedRect};

pub(crate) const WINDOW: (f32, f32) = (1180.0, 780.0);

pub(crate) const SCALE: f32 = 1.25;

const SIDEBAR: f32 = 240.0;

const HEADER: f32 = 56.0;

const LINE: f32 = 22.0;

const PARAGRAPHS: usize = 120;

const SCROLL_PER_FRAME: f32 = 36.0;

const SPIN_PER_FRAME: f32 = 0.105;

const WORDS: &[&str] = &[
    "render",
    "frame",
    "budget",
    "glyph",
    "atlas",
    "scroll",
    "layout",
    "shape",
    "paint",
    "island",
    "group",
    "offset",
    "window",
    "surface",
    "present",
    "prepare",
    "record",
    "thread",
    "cache",
    "settle",
    "spring",
    "motion",
    "theme",
    "palette",
    "column",
    "visible",
    "measure",
    "über",
    "déjà",
    "Quartz",
    "の",
    "日本語",
    "表示",
    "한국어",
    "중국어",
];

pub(crate) struct Client {
    pub(crate) scene: Scene,
    spinner: NodeId,
    caret: NodeId,
    content: GroupId,
    content_height: f32,
    shown: Rect,
}

fn px(value: f32) -> f32 {
    value * SCALE
}

fn physical(rect: Rect) -> Rect {
    Rect::new(
        px(rect.x()),
        px(rect.y()),
        px(rect.width()),
        px(rect.height()),
    )
}

fn paragraph(index: usize, words: usize) -> String {
    (0..words)
        .map(|at| WORDS[(index * 7 + at * 13 + at * at) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn text(laid: Arc<TextLayout>) -> TextPaint {
    TextPaint::new(laid).scaled(SCALE)
}

impl Client {
    pub(crate) fn extent() -> (u32, u32) {
        (px(WINDOW.0).round() as u32, px(WINDOW.1).round() as u32)
    }

    pub(crate) fn new() -> Client {
        let ink = Color::rgb(0.12, 0.13, 0.15);
        let mut scene = Scene::new();
        scene.set_background(Color::rgb(0.97, 0.97, 0.98));

        scene.insert(
            Node::rect(physical(Rect::new(0.0, 0.0, SIDEBAR, WINDOW.1)))
                .fill(Color::rgb(0.93, 0.93, 0.95)),
        );
        for row in 0..14 {
            let rect = Rect::new(12.0, HEADER + row as f32 * 34.0, SIDEBAR - 24.0, 28.0);
            if row == 2 {
                scene.insert(
                    Node::rounded_rect(physical(rect), px(8.0)).fill(Color::rgb(0.85, 0.88, 0.97)),
                );
            }
            let label = TextSpec::new(paragraph(row, 2), 13.0).layout(None);
            scene.insert(
                Node::text(
                    physical(Rect::new(
                        rect.x() + 12.0,
                        rect.y() + 6.0,
                        rect.width(),
                        18.0,
                    )),
                    text(label),
                )
                .fill(ink),
            );
        }
        let title = TextSpec::new("Rendering budget", 17.0).layout(None);
        scene.insert(
            Node::text(
                physical(Rect::new(SIDEBAR + 24.0, 16.0, 400.0, 24.0)),
                text(title),
            )
            .fill(ink),
        );
        scene.insert(
            Node::rect(physical(Rect::new(
                SIDEBAR,
                HEADER - 1.0,
                WINDOW.0 - SIDEBAR,
                1.0,
            )))
            .fill(Color::rgb(0.86, 0.86, 0.88)),
        );

        let viewport = Rect::new(SIDEBAR, HEADER, WINDOW.0 - SIDEBAR, WINDOW.1 - HEADER);
        let shown = physical(viewport);
        let content = scene.create_group();
        scene.set_group(content, None, Point::ZERO, shown);
        scene.set_group_clip(content, GroupClip::Narrow(RoundedRect::rect(shown)));
        let width = viewport.width() - 48.0;
        let mut y = viewport.y() + 16.0;
        for index in 0..PARAGRAPHS {
            if index % 6 == 0 {
                let card = Rect::new(viewport.x() + 16.0, y, viewport.width() - 32.0, 120.0);
                scene.insert(
                    Node::rounded_rect(physical(card), px(14.0))
                        .fill(Color::WHITE)
                        .stroke(Stroke::new(px(1.0), Color::rgb(0.86, 0.86, 0.9)))
                        .group(content),
                );
                let heading = TextSpec::new(paragraph(index, 4), 15.0).layout(None);
                scene.insert(
                    Node::text(
                        physical(Rect::new(card.x() + 16.0, card.y() + 14.0, width, LINE)),
                        text(heading),
                    )
                    .fill(ink)
                    .group(content),
                );
                y += 132.0;
                continue;
            }
            let laid = TextSpec::new(paragraph(index, 48), 14.0).layout(Some(width));
            let height = laid.height();
            scene.insert(
                Node::text(
                    physical(Rect::new(viewport.x() + 24.0, y, width, height)),
                    text(laid),
                )
                .fill(ink)
                .group(content),
            );
            y += height + 12.0;
        }

        let spinner = scene.insert(
            Node::arc(
                Point::new(px(WINDOW.0 - 36.0), px(HEADER * 0.5)),
                px(10.0),
                0.0,
                4.2,
            )
            .stroke(Stroke::new(px(2.5), Color::rgb(0.25, 0.45, 0.95)))
            .z(10),
        );
        let caret = scene.insert(
            Node::rect(physical(Rect::new(SIDEBAR + 190.0, 18.0, 1.5, 20.0)))
                .fill(ink)
                .z(10),
        );
        Client {
            scene,
            spinner,
            caret,
            content,
            content_height: px(y - viewport.y()),
            shown,
        }
    }

    pub(crate) fn spin(&mut self, frame: usize) {
        let start = frame as f32 * SPIN_PER_FRAME;
        self.scene.edit(self.spinner).arc(start, 4.2);
        if frame.is_multiple_of(96) {
            let shown = (frame / 96).is_multiple_of(2);
            self.scene.edit(self.caret).visible(shown);
        }
    }

    pub(crate) fn scroll(&mut self, frame: usize) {
        let room = (self.content_height - self.shown.height()).max(1.0);
        let travelled = px(SCROLL_PER_FRAME) * frame as f32;
        let lap = travelled % (2.0 * room);
        let by = match lap > room {
            true => 2.0 * room - lap,
            false => lap,
        };
        self.scene
            .set_group(self.content, None, Point::new(0.0, -by.round()), self.shown);
    }

    pub(crate) fn draw(
        &mut self,
        renderer: &mut Renderer,
        onto: &mut OffscreenTarget,
    ) -> Result<f32> {
        renderer
            .draw(&mut self.scene, onto)
            .map(|drawn| area(&drawn.damage))
    }
}
