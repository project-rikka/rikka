import rikka.post;

struct Push {
    uint source;
    uint seed;
    float2 size;
    float chromatic;
    float vignette;
    float grain;
    float sharpen;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float perceived(float4 color) {
    return clamp(rikka_luma(max(color.rgb, float3(0.0))), 0.0, 1.0);
}

float4 sharpened(int2 pixel, float4 center) {
    int2 size = int2(push.size);
    float4 up = post_fetch_clamped(push.source, pixel + int2(0, -1), size);
    float4 down = post_fetch_clamped(push.source, pixel + int2(0, 1), size);
    float4 left = post_fetch_clamped(push.source, pixel + int2(-1, 0), size);
    float4 right = post_fetch_clamped(push.source, pixel + int2(1, 0), size);
    float c = perceived(center);
    float u = perceived(up);
    float d = perceived(down);
    float l = perceived(left);
    float r = perceived(right);
    float lowest = min(c, min(min(u, d), min(l, r)));
    float highest = max(c, max(max(u, d), max(l, r)));
    float amplitude = sqrt(clamp(min(lowest, 1.0 - highest) / max(highest, 1e-4), 0.0, 1.0));
    float weight = -amplitude * 0.2 * push.sharpen;
    float4 result = (center + weight * (up + down + left + right)) / (1.0 + 4.0 * weight);
    return max(result, float4(0.0));
}

float natural_vignette(float2 uv, float aspect) {
    float2 centered = (uv - 0.5) * float2(aspect, 1.0);
    float reach = length(centered) / length(float2(0.5 * aspect, 0.5));
    float falloff = pow(cos(atan(reach * 0.9)), 4.0);
    return lerp(1.0, falloff, push.vignette);
}

float grain_noise(float2 pixel, uint seed, uint channel) {
    float2 at = pixel / 1.35;
    float2 cell = floor(at);
    float2 blend = smoothstep(0.0, 1.0, fract(at));
    uint2 base = uint2(int2(cell) + 4096);
    float a = post_random(uint3(base, seed * 3u + channel));
    float b = post_random(uint3(base + uint2(1, 0), seed * 3u + channel));
    float c = post_random(uint3(base + uint2(0, 1), seed * 3u + channel));
    float d = post_random(uint3(base + uint2(1, 1), seed * 3u + channel));
    return lerp(lerp(a, b, blend.x), lerp(c, d, blend.x), blend.y) * 2.0 - 1.0;
}

float3 film_grain(float2 pixel, float3 encoded) {
    float luma = clamp(rikka_luma(encoded), 0.0, 1.0);
    float response = 0.85 * pow(4.0 * luma * (1.0 - luma), 0.7) + 0.15 * (1.0 - luma);
    float mono = grain_noise(pixel, push.seed, 0u);
    float3 tint = float3(grain_noise(pixel, push.seed, 1u), 0.0, grain_noise(pixel, push.seed, 2u)) * 0.2;
    return encoded + (float3(mono) + tint) * response * push.grain * 0.14;
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    float2 uv = (float2(pixel) + 0.5) / push.size;
    float aspect = push.size.x / push.size.y;
    float4 color = post_fetch(push.source, pixel);
    if (push.sharpen > 0.0) {
        color = sharpened(pixel, color);
    }
    if (push.chromatic > 0.0) {
        float2 centered = (uv - 0.5) * float2(aspect, 1.0);
        float2 shift = (uv - 0.5) * dot(centered, centered) * push.chromatic * 0.025;
        color.r = post_sample(push.source, uv - shift).r;
        color.b = post_sample(push.source, uv + shift).b;
    }
    float alpha = clamp(color.a, 0.0, 1.0);
    float3 encoded = alpha > 1e-4 ? color.rgb / alpha : float3(0.0);
    if (push.vignette > 0.0) {
        encoded = rikka_encode_srgb(rikka_decode_srgb(clamp(encoded, 0.0, 1.0)) * natural_vignette(uv, aspect));
    }
    if (push.grain > 0.0) {
        encoded = film_grain(float2(pixel), encoded);
    }
    return float4(clamp(encoded, 0.0, 1.0) * alpha, alpha);
}
