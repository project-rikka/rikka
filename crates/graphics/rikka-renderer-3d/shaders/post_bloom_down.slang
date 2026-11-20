import rikka.post;

struct Push {
    uint source;
    uint destination;
    uint first;
    uint2 size;
    float2 texel;
    float threshold;
    float knee;
    float2 source_size;
    float2 source_capacity;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float4 tap(float2 uv, float2 offset) {
    float2 at = rikka_placed_uv(uv + offset * push.texel, push.source_size, push.source_capacity);
    return max(post_sample(push.source, at), float4(0.0));
}

float karis_weight(float4 a, float4 b, float4 c, float4 d) {
    return 1.0 / (1.0 + rikka_luma(0.25 * (a.rgb + b.rgb + c.rgb + d.rgb)));
}

float4 thresholded(float4 color) {
    if (push.threshold <= 0.0) {
        return color;
    }
    float brightness = max(max(color.r, color.g), color.b);
    float knee = max(push.threshold * push.knee, 1e-4);
    float soft = clamp(brightness - push.threshold + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee);
    float kept = max(soft, brightness - push.threshold) / max(brightness, 1e-4);
    return color * kept;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    float2 uv = (float2(texel) + 0.5) / float2(push.size);
    float4 a = tap(uv, float2(-2.0, -2.0));
    float4 b = tap(uv, float2(0.0, -2.0));
    float4 c = tap(uv, float2(2.0, -2.0));
    float4 d = tap(uv, float2(-1.0, -1.0));
    float4 e = tap(uv, float2(1.0, -1.0));
    float4 f = tap(uv, float2(-2.0, 0.0));
    float4 g = tap(uv, float2(0.0, 0.0));
    float4 h = tap(uv, float2(2.0, 0.0));
    float4 i = tap(uv, float2(-1.0, 1.0));
    float4 j = tap(uv, float2(1.0, 1.0));
    float4 k = tap(uv, float2(-2.0, 2.0));
    float4 l = tap(uv, float2(0.0, 2.0));
    float4 m = tap(uv, float2(2.0, 2.0));

    float4 result;
    if (push.first != 0u) {
        float w_inner = 0.5 * karis_weight(d, e, i, j);
        float w_a = 0.125 * karis_weight(a, b, f, g);
        float w_b = 0.125 * karis_weight(b, c, g, h);
        float w_c = 0.125 * karis_weight(f, g, k, l);
        float w_d = 0.125 * karis_weight(g, h, l, m);
        result = (w_inner * (d + e + i + j) + w_a * (a + b + f + g) + w_b * (b + c + g + h)
                  + w_c * (f + g + k + l) + w_d * (g + h + l + m))
            * 0.25 / (w_inner + w_a + w_b + w_c + w_d);
        result = thresholded(result);
    } else {
        result = (d + e + i + j) * 0.125
            + (a + b + f + g + b + c + g + h + f + g + k + l + g + h + l + m) * 0.03125;
    }
    post_store(push.destination, texel, result);
}
