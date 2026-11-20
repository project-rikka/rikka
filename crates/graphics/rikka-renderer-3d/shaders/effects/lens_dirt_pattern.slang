import rikka.effects.lens_dirt;

struct Push {
    Parameters* parameters;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void pattern(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float2 uv = rikka_effect_uv(pixel, push.size);
    float3 dirt = rikka_dirt(*push.parameters, uv, rikka_effect_aspect(push.size));
    rikka_effect_store(push.target, pixel, float4(dirt, 1.0));
}
