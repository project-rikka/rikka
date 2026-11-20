import rikka.ray;
import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint target;
    uint samples;
    uint2 size;
    float reach;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float BRIGHTEST = 64.0;
static const float REUSE_TOLERANCE = 0.03;

float3 glossy(float3 eye, float3 normal, float roughness, float2 noise) {
    float alpha = max(roughness * roughness, 1e-3);
    float phi = 2.0 * RIKKA_PI * noise.x;
    float cos_theta = sqrt((1.0 - noise.y) / (1.0 + (alpha * alpha - 1.0) * noise.y));
    float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    float3 half_vector = rikka_ray_around(normal, float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta));
    float3 reflected = reflect(-eye, half_vector);
    return dot(reflected, normal) > 0.0 ? reflected : reflect(-eye, normal);
}

float3 on_screen(RikkaFrameRef frame, RikkaRayHit hit, out bool found) {
    found = false;
    RikkaViewRef view = frame.view;
    float4 clip = mul(view.view_projection, float4(hit.position, 1.0));
    if (clip.w <= 0.0) {
        return float3(0.0);
    }
    float2 uv = clip.xy / clip.w * 0.5 + 0.5;
    if (any(uv < float2(0.0)) || any(uv >= float2(1.0))) {
        return float3(0.0);
    }
    int2 pixel = min(int2(uv * view.viewport), int2(view.viewport) - 1);
    RikkaGuide there = rikka_guide(frame, pixel);
    if (!there.covered || abs(there.depth - clip.w) > REUSE_TOLERANCE * clip.w) {
        return float3(0.0);
    }
    found = true;
    return rikka_images[push.source].SampleLevel(rikka_samplers[1], rikka_render_uv(view, uv, 1u), 0.0).rgb;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaTrace trace = frame.look.trace;
    RikkaScene scene = frame.look.scene;
    RikkaViewRef view = frame.view;
    int2 pixel = rikka_full_pixel(texel, 2u, uint2(view.viewport));
    float depth = rikka_read_depth(scene.depth, pixel);
    RikkaSurfaceSample surface = rikka_read_surface(scene.surface, pixel);
    if (depth <= 0.0 || surface.reflection <= 0.0 || surface.roughness > frame.look.space.roughness_limit) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    RikkaGuide guide = rikka_guide(frame, pixel);
    float3 position = rikka_world_of(frame, pixel);
    bool orthographic = view.projection[3][2] == 0.0;
    float3 eye = orthographic ? -view.camera_forward : normalize(view.camera_position - position);
    float3 origin = rikka_ray_lift(position, guide.normal, guide.depth);
    float3 sum = float3(0.0);
    uint samples = max(push.samples, 1u);
    for (uint s = 0u; s < samples; ++s) {
        uint index = trace.frame * samples + s;
        float2 noise = rikka_ray_noise(uint2(pixel), index, 7u);
        float pick = rikka_ray_noise(uint2(pixel), index, 8u).x;
        float3 direction = glossy(eye, guide.normal, surface.roughness, noise);
        RikkaRayHit hit = rikka_ray_closest(frame, origin, direction, push.reach);
        float3 light;
        if (!hit.hit) {
            light = rikka_sky_specular(frame, direction, surface.roughness);
        } else {
            bool found;
            light = on_screen(frame, hit, found);
            if (!found) {
                float2 bounce = rikka_ray_noise(uint2(pixel), index, 9u);
                light = rikka_ray_shade(frame, hit, pick, bounce, push.reach);
            }
        }
        sum += min(light, float3(BRIGHTEST));
    }
    float3 radiance = sum / float(samples);
    if (any(isnan(radiance))) {
        radiance = float3(0.0);
    }
    rikka_storage_rgba16f[push.target][texel] = float4(radiance, 1.0);
}
