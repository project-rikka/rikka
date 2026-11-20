import rikka.effects.color_lookup;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void grade(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    if (parameters.table == RIKKA_NONE || parameters.amount <= 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    uint2 extent = rikka_effect_image_size(parameters.table);
    float3 straight = rikka_effect_straight(color);
    float3 shaped = rikka_lookup_shape(parameters.shaper, straight);
    float3 unit = (shaped - parameters.domain.min)
        / max(parameters.domain.max - parameters.domain.min, float3(1e-6));
    float3 table = extent.y == 1u
        ? rikka_lookup_strip(parameters.table, unit, int(extent.x))
        : rikka_lookup_tetrahedral(parameters.table, unit, int(extent.y));
    float3 looked = parameters.output == RIKKA_LOOKUP_DISPLAY
        ? table
        : rikka_lookup_unshape(parameters.shaper, table);
    float3 graded = lerp(straight, looked, clamp(parameters.amount, 0.0, 1.0));
    rikka_effect_store(push.target, pixel, float4(graded * color.a, color.a));
}
