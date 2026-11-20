import rikka.overlay;

static const uint SHADING_FACING = 1u;
static const float3 FACING_LIGHT = float3(0.1, 0.1, 0.8);
static const float FACING_SMOOTH = 0.2;
static const float FACING_DARK = 0.5;

struct TriangleVaryings {
    [[vk::location(0)]] float4 color;
    [[vk::location(1)]] float3 world;
};

[shader("vertex")]
TriangleVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaOverlayBatchRef batch = rikka_overlay.batch;
    RikkaViewRef view = rikka_overlay.frame.view;
    uint triangle = vertex_index / 3u;
    uint vertex = rikka_overlay_vertex(batch, vertex_index);
    float3 here = rikka_overlay_view_position(view, batch, vertex);
    position = rikka_overlay_clip(view, here, batch.bias);

    TriangleVaryings out;
    out.color = rikka_overlay_color(batch, vertex);
    out.color.a *= rikka_overlay.alpha;
    out.world = rikka_overlay_world(batch, vertex);
    if (batch.shading == SHADING_FACING) {
        float3 a = rikka_overlay_view_position(view, batch, rikka_overlay_vertex(batch, triangle * 3u));
        float3 b = rikka_overlay_view_position(view, batch, rikka_overlay_vertex(batch, triangle * 3u + 1u));
        float3 c = rikka_overlay_view_position(view, batch, rikka_overlay_vertex(batch, triangle * 3u + 2u));
        float3 face = cross(b - a, c - a);
        float area = length(face);
        float3 n = area > 1e-12 ? face / area : float3(0.0, 0.0, 1.0);
        float3 toward = rikka_overlay_perspective(view) ? -here : float3(0.0, 0.0, 1.0);
        if (dot(n, toward) < 0.0) {
            n = -n;
        }
        float lit = clamp(dot(n, FACING_LIGHT) * (1.0 - FACING_SMOOTH) + FACING_SMOOTH, 0.0, 1.0);
        out.color.rgb *= lerp(FACING_DARK, 1.0, lit * lit);
    }
    return out;
}

[shader("fragment")]
float4 fragment_main(TriangleVaryings varyings) : SV_Target {
    if (rikka_overlay_clipped(rikka_overlay.batch, varyings.world)) {
        discard;
    }
    return rikka_overlay_premultiplied(varyings.color);
}
