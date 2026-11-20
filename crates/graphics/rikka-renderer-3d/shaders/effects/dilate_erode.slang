import rikka.effect;

struct Parameters {
    [Default(4.0)] [Range(-64.0, 64.0)] [Label("distance (px at 1080p, negative erodes)")] float distance;
    [Default(0.0)] [Range(0.0, 32.0)] [Label("feather (px at 1080p)")] float feather;
};

struct Push {
    Parameters* parameters;
    uint image;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float SQUARE_SHARE = 0.41421356;
static const float DIAMOND_SHARE = 0.29289322;
static const int LONGEST_REACH = 96;

bool neutral() {
    return abs(push.parameters.distance) <= 0.0 && push.parameters.feather <= 0.0;
}

float4 morph(int2 pixel, int2 axis, float share) {
    Parameters parameters = *push.parameters;
    float scale = rikka_effect_scale(push.size);
    float extent = abs(parameters.distance) * scale * share;
    float soft = max(max(parameters.feather, 0.0) * scale * share, 1.0);
    bool grow = parameters.distance >= 0.0;
    float4 center = rikka_effect_load(push.source, pixel);
    float4 result = center;
    int reach = min(int(ceil(extent + soft)), LONGEST_REACH);
    for (int t = 1; t <= reach; ++t) {
        float weight = clamp((extent + soft - float(t)) / soft, 0.0, 1.0);
        for (int side = -1; side <= 1; side += 2) {
            float4 found = rikka_effect_load_clamped(push.source, pixel + axis * t * side, push.size);
            float4 candidate = lerp(center, found, weight);
            result = grow ? max(result, candidate) : min(result, candidate);
        }
    }
    return result;
}

void run(uint3 id, int2 axis, float share, bool last) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    if (neutral()) {
        if (last) {
            rikka_effect_store(push.target, pixel, rikka_effect_load(push.image, pixel));
        }
        return;
    }
    rikka_effect_store(push.target, pixel, morph(pixel, axis, share));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void horizontal(uint3 id: SV_DispatchThreadID) {
    run(id, int2(1, 0), SQUARE_SHARE, false);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void vertical(uint3 id: SV_DispatchThreadID) {
    run(id, int2(0, 1), SQUARE_SHARE, false);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void diagonal(uint3 id: SV_DispatchThreadID) {
    run(id, int2(1, 1), DIAMOND_SHARE, false);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void antidiagonal(uint3 id: SV_DispatchThreadID) {
    run(id, int2(1, -1), DIAMOND_SHARE, true);
}
