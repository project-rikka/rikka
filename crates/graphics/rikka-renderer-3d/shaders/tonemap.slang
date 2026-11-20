import rikka.outline;

static const uint RIKKA_TONE_ACES = 0u;

struct Push {
    uint source;
    float exposure;
    uint curve;
    uint outline;
    uint reach;
    float4 outline_color;
    int4 area;
    float2 ratio;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float3 aces(float3 x) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    float4 texel = rikka_fetch(rikka_images[push.source], pixel, 0);
    if (rikka_outlined(push.outline, push.reach, push.area, rikka_outline_pixel(pixel, push.ratio))) {
        float coverage = push.outline_color.a;
        texel = float4(push.outline_color.rgb * coverage, coverage) + texel * (1.0 - coverage);
    }

    float3 straight = texel.a > 0.0 ? texel.rgb / texel.a : float3(0.0);
    float3 exposed = straight * push.exposure;
    float3 mapped = push.curve == RIKKA_TONE_ACES ? aces(exposed) : clamp(exposed, 0.0, 1.0);
    float3 color = rikka_encode_srgb(mapped);

    return float4(color * texel.a, texel.a);
}
