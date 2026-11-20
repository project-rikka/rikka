struct RikkaFluidSplat {
    [[vk::location(0)]] float3 origin;
    [[vk::location(1)]] float3 direction;
    [[vk::location(2)]] float3 span;
    [[vk::location(3)]] nointerpolation float4 clip;
    [[vk::location(4)]] nointerpolation float4 front;
    [[vk::location(5)]] nointerpolation float4 absorption;
    [[vk::location(6)]] nointerpolation float4 scattering;
    [[vk::location(7)]] nointerpolation float volume;
};
