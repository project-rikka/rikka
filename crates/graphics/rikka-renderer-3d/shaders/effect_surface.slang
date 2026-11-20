import rikka.common;

struct Push {
    uint surface;
    uint normals;
    uint velocity;
    uint motion;
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
    uint4 texel = rikka_identity_images[NonUniformResourceIndex(push.surface)].Load(int3(source, 0));
    bool covered = any(texel != uint4(0u));
    float3 normal = covered ? rikka_decode_octahedral(texel.x) : float3(0.0);
    float2 velocity = push.motion != RIKKA_NONE
        ? rikka_fetch(rikka_images[NonUniformResourceIndex(push.motion)], pixel, 0).xy
        : unpackHalf2x16ToFloat(texel.y);
    float coverage = covered ? 1.0 : 0.0;
    rikka_storage_rgba16f[NonUniformResourceIndex(push.normals)][pixel] = float4(normal, coverage);
    rikka_storage_rgba16f[NonUniformResourceIndex(push.velocity)][pixel] = float4(velocity, 0.0, coverage);
}
