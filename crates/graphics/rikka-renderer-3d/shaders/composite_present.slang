import rikka.post;
import rikka.outline;

struct Push {
    uint source;
    uint seed;
    uint outline;
    uint reach;
    int4 area;
    float4 outline_color;
    float2 ratio;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    float4 color = post_fetch(push.source, pixel);
    float alpha = clamp(color.a, 0.0, 1.0);
    float3 straight = alpha > 1e-4 ? color.rgb / alpha : float3(0.0);
    float dither = post_random(uint3(uint2(pixel), push.seed * 7u + 1u))
        + post_random(uint3(uint2(pixel), push.seed * 7u + 2u)) - 1.0;
    float3 dithered = clamp(straight + dither / 255.0, 0.0, 1.0);
    float4 result = float4(dithered * alpha, alpha);
    if (rikka_outlined(push.outline, push.reach, push.area, rikka_outline_pixel(pixel, push.ratio))) {
        float coverage = push.outline_color.a;
        result = float4(rikka_encode_srgb(push.outline_color.rgb) * coverage, coverage)
            + result * (1.0 - coverage);
    }
    return result;
}
