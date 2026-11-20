import rikka.space.view;
import rikka.space;
import rikka.sky;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint trace;
    uint2 half_size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float NORMAL_POWER = 16.0;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    float4 color = rikka_fetch(rikka_images[push.source], pixel, 0);
    RikkaFrameRef frame = push.frame;
    RikkaScene scene = frame.look.scene;
    RikkaSpace space = frame.look.space;
    float depth = rikka_read_depth(scene.depth, pixel);
    if (depth <= 0.0) {
        return color;
    }
    RikkaSurfaceSample surface = rikka_read_surface(scene.surface, pixel);
    float limit = space.roughness_limit;
    if (surface.reflection <= 0.0 || surface.roughness > limit) {
        return color;
    }
    RikkaViewRef view = frame.view;
    SpaceView v = space_view(frame);
    float2 uv = space_uv(pixel, v.size);
    float3 world = rikka_world_at(view, uv, depth);
    float3 eye = v.orthographic ? -view.camera_forward : normalize(view.camera_position - world);
    float3 normal = surface.normal;
    float n_dot_v = max(dot(normal, eye), 1e-4);
    float fade = 1.0 - smoothstep(limit * 0.6, limit, surface.roughness);
    float weight = clamp(rikka_reflectance(surface.reflection, surface.roughness, n_dot_v) * space.reflection_strength * fade, 0.0, 1.0);
    if (weight <= 0.0) {
        return color;
    }

    float distance = space_distance(v, depth);
    float tolerance = 0.03 * distance + 0.02;
    float2 centre = (float2(pixel) + 0.5) * 0.5 - 0.5;
    float spacing = 1.0 + surface.roughness * 6.0;
    int2 highest = int2(push.half_size) - 1;
    int2 full_highest = int2(v.size) - 1;
    float3 gathered = float3(0.0);
    float confidence = 0.0;
    float total = 0.0;
    [unroll]
    for (int y = -1; y <= 1; ++y) {
        [unroll]
        for (int x = -1; x <= 1; ++x) {
            int2 tap = clamp(int2(round(centre + float2(float(x), float(y)) * spacing)), int2(0), highest);
            int2 tap_pixel = min(tap * 2, full_highest);
            float tap_depth = rikka_read_depth(scene.depth, tap_pixel);
            if (tap_depth <= 0.0) {
                continue;
            }
            float along = exp(-abs(space_distance(v, tap_depth) - distance) / tolerance);
            float3 tap_normal = rikka_read_surface(scene.surface, tap_pixel).normal;
            float facing = pow(max(dot(normal, tap_normal), 0.0), NORMAL_POWER);
            float spatial = exp(-0.5 * float(x * x + y * y));
            float w = along * facing * spatial;
            float4 traced = rikka_fetch(rikka_images[push.trace], tap, 0);
            gathered += traced.rgb * traced.a * w;
            confidence += traced.a * w;
            total += w;
        }
    }
    float3 reflected_ray = reflect(-eye, normal);
    float3 environment = rikka_sky_specular(frame, reflected_ray, surface.roughness);
    float3 hit = confidence > 1e-4 ? gathered / confidence : environment;
    float found = total > 1e-4 ? confidence / total : 0.0;
    float3 reflection = lerp(environment, hit, found) * color.a;
    return float4(lerp(color.rgb, reflection, weight), color.a);
}
