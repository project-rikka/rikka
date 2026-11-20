import rikka.overlay;

struct LineVaryings {
    [[vk::location(0)]] noperspective float4 color;
    [[vk::location(1)]] noperspective float across;
    [[vk::location(2)]] noperspective float along;
    [[vk::location(3)]] float3 world;
};

static const uint CORNER_END[6] = { 0u, 0u, 1u, 1u, 0u, 1u };
static const float CORNER_SIDE[6] = { -1.0, 1.0, -1.0, -1.0, 1.0, 1.0 };

[shader("vertex")]
LineVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaOverlayBatchRef batch = rikka_overlay.batch;
    RikkaViewRef view = rikka_overlay.frame.view;
    uint segment = vertex_index / 6u;
    uint corner = vertex_index % 6u;
    uint a = rikka_overlay_vertex(batch, segment * 2u);
    uint b = rikka_overlay_vertex(batch, segment * 2u + 1u);
    float bias = batch.bias;
    float4 start = rikka_overlay_clip(view, rikka_overlay_view_position(view, batch, a), bias);
    float4 end = rikka_overlay_clip(view, rikka_overlay_view_position(view, batch, b), bias);
    float4 start_color = rikka_overlay_color(batch, a);
    float4 end_color = rikka_overlay_color(batch, b);
    float3 start_world = rikka_overlay_world(batch, a);
    float3 end_world = rikka_overlay_world(batch, b);

    LineVaryings out;
    out.color = float4(0.0);
    out.across = 0.0;
    out.along = 0.0;
    out.world = start_world;
    float start_ahead = start.w - start.z;
    float end_ahead = end.w - end.z;
    if (start_ahead < 0.0 && end_ahead < 0.0) {
        position = RIKKA_OVERLAY_CULLED;
        return out;
    }
    if (start_ahead < 0.0) {
        float t = start_ahead / (start_ahead - end_ahead);
        start = lerp(start, end, t);
        start_color = lerp(start_color, end_color, t);
        start_world = lerp(start_world, end_world, t);
    } else if (end_ahead < 0.0) {
        float t = end_ahead / (end_ahead - start_ahead);
        end = lerp(end, start, t);
        end_color = lerp(end_color, start_color, t);
        end_world = lerp(end_world, start_world, t);
    }
    if (!(start.w > 0.0) || !(end.w > 0.0)) {
        position = RIKKA_OVERLAY_CULLED;
        return out;
    }

    float2 from = rikka_overlay_screen(view, start);
    float2 to = rikka_overlay_screen(view, end);
    float2 delta = to - from;
    float span = length(delta);
    float2 direction = span > 1e-4 ? delta / span : float2(1.0, 0.0);
    float2 normal = float2(-direction.y, direction.x);
    float width = max(batch.width, 1.0);
    float extent = width * 0.5 + RIKKA_OVERLAY_FRINGE;
    bool at_end = CORNER_END[corner] == 1u;
    float side = CORNER_SIDE[corner];

    position = rikka_overlay_offset(view, at_end ? end : start, normal * (side * extent));
    out.color = at_end ? end_color : start_color;
    out.world = at_end ? end_world : start_world;
    out.color.a *= min(batch.width, 1.0) * rikka_overlay.alpha;
    out.across = side * extent;
    out.along = at_end ? span : 0.0;
    return out;
}

[shader("fragment")]
float4 fragment_main(LineVaryings varyings) : SV_Target {
    RikkaOverlayBatchRef batch = rikka_overlay.batch;
    if (rikka_overlay_clipped(batch, varyings.world)) {
        discard;
    }
    float half_width = max(batch.width, 1.0) * 0.5;
    float coverage = rikka_overlay_coverage(batch, half_width - abs(varyings.across));
    float dash = batch.dash;
    if (dash > 0.0) {
        float period = dash + batch.gap;
        float phase = rikka_mod(varyings.along, period);
        float inside = min(phase, dash - phase);
        float outside = min(phase - dash, period - phase);
        float distance = phase <= dash ? inside : -outside;
        coverage *= rikka_overlay_coverage(batch, distance);
    }
    float alpha = varyings.color.a * coverage;
    if (!(alpha > 0.0)) {
        discard;
    }
    return float4(varyings.color.rgb * alpha, alpha);
}
