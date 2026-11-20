import rikka.post;

struct Push {
    uint source;
    uint destination;
    uint velocity;
    uint neighbors;
    uint2 size;
    uint tile;
    uint seed;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int SAMPLES = 16;

float cone(float distance, float span) {
    return clamp(1.0 - distance / max(span, 1e-3), 0.0, 1.0);
}

float cylinder(float distance, float span) {
    return 1.0 - smoothstep(0.95 * span, 1.05 * span + 1e-3, distance);
}

float in_front(float near, float far) {
    return clamp(1.0 - (near - far) / max(0.02 * min(near, far), 0.01), 0.0, 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    int2 size = int2(push.size);
    if (any(pixel >= size)) {
        return;
    }
    float4 center = post_fetch(push.source, pixel);
    float2 widest = post_fetch(push.neighbors, pixel / int(push.tile)).xy;
    float widest_span = length(widest);
    if (widest_span < 0.5) {
        post_store(push.destination, pixel, center);
        return;
    }
    float4 own = post_fetch(push.velocity, pixel);
    float2 own_velocity = own.xy;
    float own_span = max(length(own_velocity), 0.5);
    float own_depth = own.z;
    float2 own_direction = own_span > 0.75 ? own_velocity : widest;

    float jitter = post_random(uint3(uint2(pixel), push.seed)) - 0.5;
    float total_weight = 1.0 / own_span;
    float4 total = center * total_weight;
    for (int i = 0; i < SAMPLES; ++i) {
        float t = lerp(-1.0, 1.0, (float(i) + jitter + 1.0) / float(SAMPLES + 1));
        float2 direction = (i & 1) == 0 ? widest : own_direction;
        int2 at = clamp(int2(round(float2(pixel) + 0.5 + direction * t - 0.5)), int2(0), size - 1);
        float4 other = post_fetch(push.velocity, at);
        float other_span = max(length(other.xy), 0.5);
        float distance = length(float2(at - pixel));
        float front = in_front(other.z, own_depth);
        float back = in_front(own_depth, other.z);
        float weight = front * cone(distance, other_span)
            + back * cone(distance, own_span)
            + 2.0 * cylinder(distance, other_span) * cylinder(distance, own_span);
        total += post_fetch(push.source, at) * weight;
        total_weight += weight;
    }
    post_store(push.destination, pixel, total / total_weight);
}
