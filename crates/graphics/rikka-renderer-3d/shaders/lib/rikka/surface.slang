__exported import rikka.common;

struct RikkaSurfaceSample {
    float3 normal;
    float2 velocity;
    uint figure;
    float roughness;
    float reflection;
    bool covered;
};

RikkaSurfaceSample rikka_read_surface(uint image, int2 pixel) {
    uint4 texel = rikka_identity_images[NonUniformResourceIndex(image)].Load(int3(pixel, 0));
    RikkaSurfaceSample s;
    s.covered = any(texel != uint4(0u));
    s.normal = s.covered ? rikka_decode_octahedral(texel.x) : float3(0.0, 1.0, 0.0);
    s.velocity = unpackHalf2x16ToFloat(texel.y);
    s.figure = texel.z;
    float4 gloss = unpackUnorm4x8ToFloat(texel.w);
    s.roughness = gloss.x;
    s.reflection = gloss.y;
    return s;
}

float rikka_read_depth(uint image, int2 pixel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], pixel, 0).r;
}

float3 rikka_world_at(RikkaViewRef view, float2 uv, float depth) {
    float4 world = mul(view.inverse_view_projection, float4(uv * 2.0 - 1.0, depth, 1.0));
    return world.xyz / world.w;
}

float rikka_linear_depth(RikkaViewRef view, float depth) {
    return view.near / max(depth, 1e-7);
}
