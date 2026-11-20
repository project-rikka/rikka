__exported import rikka.common;
__exported import rikka.editor;

static const float RIKKA_OVERLAY_FRINGE = 1.0;
static const float4 RIKKA_OVERLAY_CULLED = float4(0.0, 0.0, -2.0, 1.0);

struct RikkaOverlayBatch {
    float4x4 transform;
    uint64_t positions;
    uint64_t colors;
    uint64_t indices;
    uint first;
    float width;
    float bias;
    float dash;
    float gap;
    float outline_width;
    uint outline_color;
    uint shape;
    uint shading;
    uint clipped;
    uint aliased;
    uint pad;
};

typealias RikkaOverlayBatchRef = RikkaOverlayBatch*;

struct RikkaOverlayPush {
    RikkaFrameRef frame;
    RikkaOverlayBatchRef batch;
    float alpha;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<RikkaOverlayPush> rikka_overlay;

uint rikka_overlay_vertex(RikkaOverlayBatchRef batch, uint element) {
    uint at = batch.first + element;
    if (batch.indices != 0ul) {
        return ((uint*)batch.indices)[at];
    }
    return at;
}

float3 rikka_overlay_view_position(RikkaViewRef view, RikkaOverlayBatchRef batch, uint vertex) {
    float3 local = ((float3*)batch.positions)[vertex];
    float4 world = mul(batch.transform, float4(local, 1.0));
    return mul(view.view, world).xyz;
}

float3 rikka_overlay_world(RikkaOverlayBatchRef batch, uint vertex) {
    float3 local = ((float3*)batch.positions)[vertex];
    return mul(batch.transform, float4(local, 1.0)).xyz;
}

bool rikka_overlay_clipped(RikkaOverlayBatchRef batch, float3 world) {
    return batch.clipped != 0u && rikka_clipped(rikka_overlay.frame, world);
}

float4 rikka_overlay_color(RikkaOverlayBatchRef batch, uint vertex) {
    return unpackUnorm4x8ToFloat(((uint*)batch.colors)[vertex]);
}

bool rikka_overlay_perspective(RikkaViewRef view) {
    return view.projection[3][3] < 0.5;
}

float4 rikka_overlay_clip(RikkaViewRef view, float3 position, float bias) {
    float3 biased = rikka_overlay_perspective(view)
        ? position * (1.0 - bias)
        : float3(position.xy, position.z + bias * abs(position.z));
    return mul(view.projection, float4(biased, 1.0));
}

float2 rikka_overlay_screen(RikkaViewRef view, float4 clip) {
    return (clip.xy / clip.w * 0.5 + 0.5) * view.viewport;
}

float4 rikka_overlay_offset(RikkaViewRef view, float4 clip, float2 pixels) {
    return float4(clip.xy + pixels * 2.0 * view.inverse_viewport * clip.w, clip.zw);
}

bool rikka_overlay_visible(float4 clip) {
    return clip.w > 0.0 && clip.w - clip.z >= 0.0;
}

float4 rikka_overlay_premultiplied(float4 color) {
    return float4(color.rgb * color.a, color.a);
}

float rikka_overlay_coverage(RikkaOverlayBatchRef batch, float signed_distance) {
    return batch.aliased != 0u
        ? step(0.0, signed_distance)
        : clamp(signed_distance + 0.5, 0.0, 1.0);
}
