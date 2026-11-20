import rikka.effect;

struct Parameters {
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
    [Default(16.0)] [Range(2.0, 128.0)] [Label("block (px at 1080p)")] float block;
    [Default(0.03)] [Range(0.0, 1.0)] [Label("refresh (chance per block and frame)")] float refresh;
    [Default(1.0)] [Range(0.0, 8.0)] float motion;
    [Default(60.0)] [Range(1.0, 120.0)] [Label("frames per second")] float rate;
    [Default(11)] uint seed;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint history;
    uint velocity;
    uint target;
    uint2 size;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void mosh(uint3 id: SV_DispatchThreadID) {
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
    float side = max(parameters.block * rikka_effect_scale(push.size), 1.0);
    int2 cell = int2(floor((float2(pixel) + 0.5) / side));
    uint frame = uint(floor(max(push.time, 0.0) * max(parameters.rate, 1.0)));
    float chance = rikka_effect_random(uint3(uint(cell.x) ^ (frame * 0x9e3779b9u), uint(cell.y), parameters.seed));
    if (chance < parameters.refresh) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float2 center = (float2(cell) + 0.5) * side;
    int2 probe = clamp(int2(center), int2(0), int2(push.size) - 1);
    float2 velocity = rikka_effect_load(push.velocity, probe).xy;
    float2 uv = rikka_effect_uv(pixel, push.size) - velocity * parameters.motion;
    float4 dragged = rikka_effect_sample(push.history, clamp(uv, 0.0, 1.0));
    rikka_effect_store(push.target, pixel, lerp(color, dragged, clamp(parameters.amount, 0.0, 1.0)));
}
