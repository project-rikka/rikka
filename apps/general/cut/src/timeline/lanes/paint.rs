use std::collections::BTreeSet;

use cut_project::{
    AnimationPresets, Clip, ClipId, EditPoint, Layer, Media, MediaId, Mix, Project, Source, Stream,
    Timecode,
};
use rikka_app::prelude::*;

use super::hit::{self, DISCLOSURE};
use super::parts::{self, Parts};
use super::sound::Sound;
use super::strip::{Requests, Strip};
use super::transition_spans::{self, DropTarget};
use super::trimming::Trimming;
use super::wave::{AudioLevel, Wave};
use super::{Disclosure, wedges};
use crate::live::LivePreview;
use crate::media_previews::{Display, MediaPreviews};
use crate::timeline::lanes::grab::{Grab, Moving};
use crate::timeline::markers;
use crate::timeline::rows::{Row, Rows};
use crate::timeline::scale::Scale;
use crate::wording;

const NAME: f32 = 11.5;
const NAME_LINE: f32 = 16.0;
const NAME_PAD: f32 = 6.0;
const FILL: f32 = 0.42;
const SELECTED_FILL: f32 = 0.7;
const CONTENT_SHADE: f32 = 0.22;
const SELECTED_EDGE: f32 = 1.5;
const LIFTED: f32 = 0.3;
const HIDDEN: f32 = 0.45;
const GHOST: f32 = 0.5;
const SHADE: f32 = 0.04;
const LOCKED_SHADE: f32 = 0.12;
const MARKED: f32 = 0.08;
const LINE: f32 = 1.0;
const SNAP_LINE: f32 = 1.5;
const BAND: f32 = 0.16;
const ARRIVAL: f32 = 2.0;
const RANGE_HUE: f32 = 0.45;
const BADGE: f32 = 13.0;
const BADGE_GAP: f32 = 3.0;
const TICK: f32 = 8.0;
const CLIP_FLAG: f32 = 6.0;
const DELTA: f32 = 10.5;
const DELTA_LINE: f32 = 16.0;
const DELTA_WIDTH: f32 = 96.0;

pub(crate) struct Scene<'a> {
    pub(crate) project: &'a Project,
    pub(crate) rows: &'a Rows,
    pub(crate) scale: &'a Scale,
    pub(crate) selected: &'a BTreeSet<ClipId>,
    pub(crate) edit_point: Option<EditPoint>,
    pub(crate) offline: &'a BTreeSet<MediaId>,
    pub(crate) disclosure: Option<Disclosure>,
    pub(crate) media_previews: &'a MediaPreviews,
    pub(crate) display: Display,
    pub(crate) live: &'a LivePreview,
    pub(crate) bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Placed,
    Lifted,
    Ghost,
}

#[derive(Clone, Copy)]
struct ClipBody<'c> {
    layer: &'c Layer,
    clip: &'c Clip,
    look: Look,
    animation_presets: &'c AnimationPresets,
}

impl<'c> ClipBody<'c> {
    fn ghost(layer: &'c Layer, clip: &'c Clip) -> ClipBody<'c> {
        ClipBody {
            layer,
            clip,
            look: Look::Ghost,
            animation_presets: clip.animation_presets(),
        }
    }
}

struct LanePainters<'a> {
    strip: Strip<'a>,
    wave: Wave<'a>,
    requests: Requests,
}

