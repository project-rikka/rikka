import rikka.effects.bokeh_blur;

struct Push {
    Parameters* parameters;
    uint source;
    uint size_map;
    uint blurred;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void combine(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    int2 pixel = int2(id.xy);
    float4 color = rikka_effect_load(push.source, pixel);
    float2 uv = rikka_effect_uv(pixel, push.size);
    float radius = rikka_bokeh_radius(push.parameters, push.size_map, uv, push.size);
    if (radius < 0.5) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float4 blurred = rikka_effect_sample(push.blurred, uv);
    rikka_effect_store(push.target, pixel, lerp(color, blurred, smoothstep(0.5, 2.0, radius)));
}
