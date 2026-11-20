__exported import rikka.common;
__exported import rikka.editor;

struct RikkaVaryings {
    [[vk::location(0)]] float3 world_position;
    [[vk::location(1)]] float3 normal;
    [[vk::location(2)]] float4 tangent;
    [[vk::location(3)]] float2 uv;
    [[vk::location(4)]] float4 color;
    [[vk::location(5)]] nointerpolation uint instance;
    [[vk::location(6)]] float4 wet;
};

struct RikkaHullVaryings {
    [[vk::location(0)]] float3 world_position;
    [[vk::location(1)]] float3 normal;
    [[vk::location(2)]] float4 tangent;
    [[vk::location(3)]] float2 uv;
    [[vk::location(4)]] float4 color;
    [[vk::location(5)]] nointerpolation uint instance;
    [[vk::location(6)]] float3 surface_position;

    RikkaVaryings shaded() {
        RikkaVaryings v;
        v.world_position = world_position;
        v.normal = normal;
        v.tangent = tangent;
        v.uv = uv;
        v.color = color;
        v.instance = instance;
        v.wet = float4(0.0);
        return v;
    }
};

struct RikkaDepthVaryings {
    [[vk::location(0)]] float2 uv;
    [[vk::location(1)]] nointerpolation uint instance;
    [[vk::location(2)]] float3 world_position;
};

struct RikkaSurfaceVaryings {
    [[vk::location(0)]] float3 normal;
    [[vk::location(1)]] float2 uv;
    [[vk::location(2)]] float4 clip;
    [[vk::location(3)]] float4 previous_clip;
    [[vk::location(4)]] nointerpolation uint instance;
    [[vk::location(5)]] float wet;
};
