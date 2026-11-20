import rikka.reservoir;

struct Push {
    RikkaFrameRef frame;
    uint samples;
    float reach;
    uint2 size;
    uint radiance;
    uint guide_history;
    uint sample_target;
    uint state_target;
    uint sample_history;
    uint state_history;
    uint temporal;
    uint _pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DEPTH_TOLERANCE = 0.1;
static const float NORMAL_LIMIT = 0.8;
static const float HISTORY_CAP = 20.0;
static const uint OLDEST = 32u;
static const uint VALIDATION_PERIOD = 4u;
static const float MOVED_TOLERANCE = 0.05;
static const float CHANGE_LOW = 0.1;
static const float CHANGE_HIGH = 0.5;

bool reprojected(RikkaFrameRef frame, int2 pixel, float3 position, RikkaGuide here, out RikkaReservoir history) {
    history = rikka_reservoir_empty();
    if (push.temporal == 0u || frame.view.cut != 0u) {
        return false;
    }
    RikkaViewRef view = frame.view;
    float2 velocity = rikka_read_surface(frame.look.scene.surface, pixel).velocity;
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float2 previous = rikka_history_uv(view, uv, velocity);
    float previous_depth = mul(frame.look.scene.previous_view_projection, float4(position, 1.0)).w;
    float2 before = rikka_previous_size(view, 2u);
    int2 last = int2(before) - 1;
    int2 nearest = int2(floor(previous * before));
    float2 jitter = rikka_reservoir_random(uint2(pixel), frame.look.trace.frame, 1u);
    [unroll]
    for (int attempt = 0; attempt < 2; ++attempt) {
        int2 tap = attempt == 0 ? nearest : nearest + int2(floor(jitter * 3.0)) - 1;
        if (any(tap < int2(0)) || any(tap > last)) {
            continue;
        }
        RikkaGuide guide = rikka_guide_load(push.guide_history, tap);
        if (!guide.covered || abs(guide.depth - previous_depth) >= DEPTH_TOLERANCE * previous_depth
            || dot(guide.normal, here.normal) <= NORMAL_LIMIT) {
            continue;
        }
        history = rikka_reservoir_load(push.sample_history, push.state_history, tap);
        return history.count > 0.0 && history.age < OLDEST;
    }
    return false;
}

float adopted(inout RikkaReservoir history, float3 radiance) {
    float3 relative = abs(radiance - history.radiance) / max(radiance + history.radiance, float3(1e-4));
    history.radiance = radiance;
    return 1.0 - smoothstep(CHANGE_LOW, CHANGE_HIGH, length(relative) * rsqrt(3.0));
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
    int2 pixel = rikka_full_pixel(texel, 2u, uint2(frame.view.viewport));
    RikkaGuide guide = rikka_guide(frame, pixel);
    if (!guide.covered) {
        rikka_reservoir_store(push.sample_target, push.state_target, texel, rikka_reservoir_empty());
        return;
    }
    RikkaBounce bounce;
    bounce.radiance = push.radiance;
    bounce.guide_history = push.guide_history;
    bounce.reach = push.reach;
    float3 position = rikka_world_of(frame, pixel);
    float3 origin = rikka_ray_lift(position, guide.normal, guide.depth);
    uint samples = max(push.samples, 1u);

    RikkaReservoir history;
    bool found = reprojected(frame, pixel, position, guide, history);
    float confidence = 1.0;
    bool validated = false;
    bool holding = found && history.weight > 0.0;
    if (holding && frame_index % VALIDATION_PERIOD == VALIDATION_PERIOD - 1u) {
        float pick = rikka_ray_noise(uint2(pixel), frame_index, 6u).x;
        float2 onward = rikka_ray_noise(uint2(pixel), frame_index, 10u);
        RikkaBounceSample again = rikka_bounce_trace(frame, bounce, origin,
            rikka_reservoir_direction(history, origin), pick, onward);
        float expected = length(history.position - origin);
        bool moved = again.sky != history.sky
            || (!again.sky && abs(again.distance - expected) > MOVED_TOLERANCE * expected);
        if (moved) {
            found = false;
            confidence = 0.0;
        } else {
            history.position = again.position;
            history.normal = again.normal;
            confidence = adopted(history, rikka_bounce_bounded(again.radiance));
            validated = true;
        }
    } else if (holding && !history.sky) {
        bool seen;
        float3 radiance = rikka_bounce_reused(frame, bounce, history.position, history.normal, seen);
        if (seen) {
            confidence = adopted(history, rikka_bounce_bounded(radiance));
        }
    }

    RikkaResampling fresh = rikka_resampling_empty();
    if (!validated) {
        for (uint s = 0u; s < samples; ++s) {
            uint index = frame_index * samples + s;
            float2 noise = rikka_ray_noise(uint2(pixel), index, 5u);
            float pick = rikka_ray_noise(uint2(pixel), index, 6u).x;
            float choice = rikka_reservoir_random(uint2(pixel), index, 2u).x;
            float2 onward = rikka_ray_noise(uint2(pixel), index, 10u);
            float3 direction = rikka_ray_cosine(guide.normal, noise);
            RikkaReservoir candidate = rikka_reservoir_of(rikka_bounce_trace(frame, bounce, origin, direction,
                pick, onward));
            float facing = dot(guide.normal, direction);
            float target = rikka_luma(candidate.radiance) * max(facing, 0.0);
            float weight = facing > 0.0 ? RIKKA_PI * rikka_luma(candidate.radiance) : 0.0;
            rikka_resampling_add(fresh, candidate, target, weight, 1.0, choice, s);
        }
    }

    RikkaReservoir result;
    if (found) {
        history.count = min(history.count, HISTORY_CAP * float(samples));
        float2 random = rikka_reservoir_random(uint2(pixel), frame_index, 3u);
        RikkaResampling merged = rikka_resampling_empty();
        RikkaReservoir drawn = fresh.chosen;
        drawn.weight = rikka_resampling_weight(fresh, fresh.count);
        rikka_resampling_add(merged, drawn, fresh.target, fresh.sum, fresh.count, random.x, 0u);
        float target = rikka_reservoir_target(history, position, guide.normal);
        rikka_resampling_add(merged, history, target, target * history.weight * history.count, history.count,
            random.y, 1u);
        result = merged.chosen;
        result.weight = rikka_resampling_weight(merged, merged.count);
        result.count = merged.count;
        result.age = merged.index == 1u ? history.age + 1u : 0u;
    } else {
        result = fresh.chosen;
        result.weight = rikka_resampling_weight(fresh, fresh.count);
        result.count = fresh.count;
        result.age = 0u;
    }
    result.confidence = confidence;
    rikka_reservoir_store(push.sample_target, push.state_target, texel, result);
}
