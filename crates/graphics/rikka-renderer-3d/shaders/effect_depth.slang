import rikka.common;

struct Push {
    uint depth;
    uint target;
    uint2 size;
    float2 ratio;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (any(uint2(pixel) >= push.size)) {
        return;
    }
    int2 source = int2((float2(pixel) + 0.5) * push.ratio);
    float depth = rikka_fetch(rikka_images[NonUniformResourceIndex(push.depth)], source, 0).r;
    rikka_storage_r32f[NonUniformResourceIndex(push.target)][pixel] = depth;
}
