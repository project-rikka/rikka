struct Varyings {
    float4 position : SV_Position;
    [[vk::location(0)]] float2 uv : TEXCOORD0;
}

[shader("vertex")]
Varyings vertex(int vertex_index : SV_VulkanVertexID) {
    float2 uv = float2((vertex_index << 1) & 2, vertex_index & 2);
    Varyings out;
    out.uv = uv;
    out.position = float4(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}
