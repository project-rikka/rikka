use animate_cloth::Tool;
use animate_stage::{Lens, PaintSurface};
use animate_ui::disc;
use rikka_app::prelude::*;
use rikka_physics_tailor::{AnalyzedPiece, ClothAnalysis, ClothSettings, PieceKey, Role};
use rustc_hash::FxHashMap;

const CELL: f32 = 4.0;
const DOT: f32 = 2.6;
const CHOSEN_DOT: f32 = 3.6;
const RING: f32 = 1.5;
const FULL_SPREAD: f32 = 2.0;

const FREE: Color = Color::rgb(0.3, 0.75, 1.0);
const PINNED: Color = Color::rgb(1.0, 0.55, 0.1);
const SKINNED: Color = Color::rgb(0.75, 0.5, 1.0);
const BODY: Color = Color::rgb(0.85, 0.85, 0.85);
const CHOSEN: Color = Color::rgb(1.0, 0.9, 0.2);
const NARROW: Color = Color::rgb(0.3, 0.5, 1.0);
const WIDE: Color = Color::rgb(1.0, 0.3, 0.3);

pub(crate) struct ClothOverlay<'a> {
    pub paint_surface: &'a PaintSurface,
    pub cloth_settings: &'a ClothSettings,
    pub analysis: Option<&'a ClothAnalysis>,
    pub piece: Option<PieceKey>,
    pub tool: Tool,
    pub brush: Option<(Point, f32)>,
}

pub(crate) fn paint(ctx: &mut PaintCtx<'_>, shown: &ClothOverlay<'_>, frame: Rect) {
    let lens = Lens::new(shown.paint_surface.view_projection, frame);
    let surface = &shown.paint_surface.surface;
    let chosen: Option<&AnalyzedPiece> = shown.piece.and_then(|key| {
        shown
            .analysis?
            .pieces()
            .iter()
            .find(|piece| piece.key == key)
    });
    let mut nearest: FxHashMap<(i32, i32), (f32, usize, Point)> = FxHashMap::default();
    for (index, point) in surface.points().iter().enumerate() {
        let toward = shown.paint_surface.eye - *point;
        if surface.normals()[index].dot(toward) <= 0.0 {
            continue;
        }
        let Some(at) = lens.project(*point) else {
            continue;
        };
        if !frame.contains(at) {
            continue;
        }
        let cell = ((at.x / CELL).floor() as i32, (at.y / CELL).floor() as i32);
        let away = toward.length_squared();
        let slot = nearest.entry(cell).or_insert((away, index, at));
        if away < slot.0 {
            *slot = (away, index, at);
        }
    }
    let roles: Vec<Role> = shown
        .analysis
        .map(|analysis| analysis.vertices().map(|(_, role)| role).collect())
        .unwrap_or_default();
    ctx.clipped(frame, |ctx| {
        for (_, index, at) in nearest.values() {
            let key = surface.keys()[*index];
            let paint = shown.cloth_settings.paint_at(key);
            if chosen.is_some_and(|piece| piece.contains(key)) {
                ctx.draw_quad(disc(*at, CHOSEN_DOT / 2.0).fill(CHOSEN));
                continue;
            }
            let role = paint.role.or_else(|| roles.get(*index).copied());
            let tint = match (shown.tool, paint.spread) {
                (Tool::Spread, Some(spread)) => {
                    let share = (spread / FULL_SPREAD).clamp(0.0, 1.0);
                    Some(NARROW.mix(WIDE, share))
                }
                (Tool::Spread, None) => role.map(|role| tinted(role).scale_alpha(0.35)),
                _ => role.map(tinted),
            };
            if let Some(tint) = tint {
                ctx.draw_quad(disc(*at, DOT / 2.0).fill(tint));
            }
        }
        if let Some((at, radius)) = shown.brush {
            let tint = match shown.tool.role() {
                Some(role) => tinted(role).with_alpha(1.0),
                None => WIDE,
            };
            ctx.draw_quad(
                Quad::new(Rect::from_circle(at, radius))
                    .radius(radius)
                    .stroke(RING, tint),
            );
        }
    });
}

fn tinted(role: Role) -> Color {
    match role {
        Role::Free => FREE,
        Role::Pinned => PINNED,
        Role::Skinned => SKINNED.scale_alpha(0.5),
        Role::Body => BODY.scale_alpha(0.3),
    }
}
