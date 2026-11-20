struct Push {
    float4 row0;
    float4 row1;
    float4 row2;
    float2 uv_scale;
    uint chroma_packed;

    uint chroma_swapped;
    uint4 planes;
    float2 chroma_offset;
}

[[vk::binding(0, 0)]] Sampler2D textures[];

[[vk::push_constant]] ConstantBuffer<Push> pc;

float2 extent_of(uint plane) {
    uint width;
    uint height;
    textures[plane].GetDimensions(width, height);
    return float2(int2(int(width), int(height)));
}

float4 sample_plane(uint plane, float2 uv) {
    float2 texels = extent_of(plane);
    float2 inside = min(uv, pc.uv_scale - 0.5 / texels);
    return textures[plane].Sample(inside);
}

float2 chroma_uv(float2 uv) {
    float2 luma = extent_of(pc.planes[0]);
    float2 chroma = extent_of(pc.planes[1]);
    float2 subsampled = float2(chroma.x < luma.x ? 1.0 : 0.0, chroma.y < luma.y ? 1.0 : 0.0);
    return uv + pc.chroma_offset * subsampled / chroma;
}

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    float2 uv = v_uv * pc.uv_scale;
    float y = sample_plane(pc.planes[0], uv).r;
    float2 at = chroma_uv(uv);
    float2 c = pc.chroma_packed != 0u
        ? sample_plane(pc.planes[1], at).rg
        : float2(sample_plane(pc.planes[1], at).r, sample_plane(pc.planes[2], at).r);
    if (pc.chroma_swapped != 0u) {
        c = c.yx;
    }
    float3 yuv = float3(y, c);
    float3 rgb = float3(
        dot(pc.row0.xyz, yuv) + pc.row0[3],
        dot(pc.row1.xyz, yuv) + pc.row1[3],
        dot(pc.row2.xyz, yuv) + pc.row2[3]);
    return float4(clamp(rgb, 0.0, 1.0), 1.0);
}
