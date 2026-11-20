use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::pipeline::glyph_flags::propagate_flags;
use crate::pipeline::ignorables::hide_default_ignorables;
use crate::pipeline::position::position;
use crate::pipeline::substitute::{substitute_by_plan, substitute_default};
use crate::pipeline::unicode_props::{
    ensure_native_direction, form_clusters, insert_dotted_circle, set_unicode_props,
};
use crate::plan::shape_plan::PlanCore;

pub(crate) fn shape(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    let properties = plan.properties;
    buffer.enter(properties.direction, properties.script);
    if buffer.len > 0 {
        shape_text(plan, font, buffer);
    }
    buffer.direction = properties.direction;
    buffer.leave();
}

fn shape_text(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    buffer.reset_masks(plan.map.global_mask);
    set_unicode_props(buffer);
    insert_dotted_circle(buffer, font);
    form_clusters(buffer);
    ensure_native_direction(buffer);
    plan.shaper.preprocess_text(plan, font, buffer);
    substitute_default(plan, font, buffer);
    substitute_by_plan(plan, font, buffer);
    position(plan, font, buffer);
    hide_default_ignorables(font, buffer);
    plan.shaper.postprocess_glyphs(plan, font, buffer);
    propagate_flags(buffer);
}
