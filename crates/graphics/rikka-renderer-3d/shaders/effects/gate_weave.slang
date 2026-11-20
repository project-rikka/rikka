import rikka.effects.gate_weave;

struct Push {
    Parameters* parameters;
    uint source;
    uint motion;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float4 catmull_rom(uint image, float2 position, float2 size) {
    float2 coordinate = position - 0.5;
    float2 base = floor(coordinate);
    float2 f = coordinate - base;
    float2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    float2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    float2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    float2 w3 = f * f * (-0.5 + 0.5 * f);
    float2 w12 = w1 + w2;
    float2 middle = (base + 0.5 + w2 / w12) / size;
    float2 low = (base - 0.5) / size;
    float2 high = (base + 2.5) / size;
    float4 sum = float4(0.0);
    sum += rikka_effect_sample(image, float2(low.x, low.y)) * w0.x * w0.y;
    sum += rikka_effect_sample(image, float2(middle.x, low.y)) * w12.x * w0.y;
    sum += rikka_effect_sample(image, float2(high.x, low.y)) * w3.x * w0.y;
    sum += rikka_effect_sample(image, float2(low.x, middle.y)) * w0.x * w12.y;
    sum += rikka_effect_sample(image, float2(middle.x, middle.y)) * w12.x * w12.y;
    sum += rikka_effect_sample(image, float2(high.x, middle.y)) * w3.x * w12.y;
    sum += rikka_effect_sample(image, float2(low.x, high.y)) * w0.x * w3.y;
    sum += rikka_effect_sample(image, float2(middle.x, high.y)) * w12.x * w3.y;
    sum += rikka_effect_sample(image, float2(high.x, high.y)) * w3.x * w3.y;
    return max(sum, float4(0.0));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void weave(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    if (rikka_weave_still(parameters)) {
        rikka_effect_store(push.target, pixel, rikka_effect_load(push.source, pixel));
        return;
    }
    float4 motion = rikka_effect_load(push.motion, int2(0, 0));
    float2 size = float2(push.size);
    float2 offset = motion.xy * rikka_effect_scale(push.size);
    float2 center = 0.5 * size;
    float2 position = center + rikka_effect_rotate(float2(pixel) + 0.5 - center - offset, -motion.z);
    float4 color = catmull_rom(push.source, position, size);
    color.rgb *= exp2(motion.w);
    rikka_effect_store(push.target, pixel, color);
}
