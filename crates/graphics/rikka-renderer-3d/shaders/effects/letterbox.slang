import rikka.effect;

struct Parameters {
    [Default(2.39)] [Range(0.5, 4.0)] float aspect;
    [Color] float3 color;
    [Default(1.0)] [Range(0.0, 1.0)] float opacity;
    [Default(0.0)] [Range(0.0, 32.0)] [Label("softness (px at 1080p)")] float softness;
    [Default(0.0)] [Range(-1.0, 1.0)] float offset;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void matte(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float2 size = float2(push.size);
    float2 at = float2(pixel) + 0.5;
    float frame = size.x / size.y;
    float wanted = max(parameters.aspect, 0.01);
    float soft = max(parameters.softness * rikka_effect_scale(push.size), 0.5);
    float covered;
    if (wanted >= frame) {
        float open = size.x / wanted;
        float top = 0.5 * (size.y - open) * (1.0 + parameters.offset);
        float bottom = top + open;
        covered = 1.0 - smoothstep(top - soft, top + soft, at.y) * (1.0 - smoothstep(bottom - soft, bottom + soft, at.y));
    } else {
        float open = size.y * wanted;
        float left = 0.5 * (size.x - open) * (1.0 + parameters.offset);
        float right = left + open;
        covered = 1.0 - smoothstep(left - soft, left + soft, at.x) * (1.0 - smoothstep(right - soft, right + soft, at.x));
    }
    float matte = covered * clamp(parameters.opacity, 0.0, 1.0);
    float4 bar = float4(rikka_effect_encode(parameters.color), 1.0);
    rikka_effect_store(push.target, pixel, lerp(color, bar, matte));
}
