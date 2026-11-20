import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint STEPS = 32u;
static const uint REFINES = 5u;
static const float REACH = 60.0;
static const float BRIGHTEST = 64.0;

float3 around(float3 normal, float3 local) {
    float3 helper = abs(normal.z) < 0.999 ? float3(0.0, 0.0, 1.0) : float3(1.0, 0.0, 0.0);
    float3 tangent = normalize(cross(helper, normal));
    float3 bitangent = cross(normal, tangent);
    return tangent * local.x + bitangent * local.y + normal * local.z;
}

float3 glossy_direction(float3 eye, float3 normal, float roughness, float2 noise) {
    float alpha = max(roughness * roughness, 1e-3);
    float phi = 2.0 * RIKKA_PI * noise.x;
    float cos_theta = sqrt((1.0 - noise.y) / (1.0 + (alpha * alpha - 1.0) * noise.y));
    float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    float3 half_vector = around(normal, float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta));
    float3 reflected = reflect(-eye, half_vector);
    return dot(reflected, normal) > 0.0 ? reflected : reflect(-eye, normal);
}

float clip_to_screen(float2 from, float2 to) {
    float2 delta = to - from;
    float most = 1.0;
    for (int axis = 0; axis < 2; ++axis) {
        if (to[axis] < 0.0) {
            most = min(most, -from[axis] / delta[axis]);
        } else if (to[axis] > 1.0) {
            most = min(most, (1.0 - from[axis]) / delta[axis]);
        }
    }
    return max(most, 0.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaScene scene = frame.look.scene;
    RikkaSpace space = frame.look.space;
    SpaceView v = space_view(frame);
    int2 pixel = min(int2(texel) * 2, int2(v.size) - 1);
    float depth = rikka_read_depth(scene.depth, pixel);
    RikkaSurfaceSample surface = rikka_read_surface(scene.surface, pixel);
    if (depth <= 0.0 || surface.reflection <= 0.0 || surface.roughness > space.roughness_limit) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    float2 uv = space_uv(pixel, v.size);
    float3 origin = space_position(v, uv, depth);
    float3 normal = normalize(mul((float3x3)v.view, surface.normal));
    float3 eye = space_toward_eye(v, origin);
    float2 noise = float2(rikka_interleaved_gradient(float2(pixel)),
                          rikka_interleaved_gradient(float2(pixel.yx) + float2(41.0, 17.0)));
    float3 ray = glossy_direction(eye, normal, surface.roughness, noise);

    float reach = REACH;
    float near = frame.view.near;
    if (ray.z > 1e-5) {
        reach = min(reach, (-near * 1.01 - origin.z) / ray.z);
    }
    if (reach <= 0.0) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    float3 start = origin + normal * (0.002 * -origin.z);
    float3 from = space_project(v, start);
    float3 to = space_project(v, start + ray * reach);
    float extent = clip_to_screen(from.xy, to.xy);
    to = lerp(from, to, extent);
    float2 travel = (to.xy - from.xy) * v.size;
    if (dot(travel, travel) < 1.0) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }

    float jitter = rikka_interleaved_gradient(float2(pixel) + float2(13.0, 71.0));
    float before = 0.0;
    float found = -1.0;
    for (uint step = 1u; step <= STEPS; ++step) {
        float s = (float(step) - 1.0 + jitter) / float(STEPS);
        s = s * s * 0.5 + s * 0.5;
        float3 at = lerp(from, to, s);
        float scene_depth = rikka_read_depth(scene.depth, space_pixel(at.xy, v.size));
        if (scene_depth > at.z) {
            float scene_distance = space_distance(v, scene_depth);
            float behind = space_distance(v, at.z) - scene_distance;
            if (behind < 0.15 + 0.04 * scene_distance) {
                found = s;
                break;
            }
        }
        before = s;
    }
    if (found < 0.0) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    float low = before;
    float high = found;
    for (uint refine = 0u; refine < REFINES; ++refine) {
        float middle = 0.5 * (low + high);
        float3 at = lerp(from, to, middle);
        float scene_depth = rikka_read_depth(scene.depth, space_pixel(at.xy, v.size));
        if (scene_depth > at.z) {
            high = middle;
        } else {
            low = middle;
        }
    }
    float2 hit = lerp(from.xy, to.xy, high);
    int2 hit_pixel = space_pixel(hit, v.size);
    float3 hit_normal = normalize(mul((float3x3)v.view, rikka_read_surface(scene.surface, hit_pixel).normal));
    if (dot(hit_normal, ray) > 0.2) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0);
        return;
    }
    float2 edge = abs(hit * 2.0 - 1.0);
    float confidence = 1.0 - smoothstep(0.85, 1.0, max(edge.x, edge.y));
    confidence *= 1.0 - smoothstep(0.6, 1.0, high * extent);
    float3 color = rikka_images[push.source]
        .SampleLevel(rikka_samplers[1], rikka_render_uv(frame.view, hit, 1u), 0.0).rgb;
    color = min(color, float3(BRIGHTEST));
    if (any(isnan(color))) {
        color = float3(0.0);
        confidence = 0.0;
    }
    rikka_storage_rgba16f[push.target][texel] = float4(color, confidence);
}
