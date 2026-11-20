import rikka.effects.kuwahara;

struct Push {
    Parameters* parameters;
    uint source;
    uint tensor;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void filter(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    rikka_effect_store(push.target, pixel, rikka_kuwahara(*push.parameters, push.source, push.tensor, pixel, push.size));
}
