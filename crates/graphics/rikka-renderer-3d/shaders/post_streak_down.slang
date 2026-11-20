import rikka.post;

struct Push {
    uint source;
    uint destination;
    uint first;
    uint2 size;
    float2 texel;
    float threshold;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    float2 uv = (float2(texel) + 0.5) / float2(push.size);
    if (push.first != 0u) {
        float4 color = max(post_sample(push.source, uv), float4(0.0));
        float brightness = max(max(color.r, color.g), color.b);
        float knee = 0.5 * push.threshold;
        float soft = clamp(brightness - push.threshold + knee, 0.0, 2.0 * knee);
        soft = soft * soft / (4.0 * max(knee, 1e-4));
        float kept = max(soft, brightness - push.threshold) / max(brightness, 1e-4);
        post_store(push.destination, texel, color * kept);
        return;
    }
    float2 step = float2(push.texel.x * 2.0, 0.0);
    float4 result = 0.25 * post_sample(push.source, uv - step)
        + 0.5 * post_sample(push.source, uv)
        + 0.25 * post_sample(push.source, uv + step);
    post_store(push.destination, texel, result);
}
