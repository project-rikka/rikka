import rikka.space.view;
import rikka.denoise;
import rikka.sky;
import rikka.space;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint samples;
    uint2 size;
    float reach;
    uint radiance;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint STEPS = 16u;
static const float BRIGHTEST = 64.0;
static const float SKY_ROUGHNESS = 1.0;
static const float SCREEN_REACH = 4.0;

float3 around(float3 normal, float3 local) {
    float3 helper = abs(normal.z) < 0.999 ? float3(0.0, 0.0, 1.0) : float3(1.0, 0.0, 0.0);
    float3 tangent = normalize(cross(helper, normal));
    float3 bitangent = cross(normal, tangent);
    return tangent * local.x + bitangent * local.y + normal * local.z;
}

float3 cosine(float3 normal, float2 noise) {
    float radius = sqrt(noise.x);
    float phi = 2.0 * RIKKA_PI * noise.y;
    return normalize(around(normal, float3(radius * cos(phi), radius * sin(phi), sqrt(max(1.0 - noise.x, 0.0)))));
}

float3 previous_radiance(RikkaFrameRef frame, float2 hit_uv, int2 hit_pixel, out bool found) {
    found = false;
    RikkaScene scene = frame.look.scene;
    if (push.radiance == RIKKA_NONE || frame.view.cut != 0u) {
        return float3(0.0);
    }
    float3 world = rikka_world_of(frame, hit_pixel);
    float4 clip = mul(scene.previous_view_projection, float4(world, 1.0));
    if (clip.w <= 0.0) {
        return float3(0.0);
    }
    float2 uv = rikka_previous_image_uv(frame.view, clip.xy / clip.w * 0.5 + 0.5);
    if (!space_on_screen(uv)) {
        return float3(0.0);
    }
    found = true;
    return rikka_images[push.radiance]
        .SampleLevel(rikka_samplers[1], rikka_previous_render_uv(frame.view, uv, 2u), 0.0).rgb;
}

float3 incoming(RikkaFrameRef frame, SpaceView v, float3 origin, float3 normal_view, float3 direction_world,
                float ambient, float jitter) {
    RikkaScene scene = frame.look.scene;
    float3 ray = normalize(mul((float3x3)v.view, direction_world));
    float reach = min(push.reach, SCREEN_REACH);
    float near = frame.view.near;
    if (ray.z > 1e-5) {
        reach = min(reach, (-near * 1.01 - origin.z) / ray.z);
    }
    float3 sky = rikka_sky_specular(frame, direction_world, SKY_ROUGHNESS) * ambient;
    if (reach <= 0.0) {
        return sky;
    }
    float3 start = origin + normal_view * (0.002 * -origin.z);
    float3 from = space_project(v, start);
    float3 to = space_project(v, start + ray * reach);
    for (uint step = 1u; step <= STEPS; ++step) {
        float s = (float(step) - 1.0 + jitter) / float(STEPS);
        s = s * s;
        float3 at = lerp(from, to, s);
        if (!space_on_screen(at.xy)) {
            break;
        }
        int2 pixel = space_pixel(at.xy, v.size);
        float scene_depth = rikka_read_depth(scene.depth, pixel);
        if (scene_depth > at.z) {
            float scene_distance = space_distance(v, scene_depth);
            float behind = space_distance(v, at.z) - scene_distance;
            if (behind < 0.1 + 0.05 * scene_distance) {
                bool found;
                float3 color = previous_radiance(frame, at.xy, pixel, found);
                return found ? color : sky;
            }
        }
    }
    return sky;
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
    SpaceView v = space_view(frame);
    int2 pixel = rikka_full_pixel(texel, 2u, uint2(v.size));
    RikkaGuide guide = rikka_guide(frame, pixel);
    if (!guide.covered) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    float depth = rikka_read_depth(frame.look.scene.depth, pixel);
    float2 uv = space_uv(pixel, v.size);
    float3 origin = space_position(v, uv, depth);
    float3 normal_view = normalize(mul((float3x3)v.view, guide.normal));
    uint occlusion = frame.look.space.occlusion_image;
    float ambient = occlusion == RIKKA_NONE
        ? 1.0
        : rikka_images[occlusion].SampleLevel(rikka_samplers[1], rikka_render_uv(frame.view, uv, 1u), 0.0).x;
    float3 sum = float3(0.0);
    uint samples = max(push.samples, 1u);
    for (uint s = 0u; s < samples; ++s) {
        uint index = trace.frame * samples + s;
        uint seed = rikka_pcg_hash(uint(pixel.x) + rikka_pcg_hash(uint(pixel.y) + rikka_pcg_hash(index)));
        float2 noise = fract(float2(rikka_interleaved_gradient(float2(pixel) + float2(float(index % 64u) * 5.588, 0.0)),
                                    float(seed >> 8u) * (1.0 / 16777216.0)));
        float jitter = float(rikka_pcg_hash(seed) >> 8u) * (1.0 / 16777216.0);
        float3 direction = cosine(guide.normal, noise);
        sum += min(incoming(frame, v, origin, normal_view, direction, ambient, jitter), float3(BRIGHTEST));
    }
    float3 irradiance = sum / float(samples);
    if (any(isnan(irradiance))) {
        irradiance = float3(0.0);
    }
    rikka_storage_rgba16f[push.target][texel] = float4(irradiance, 1.0);
}