impl Scene<'_> {
    pub(crate) fn paint(
        &self,
        ctx: &mut PaintCtx<'_>,
        grab: Option<&Grab>,
        arrival: Option<(Row, i64)>,
        drop_target: Option<DropTarget>,
    ) {
        let palette = palette();
        self.media_previews.watch();
        ctx.draw_quad(Quad::new(self.bounds).fill(palette.background));
        self.paint_rows(ctx);
        self.paint_marks(ctx);

        let lifted: &[ClipId] = match grab {
            Some(Grab::Moving(moving)) if moving.has_moved() => &moving.clips,
            _ => &[],
        };
        let animation_presets = match grab {
            Some(Grab::PresetResize(animation_presets)) => Some(animation_presets),
            _ => None,
        };
        let visible = ctx.visible();
        let mut painters = LanePainters {
            strip: Strip {
                filmstrips: self.media_previews.filmstrips(),
                mode: self.display.thumbnails,
                rate: self.project.rate(),
                scale: self.scale,
                bounds: self.bounds,
                visible,
                resolution: ctx.scale(),
            },
            wave: Wave {
                loudness: self.display.waveform,
                rate: self.project.rate(),
                scale: self.scale,
                bounds: self.bounds,
                visible,
            },
            requests: Requests::default(),
        };
        for row in self.rows.all() {
            let Some(layer) = self.project.layer(row.layer) else {
                continue;
            };
            for clip in self.visible(layer) {
                let look = match lifted.contains(&clip.id()) {
                    true => Look::Lifted,
                    false => Look::Placed,
                };
                let animation_presets = animation_presets
                    .filter(|animation_presets| animation_presets.clip == clip.id())
                    .map_or(clip.animation_presets(), |animation_presets| {
                        &animation_presets.animation_presets
                    });
                let body = self.body(&row, clip.start(), clip.end());
                let bodied = ClipBody {
                    layer,
                    clip,
                    look,
                    animation_presets,
                };
                self.paint_body(ctx, body, bodied, Some(&mut painters));
            }
        }
        if let Some(filmstrips) = self.media_previews.filmstrips() {
            painters.requests.submit(filmstrips);
        }
        self.paint_passages(ctx, grab);
        match grab {
            Some(Grab::Moving(moving)) if moving.has_moved() => self.paint_moving(ctx, moving),
            Some(Grab::Trimming(trimming)) => self.paint_trimming(ctx, trimming),
            Some(Grab::Banding(banding)) => {
                let band = banding.band.rect().translate(self.origin());
                ctx.draw_quad(
                    Quad::new(band)
                        .fill(palette.selection.scale_alpha(BAND))
                        .stroke(LINE, palette.selection),
                );
            }
            _ => {}
        }
        if let Some((row, frame)) = arrival {
            let x = self.bounds.x() + self.scale.x_of(frame as f64);
            ctx.draw_quad(
                Quad::new(Rect::new(
                    x - ARRIVAL / 2.0,
                    self.bounds.y() + row.top,
                    ARRIVAL,
                    row.height,
                ))
                .fill(palette.accent),
            );
        }
        if let Some(drop_target) = drop_target {
            transition_spans::paint_landing(ctx, self.origin(), drop_target);
        }
    }

    fn origin(&self) -> Point {
        Point::new(self.bounds.x(), self.bounds.y())
    }

    fn paint_passages(&self, ctx: &mut PaintCtx<'_>, grab: Option<&Grab>) {
        let origin = self.origin();
        let mut bars = transition_spans::bars(self.project, self.rows.all(), self.scale);
        if let Some(Grab::TransitionResize(passing)) = grab
            && let Some(bar) = bars
                .iter_mut()
                .find(|bar| bar.edit_point == passing.edit_point())
        {
            let dragged =
                transition_spans::spanned(self.scale, self.rows.layer_row(0), &passing.span());
            bar.rect = Rect::new(
                dragged.x(),
                bar.rect.y(),
                dragged.width(),
                bar.rect.height(),
            );
        }
        let per_frame = self.scale.x_of(1.0) - self.scale.x_of(0.0);
        transition_spans::paint_bars(ctx, origin, &bars, self.edit_point, per_frame);
        if let Some(edit_point) = self
            .edit_point
            .filter(|edit_point| bars.iter().all(|bar| bar.edit_point != *edit_point))
        {
            transition_spans::paint_edit_point(
                ctx,
                origin,
                self.project,
                self.rows,
                self.scale,
                edit_point,
            );
        }
    }

    fn visible<'l>(&self, layer: &'l Layer) -> &'l [Clip] {
        let first = self.scale.first().floor() as i64;
        let last = self.scale.frame_at(self.bounds.width()).ceil() as i64 + 1;
        layer.clips_in(first..last)
    }

    fn paint_rows(&self, ctx: &mut PaintCtx<'_>) {
        let palette = palette();
        for row in self.rows.all() {
            let lane = Rect::new(
                self.bounds.x(),
                self.bounds.y() + row.top,
                self.bounds.width(),
                row.height,
            );
            let locked = self.project.layer(row.layer).is_some_and(Layer::is_locked);
            let shade = match locked {
                true => LOCKED_SHADE,
                false => SHADE,
            };
            ctx.draw_quad(Quad::new(lane).fill(palette.text.scale_alpha(shade)));
            ctx.draw_quad(
                Quad::new(Rect::new(lane.x(), lane.max_y() - LINE, lane.width(), LINE))
                    .fill(palette.outline),
            );
        }
    }

    fn paint_marks(&self, ctx: &mut PaintCtx<'_>) {
        let marks = self.project.marks();
        if marks.mark_in.is_none() && marks.mark_out.is_none() {
            return;
        }
        let from = marks.mark_in.unwrap_or(0) as f64;
        let to = marks
            .mark_out
            .map_or(self.project.end().max(1) as f64, |out| (out + 1) as f64);
        let left = self.bounds.x() + self.scale.x_of(from).max(0.0);
        let right = self.bounds.x() + self.scale.x_of(to).min(self.bounds.width());
        if right <= left {
            return;
        }
        ctx.draw_quad(
            Quad::new(Rect::new(
                left,
                self.bounds.y(),
                right - left,
                self.bounds.height(),
            ))
            .fill(palette().accent.scale_alpha(MARKED)),
        );
    }

    fn body(&self, row: &Row, from: i64, to: i64) -> Rect {
        parts::body(self.scale, row, from, to).translate(self.origin())
    }

    fn frames_of(&self, body: Rect) -> (i64, i64) {
        let start = self.scale.frame_at(body.x() - self.bounds.x()).round() as i64;
        let end = self.scale.frame_at(body.max_x() - self.bounds.x()).round() as i64;
        (start, (end - start).max(1))
    }

    fn paint_body(
        &self,
        ctx: &mut PaintCtx<'_>,
        body: Rect,
        bodied: ClipBody<'_>,
        painters: Option<&mut LanePainters<'_>>,
    ) {
        let ClipBody {
            layer,
            clip,
            look,
            animation_presets,
        } = bodied;
        let palette = palette();
        let media = clip.media().and_then(|media| self.project.media(media));
        let offline = clip
            .media()
            .is_some_and(|media| self.offline.contains(&media));
        let hue = match clip.source() {
            Source::Graphic(_) => palette.warning,
            Source::Region(_) => palette.accent.mix(palette.danger, RANGE_HUE),
            Source::Camera(_) => palette.text_muted,
            Source::Footage { .. } if offline => palette.danger,
            Source::Footage { .. } if media.is_some_and(|media| media.has(Stream::Picture)) => {
                palette.accent
            }
            Source::Footage { .. } => palette.success,
        };
        let selected = self.selected.contains(&clip.id());
        let factor = match (look, layer.is_shown()) {
            (Look::Lifted, _) => LIFTED,
            (Look::Ghost, _) => GHOST,
            (Look::Placed, false) => HIDDEN,
            (Look::Placed, true) => 1.0,
        };
        let fill = hue.scale_alpha(if selected { SELECTED_FILL } else { FILL } * factor);
        let ink = palette.text.scale_alpha(factor);
        ctx.draw_quad(Quad::new(body).fill(fill).radius(Corner::Detail));
        let parts = Parts::layout(body, media.filter(|_| !offline && look != Look::Ghost));
        if let (Some(painters), Some(media)) = (painters, media) {
            self.paint_content(ctx, painters, &parts, clip, media, factor);
        }
        let (start, length) = self.frames_of(body);
        for wedge in wedges::wedges(start, length, animation_presets) {
            wedges::paint(ctx, body, self.bounds.x(), self.scale, &wedge, ink);
        }
        if selected && look == Look::Placed {
            self.paint_keys(ctx, body, clip, ink);
            if parts.picture.is_some() || parts.sound.is_some() {
                ctx.draw_quad(
                    Quad::new(body)
                        .stroke(SELECTED_EDGE, palette.selection)
                        .radius(Corner::Detail),
                );
            }
        }
        if look == Look::Placed {
            self.paint_clip_markers(ctx, &parts, clip);
        }
        let middle = parts.header.center().y;
        let disclosed = self
            .disclosure
            .filter(|disclosure| disclosure.clip == clip.id() && look == Look::Placed)
            .and_then(|disclosure| {
                let (from, to) = (body.x() - self.bounds.x(), body.max_x() - self.bounds.x());
                hit::disclosure(from, to).map(|(left, _)| (left, disclosure.open))
            });
        if let Some((left, open)) = disclosed {
            let symbol = match open {
                true => MaterialSymbol::ARROW_DROP_DOWN,
                false => MaterialSymbol::ARROW_RIGHT,
            };
            ctx.draw_icon(
                Rect::from_center(
                    Point::new(self.bounds.x() + left + DISCLOSURE / 2.0, middle),
                    Size::new(DISCLOSURE, DISCLOSURE),
                ),
                IconGlyph::from(symbol).spec(DISCLOSURE),
                ink,
            );
        }

        if body.width() - NAME_PAD * 2.0 < NAME {
            return;
        }
        let name = match clip.source() {
            Source::Graphic(graphic) => wording::graphic(graphic),
            Source::Region(region) => wording::region(region, clip.effects()),
            Source::Camera(_) => wording::camera().to_string(),
            Source::Footage { .. } => media.map(Media::name).unwrap_or_default(),
        };
        let run = TextRun::new(name, NAME).no_wrap().ellipsized();
        let mut left = body.x().max(self.bounds.x()) + NAME_PAD;
        if disclosed.is_some() {
            left += DISCLOSURE;
        }
        ctx.clipped(parts.header, |ctx| {
            if clip.region().is_some() {
                ctx.draw_icon(
                    Rect::from_center(
                        Point::new(left + BADGE / 2.0, middle),
                        Size::new(BADGE, BADGE),
                    ),
                    IconGlyph::from(MaterialSymbol::AUTO_FIX).spec(BADGE),
                    ink,
                );
                left += BADGE + BADGE_GAP;
            }
            let text = Rect::new(
                left,
                middle - NAME_LINE / 2.0,
                (body.max_x() - left - NAME_PAD).max(0.0),
                NAME_LINE,
            );
            ctx.draw_text(text, run, ink);
        });
    }

    fn paint_content(
        &self,
        ctx: &mut PaintCtx<'_>,
        painters: &mut LanePainters<'_>,
        parts: &Parts,
        clip: &Clip,
        media: &Media,
        factor: f32,
    ) {
        let content = match (parts.picture, parts.sound) {
            (Some(picture), Some(sound)) => picture.union(&sound),
            (Some(only), None) | (None, Some(only)) => only,
            (None, None) => return,
        };
        let shade = palette().background.scale_alpha(CONTENT_SHADE);
        let ink = palette().text;
        let preview = self.live.mix(clip.id());
        let corner = RoundedRect::new(content.union(&parts.header), Corner::Detail);
        ctx.with_opacity(factor, |ctx| {
            ctx.clipped(corner, |ctx| {
                ctx.draw_quad(Quad::new(content).fill(shade));
                if let Some(picture) = parts.picture {
                    painters
                        .strip
                        .paint(ctx, &mut painters.requests, clip, media, picture);
                }
                if let Some(sound) = parts.sound {
                    let waveform = self
                        .media_previews
                        .waveforms()
                        .map(|waveforms| waveforms.waveform(media.path()));
                    let gain = |frame: f64| {
                        let mix = clip.audible_mix(frame, preview).unwrap_or_default();
                        Mix {
                            muted: false,
                            ..mix
                        }
                        .level(frame - clip.start() as f64, clip.length())
                        .gain
                    };
                    let audio = AudioLevel {
                        gain: &gain,
                        muted: preview
                            .or_else(|| clip.treatment().map(|treatment| treatment.mix))
                            .is_some_and(|mix| mix.muted),
                    };
                    if let Some(waveform) = waveform {
                        waveform.with_peaks(|peaks| {
                            painters.wave.paint(ctx, peaks, clip, &audio, sound, ink);
                        });
                    }
                    Sound {
                        scale: self.scale,
                        left: self.bounds.x(),
                        band: sound,
                        clip,
                        preview,
                    }
                    .paint(ctx);
                }
            });
        });
    }

    fn paint_clip_markers(&self, ctx: &mut PaintCtx<'_>, parts: &Parts, clip: &Clip) {
        if clip.markers().is_empty() {
            return;
        }
        let header = parts.header;
        ctx.clipped(header, |ctx| {
            for (_, marker) in clip.markers().iter() {
                let Some(span) = clip.marker_span(marker) else {
                    continue;
                };
                let ink = markers::color(marker.color);
                let from = self.bounds.x() + self.scale.x_of(span.start as f64);
                let to = self.bounds.x() + self.scale.x_of(span.end as f64);
                markers::paint_range(ctx, from, to, header.max_y() - 3.0, ink);
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        from - CLIP_FLAG / 2.0,
                        header.y() + 1.0,
                        CLIP_FLAG,
                        CLIP_FLAG,
                    ))
                    .fill(ink)
                    .radius(Corner::Detail),
                );
                ctx.draw_quad(
                    Quad::new(Rect::new(
                        from - LINE / 2.0,
                        header.y() + 1.0,
                        LINE,
                        header.height() - 1.0,
                    ))
                    .fill(ink),
                );
            }
        });
    }

    fn paint_keys(&self, ctx: &mut PaintCtx<'_>, body: Rect, clip: &Clip, ink: Color) {
        let keyed = clip.keyed_frames();
        if keyed.is_empty() {
            return;
        }
        let y = body.max_y() - TICK / 2.0 - 1.0;
        let spec = IconGlyph::from(MaterialSymbol::DIAMOND)
            .spec(TICK)
            .fill(1.0);
        ctx.clipped(body, |ctx| {
            for frame in keyed {
                let x = self.bounds.x() + self.scale.x_of(frame as f64);
                ctx.draw_icon(
                    Rect::from_center(Point::new(x, y), Size::new(TICK, TICK)),
                    spec,
                    ink,
                );
            }
        });
    }

    fn paint_moving(&self, ctx: &mut PaintCtx<'_>, moving: &Moving) {
        for id in &moving.clips {
            let Some((index, clip)) = self.project.locate(*id) else {
                continue;
            };
            let Some(target) = index.checked_add_signed(moving.across as isize) else {
                continue;
            };
            let row = self.rows.layer_row(target);
            let body = self.body(&row, clip.start() + moving.by, clip.end() + moving.by);
            let layer = &self.project.layers()[index];
            self.paint_body(ctx, body, ClipBody::ghost(layer, clip), None);
        }
        self.paint_snap(ctx, moving.snapped);
    }

    fn paint_trimming(&self, ctx: &mut PaintCtx<'_>, trimming: &Trimming) {
        self.paint_snap(ctx, trimming.snapped);
        let Some(clip) = trimming.trim.moving(self.project).first().copied() else {
            return;
        };
        let Some((layer, _)) = self.project.locate(clip) else {
            return;
        };
        let palette = palette();
        let row = self.rows.layer_row(layer);
        let sign = if trimming.by < 0 { "−" } else { "+" };
        let label = format!(
            "{sign}{}",
            Timecode::from_frame(trimming.by.abs(), self.project.rate())
        );
        let x = self.bounds.x() + self.scale.x_of(trimming.point() as f64);
        let pill = Rect::new(
            x - DELTA_WIDTH / 2.0,
            self.bounds.y() + row.top - DELTA_LINE - 2.0,
            DELTA_WIDTH,
            DELTA_LINE,
        );
        let pill = Rect::new(
            pill.x(),
            pill.y().max(self.bounds.y()),
            pill.width(),
            pill.height(),
        );
        ctx.draw_quad(Quad::new(pill).fill(palette.surface).radius(Corner::Item));
        ctx.draw_text(
            pill,
            TextRun::new(label, DELTA)
                .no_wrap()
                .align(TextAlign::Center),
            palette.text,
        );
    }

    fn paint_snap(&self, ctx: &mut PaintCtx<'_>, snapped: Option<i64>) {
        let Some(frame) = snapped else {
            return;
        };
        let x = self.bounds.x() + self.scale.x_of(frame as f64);
        ctx.draw_quad(
            Quad::new(Rect::new(
                x - SNAP_LINE / 2.0,
                self.bounds.y(),
                SNAP_LINE,
                self.bounds.height(),
            ))
            .fill(palette().warning),
        );
    }
}
