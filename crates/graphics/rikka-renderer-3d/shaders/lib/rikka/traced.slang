__exported import rikka.surface;

static const float RIKKA_TRACED_TOLERANCE = 0.02;

bool rikka_traced_available(RikkaFrameRef frame) {
    return frame.look.scene.enabled != 0u && frame.look.trace.visibility_image != RIKKA_NONE;
}

bool rikka_traced_projects(RikkaViewRef view, float3 world_position, out float2 at) {
    at = float2(0.0);
    float4 clip = mul(view.view_projection, float4(world_position, 1.0));
    if (clip.w <= 0.0) {
        return false;
    }
    float2 uv = clip.xy / clip.w * 0.5 + 0.5;
    at = uv * view.viewport;
    return all(uv >= float2(0.0)) && all(uv < float2(1.0));
}

bool rikka_traced_shows(RikkaFrameRef frame, int2 pixel, float view_depth) {
    float depth = rikka_read_depth(frame.look.scene.depth, pixel);
    float tolerance = RIKKA_TRACED_TOLERANCE * abs(view_depth) + 0.01;
    return depth > 0.0 && abs(rikka_linear_depth(frame.view, depth) - view_depth) <= tolerance;
}

int2 rikka_traced_pixel_at(RikkaViewRef view, float2 at) {
    return clamp(int2(at), int2(0), int2(view.viewport) - 1);
}

bool rikka_traced_seen(RikkaFrameRef frame, float3 world_position, float view_depth, out int2 pixel) {
    float2 at;
    bool inside = rikka_traced_projects(frame.view, world_position, at);
    pixel = rikka_traced_pixel_at(frame.view, at);
    return inside && rikka_traced_shows(frame, pixel, view_depth);
}

float4 rikka_traced_image(RikkaFrameRef frame, int2 pixel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(frame.look.trace.visibility_image)], pixel, 0);
}

float rikka_traced_shadow(RikkaFrameRef frame, uint shadow, uint traced, float3 world_position, float view_depth) {
    if (!rikka_traced_available(frame)) {
        return -1.0;
    }
    RikkaTrace trace = frame.look.trace;
    uint channel = RIKKA_TRACED_LIGHTS;
    [unroll]
    for (uint c = 0u; c < RIKKA_TRACED_LIGHTS; ++c) {
        if (trace.shadow_lights[c] == shadow) {
            channel = c;
        }
    }
    if (channel == RIKKA_TRACED_LIGHTS) {
        return -1.0;
    }
    float known = rikka_traced_unpack(traced, channel);
    if (known >= 0.0) {
        return known;
    }
    int2 pixel;
    if (!rikka_traced_seen(frame, world_position, view_depth, pixel)) {
        return -1.0;
    }
    return rikka_traced_image(frame, pixel)[channel];
}

float3 rikka_traced_indirect(RikkaFrameRef frame, float2 uv, out bool present) {
    RikkaTrace trace = frame.look.trace;
    present = frame.look.scene.enabled != 0u && trace.indirect_image != RIKKA_NONE;
    if (!present) {
        return float3(0.0);
    }
    float3 irradiance = rikka_images[NonUniformResourceIndex(trace.indirect_image)]
        .SampleLevel(rikka_samplers[1], rikka_render_uv(frame.view, uv, 1u), 0.0).rgb;
    return irradiance * trace.indirect_strength;
}
