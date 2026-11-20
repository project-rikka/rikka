import rikka.common;

struct Push {
    uint source;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(push.source)], int2(frag_coord.xy), 0);
}
