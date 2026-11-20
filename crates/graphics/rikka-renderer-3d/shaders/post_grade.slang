import rikka.post;

struct Grade {
    float4 balance_red;
    float4 balance_green;
    float4 balance_blue;
    float3 lift;
    float contrast;
    float3 gamma;
    float saturation;
    float3 gain;
    float vibrance;
    float3 shadow_tint;
    float black;
    float3 highlight_tint;
    float night;
    uint display;
    uint pad0;
    uint pad1;
    uint pad2;
};

typealias GradeRef = Grade*;

struct Push {
    GradeRef params;
    uint source;
    uint exposure;
    float exposure_value;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DISPLAY_LIMIT = 0.9999;

float3 night_vision(GradeRef p, float4 color, float exposure) {
    if (p.night <= 0.0 || color.a <= 1e-4) {
        return color.rgb;
    }
    float adapted = 0.18 / max(exposure, 1e-4);
    float dark = 1.0 - smoothstep(0.012, 0.07, adapted);
    float luma = rikka_luma(max(color.rgb, float3(0.0))) / color.a;
    float rods = p.night * dark * (1.0 - smoothstep(0.09, 1.08, luma));
    float3 scotopic = float3(rikka_luma(max(color.rgb, float3(0.0)))) * float3(0.6, 0.82, 1.3);
    return lerp(color.rgb, scotopic, rods * 0.6);
}

float3 graded_scene(GradeRef p, float3 color) {
    float3x3 balance = float3x3(p.balance_red.xyz, p.balance_green.xyz, p.balance_blue.xyz);
    color = max(mul(balance, color), float3(0.0));
    color = 0.18 * pow(color / 0.18, float3(p.contrast));
    float luma = rikka_luma(color);
    color = luma + (color - luma) * p.saturation;
    float highest = max(color.r, max(color.g, color.b));
    float lowest = min(color.r, min(color.g, color.b));
    float chroma = highest > 1e-5 ? (highest - lowest) / highest : 0.0;
    color = luma + (color - luma) * (1.0 + p.vibrance * (1.0 - chroma));
    return max(color, float3(0.0));
}

float3 graded_display(GradeRef p, float3 color) {
    float3 compressed = color / (1.0 + color);
    compressed = p.gain * (compressed + p.lift * (1.0 - compressed));
    compressed = pow(max(compressed, float3(0.0)), 1.0 / max(p.gamma, float3(1e-3)));
    float luma = clamp(rikka_luma(compressed), 0.0, 1.0);
    float shadow = (1.0 - luma) * (1.0 - luma);
    compressed *= lerp(float3(1.0), p.shadow_tint, shadow) * lerp(float3(1.0), p.highlight_tint, luma * luma);
    compressed = clamp(p.black + (1.0 - p.black) * compressed, 0.0, DISPLAY_LIMIT);
    return compressed / (1.0 - compressed);
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    GradeRef p = push.params;
    int2 pixel = int2(frag_coord.xy);
    float4 color = post_fetch(push.source, pixel);
    float exposure = post_exposure_or(push.exposure, push.exposure_value);
    color.rgb = night_vision(p, color, exposure);
    float alpha = clamp(color.a, 0.0, 1.0);
    float3 straight = alpha > 1e-4 ? color.rgb / alpha : float3(0.0);
    float3 graded = graded_scene(p, straight);
    if (p.display != 0u) {
        graded = graded_display(p, graded);
    }
    return float4(graded * alpha, alpha);
}
