import rikka.reservoir;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint guide;
    uint samples;
    uint states;
    uint2 size;
    float reach;
    uint spatial;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint TAPS = 5u;
static const float RADIUS = 1.0 / 32.0;
static const float SMALLEST_RADIUS = 4.0;
static const float DEPTH_TOLERANCE = 0.1;
static const float NORMAL_LIMIT = 0.906;
static const float GOLDEN_ANGLE = 2.39996323;

struct Neighbour {
    float3 position;
    float3 normal;
    float count;
};

bool similar(RikkaGuide here, RikkaGuide there) {
    return there.covered && abs(there.depth - here.depth) < DEPTH_TOLERANCE * here.depth
        && dot(there.normal, here.normal) > NORMAL_LIMIT;
}

bool visible(RikkaFrameRef frame, RikkaReservoir r, float3 origin) {
    if (r.sky) {
        return rikka_ray_escapes(frame, origin, r.position, push.reach, false, RIKKA_ALL_CHANNELS) > 0.0;
    }
    float3 offset = r.position - origin;
    float distance = length(offset);
    float reach = distance * 0.999 - 1e-3;
    if (reach <= 0.0) {
        return true;
    }
    return rikka_ray_escapes(frame, origin, offset / distance, reach, false, RIKKA_ALL_CHANNELS) > 0.0;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    uint frame_index = frame.look.trace.frame;
    uint2 full = uint2(frame.view.viewport);
    int2 pixel = rikka_full_pixel(texel, 2u, full);
    RikkaGuide here = rikka_guide_load(push.guide, int2(texel));
    if (!here.covered) {
        rikka_storage_rgba16f[push.target][texel] = float4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    float3 position = rikka_world_of(frame, pixel);
    RikkaReservoir own = rikka_reservoir_load(push.samples, push.states, int2(texel));
    RikkaReservoir chosen = own;
    if (push.spatial != 0u) {
        RikkaResampling s = rikka_resampling_empty();
        float2 random = rikka_reservoir_random(uint2(pixel), frame_index, 4u);
        float own_target = rikka_reservoir_target(own, position, here.normal);
        rikka_resampling_add(s, own, own_target, own_target * own.weight * own.count, own.count, random.y, 0u);
        Neighbour neighbours[TAPS];
        uint taken = 0u;
        float radius = max(SMALLEST_RADIUS, float(push.size.y) * RADIUS);
        int2 highest = int2(push.size) - 1;
        [unroll]
        for (uint k = 0u; k < TAPS; ++k) {
            float2 choice = rikka_reservoir_random(uint2(pixel), frame_index, 5u + k);
            float angle = float(k) * GOLDEN_ANGLE + random.x * 2.0 * RIKKA_PI;
            float2 offset = float2(cos(angle), sin(angle)) * (radius * sqrt((float(k) + choice.x) / float(TAPS)));
            int2 tap = int2(texel) + int2(round(offset));
            if (any(tap < int2(0)) || any(tap > highest) || all(tap == int2(texel))) {
                continue;
            }
            RikkaGuide there = rikka_guide_load(push.guide, tap);
            if (!similar(here, there)) {
                continue;
            }
            RikkaReservoir q = rikka_reservoir_load(push.samples, push.states, tap);
            if (!(q.count > 0.0)) {
                continue;
            }
            float3 from = rikka_world_of(frame, rikka_full_pixel(uint2(tap), 2u, full));
            float jacobian = rikka_reservoir_jacobian(q, from, position);
            if (jacobian <= 0.0) {
                continue;
            }
            float target = rikka_reservoir_target(q, position, here.normal);
            rikka_resampling_add(s, q, target, target * q.weight * q.count * jacobian, q.count, choice.y, k + 1u);
            neighbours[taken].position = from;
            neighbours[taken].normal = there.normal;
            neighbours[taken].count = q.count;
            ++taken;
        }
        float support = own.count;
        for (uint i = 0u; i < taken; ++i) {
            if (rikka_reservoir_target(s.chosen, neighbours[i].position, neighbours[i].normal) > 0.0) {
                support += neighbours[i].count;
            }
        }
        chosen = s.chosen;
        chosen.weight = rikka_resampling_weight(s, support);
        if (s.index != 0u && s.index != RIKKA_NONE
            && !visible(frame, chosen, rikka_ray_lift(position, here.normal, here.depth))) {
            chosen.weight = 0.0;
        }
    }
    float3 irradiance = rikka_reservoir_irradiance(chosen, position, here.normal);
    rikka_storage_rgba16f[push.target][texel] = float4(irradiance, own.confidence);
}
