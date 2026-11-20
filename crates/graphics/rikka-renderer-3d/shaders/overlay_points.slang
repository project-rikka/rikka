import rikka.overlay;

static const uint SHAPE_DISC = 0u;

struct PointVaryings {
    [[vk::location(0)]] nointerpolation float4 color;
    [[vk::location(1)]] noperspective float2 local;
};

static const float2 CORNERS[6] = {
    float2(-1.0, -1.0),
    float2(1.0, -1.0),
    float2(-1.0, 1.0),
    float2(-1.0, 1.0),
    float2(1.0, -1.0),
    float2(1.0, 1.0),
};

[shader("vertex")]
PointVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaOverlayBatchRef batch = rikka_overlay.batch;
    RikkaViewRef view = rikka_overlay.frame.view;
    uint vertex = rikka_overlay_vertex(batch, vertex_index / 6u);
    float2 corner = CORNERS[vertex_index % 6u];
    float4 clip = rikka_overlay_clip(view, rikka_overlay_view_position(view, batch, vertex), batch.bias);

    PointVaryings out;
    out.color = rikka_overlay_color(batch, vertex);
    out.color.a *= rikka_overlay.alpha;
    if (!rikka_overlay_visible(clip) || rikka_overlay_clipped(batch, rikka_overlay_world(batch, vertex))) {
        out.local = float2(0.0);
        position = RIKKA_OVERLAY_CULLED;
        return out;
    }
    float extent = batch.width * 0.5 + RIKKA_OVERLAY_FRINGE;
    out.local = corner * extent;
    position = rikka_overlay_offset(view, clip, out.local);
    return out;
}

[shader("fragment")]
float4 fragment_main(PointVaryings varyings) : SV_Target {
    RikkaOverlayBatchRef batch = rikka_overlay.batch;
    float radius = batch.width * 0.5;
    float2 local = varyings.local;
    float distance = batch.shape == SHAPE_DISC ? length(local) : max(abs(local.x), abs(local.y));
    float coverage = rikka_overlay_coverage(batch, radius - distance);
    float4 color = varyings.color;
    float outline_width = batch.outline_width;
    if (outline_width > 0.0) {
        float4 outline = unpackUnorm4x8ToFloat(batch.outline_color);
        outline.a *= rikka_overlay.alpha;
        float inner = rikka_overlay_coverage(batch, radius - outline_width - distance);
        color = lerp(outline, color, inner);
    }
    float alpha = color.a * coverage;
    if (!(alpha > 0.0)) {
        discard;
    }
    return float4(color.rgb * alpha, alpha);
}
