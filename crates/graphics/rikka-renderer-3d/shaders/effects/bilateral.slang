import rikka.effect;

struct Parameters {
    [Default(6.0)] [Range(0.0, 32.0)] [Label("radius (σ, px at 1080p)")] float radius;
    [Default(0.1)] [Range(0.005, 1.0)] [Label("colour tolerance")] float tolerance;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("depth guide")] float depth;
    [Default(1.0)] [Range(0.0, 4.0)] [Label("normal guide")] float normals;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

struct Push {
    Parameters* parameters;
    uint image;
    uint source;
    uint depth;
    uint normals;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int MOST_TAPS = 12;
static const float DEPTH_SCALE = 100.0;
static const float NORMAL_POWER = 8.0;

struct Guide {
    float3 color;
    float depth;
    float4 normal;
};

Guide guide_at(int2 pixel, float4 color) {
    Guide guide;
    guide.color = rikka_effect_straight(color);
    guide.depth = rikka_effect_linear_depth(push.frame, rikka_effect_load(push.depth, pixel).r);
    guide.normal = rikka_effect_load(push.normals, pixel);
    return guide;
}

float similarity(Parameters parameters, Guide center, Guide other) {
    float3 difference = other.color - center.color;
    float tolerance = max(parameters.tolerance, 1e-3);
    float weight = exp(-0.5 * dot(difference, difference) / (tolerance * tolerance));
    if (parameters.depth > 0.0) {
        float relative = abs(other.depth - center.depth) / max(min(other.depth, center.depth), 1e-4);
        float scaled = relative * parameters.depth * DEPTH_SCALE;
        weight *= exp(-0.5 * scaled * scaled);
    }
    if (parameters.normals > 0.0) {
        if (abs(other.normal.w - center.normal.w) > 0.5) {
            return 0.0;
        }
        float facing = max(dot(other.normal.xyz, center.normal.xyz), 0.0);
        weight *= pow(facing, parameters.normals * NORMAL_POWER);
    }
    return weight;
}

void smooth(uint3 id, int2 axis, bool last) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float sigma = max(parameters.radius, 0.0) * rikka_effect_scale(push.size);
    if (parameters.amount <= 0.0 || sigma < 0.3) {
        if (last) {
            rikka_effect_store(push.target, pixel, rikka_effect_load(push.image, pixel));
        }
        return;
    }
    float4 color = rikka_effect_load(push.source, pixel);
    Guide center = guide_at(pixel, color);
    int stride = max(1, int(floor(sigma / 3.0)));
    int reach = min(int(ceil(3.0 * sigma / float(stride))), MOST_TAPS);
    float4 sum = color;
    float total = 1.0;
    for (int tap = 1; tap <= reach; ++tap) {
        float distance = float(tap * stride);
        float spatial = exp(-0.5 * distance * distance / (sigma * sigma));
        for (int side = -1; side <= 1; side += 2) {
            int2 at = clamp(pixel + axis * tap * stride * side, int2(0), int2(push.size) - 1);
            float4 other = rikka_effect_load(push.source, at);
            float weight = spatial * similarity(parameters, center, guide_at(at, other));
            sum += other * weight;
            total += weight;
        }
    }
    float4 smoothed = sum / total;
    if (last) {
        smoothed = lerp(rikka_effect_load(push.image, pixel), smoothed, clamp(parameters.amount, 0.0, 1.0));
    }
    rikka_effect_store(push.target, pixel, smoothed);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void horizontal(uint3 id: SV_DispatchThreadID) {
    smooth(id, int2(1, 0), false);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void vertical(uint3 id: SV_DispatchThreadID) {
    smooth(id, int2(0, 1), true);
}
