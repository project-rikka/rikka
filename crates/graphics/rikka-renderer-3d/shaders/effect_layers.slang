import rikka.common;

struct Push {
    uint layers;
    uint pad;
    float2 ratio;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
uint fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 source = int2(frag_coord.xy * push.ratio);
    return rikka_identity_images[NonUniformResourceIndex(push.layers)].Load(int3(source, 0)).r;
}
