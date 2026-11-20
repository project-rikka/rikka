import rikka.effect;

struct Parameters {
    [Options("sobel|prewitt|kirsch|laplacian")] [Default(0)] uint kernel;
    [Options("luminance|colour|depth|normals")] [Default(0)] uint signal;
    [Default(1.0)] [Range(0.5, 8.0)] [Label("spacing (px at 1080p)")] float spacing;
    [Default(1.0)] [Range(0.0, 16.0)] float gain;
    [Default(0.0)] [Range(0.0, 1.0)] float threshold;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint depth;
    uint normals;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint PREWITT = 1u;
static const uint KIRSCH = 2u;
static const uint LAPLACIAN = 3u;
static const uint COLOUR = 1u;
static const uint DEPTH = 2u;
static const uint NORMALS = 3u;

float3 signal_at(uint signal, float2 position) {
    float2 uv = position / float2(push.size);
    if (signal == DEPTH) {
        int2 pixel = clamp(int2(floor(position)), int2(0), int2(push.size) - 1);
        float away = rikka_effect_linear_depth(push.frame, rikka_effect_load(push.depth, pixel).r);
        return float3(log2(max(away, 1e-4)));
    }
    if (signal == NORMALS) {
        return rikka_effect_sample(push.normals, uv).xyz * 0.5;
    }
    float3 color = max(rikka_effect_straight(rikka_effect_sample(push.source, uv)), float3(0.0));
    return signal == COLOUR ? color : float3(rikka_luma(color));
}

float3 gradient(uint kernel, float3 s[9]) {
    if (kernel == LAPLACIAN) {
        float3 ring = s[0] + s[1] + s[2] + s[3] + s[5] + s[6] + s[7] + s[8];
        return abs(8.0 * s[4] - ring) / 3.0;
    }
    if (kernel == KIRSCH) {
        const int RING[8] = { 0, 1, 2, 5, 8, 7, 6, 3 };
        float3 strongest = float3(0.0);
        for (int turn = 0; turn < 8; ++turn) {
            float3 response = float3(0.0);
            for (int at = 0; at < 8; ++at) {
                int offset = (at - turn + 8) % 8;
                response += s[RING[at]] * (offset < 3 ? 5.0 : -3.0);
            }
            strongest = max(strongest, response);
        }
        return strongest / 15.0;
    }
    float side = kernel == PREWITT ? 1.0 : 2.0;
    float3 x = (s[2] + side * s[5] + s[8]) - (s[0] + side * s[3] + s[6]);
    float3 y = (s[6] + side * s[7] + s[8]) - (s[0] + side * s[1] + s[2]);
    return sqrt(x * x + y * y) / (2.0 + side);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void detect(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    if (parameters.amount <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float spacing = max(parameters.spacing * rikka_effect_scale(push.size), 1.0);
    float2 center = float2(pixel) + 0.5;
    float3 window[9];
    for (int y = 0; y < 3; ++y) {
        for (int x = 0; x < 3; ++x) {
            window[y * 3 + x] = signal_at(parameters.signal, center + float2(x - 1, y - 1) * spacing);
        }
    }
    float3 strength = max(gradient(parameters.kernel, window) * parameters.gain - parameters.threshold, float3(0.0));
    if (parameters.signal != COLOUR) {
        strength = float3(max(strength.x, max(strength.y, strength.z)));
    }
    float4 edges = float4(strength, 1.0);
    rikka_effect_store(push.target, pixel, lerp(color, edges, clamp(parameters.amount, 0.0, 1.0)));
}
