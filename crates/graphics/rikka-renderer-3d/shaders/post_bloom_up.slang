import rikka.post;

struct Push {
    uint lower;
    uint current;
    uint destination;
    uint pad;
    uint2 size;
    float2 texel;
    float spread;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float4 tent(float2 uv) {
    float2 t = push.texel;
    return (post_sample(push.lower, uv + float2(-t.x, -t.y))
            + 2.0 * post_sample(push.lower, uv + float2(0.0, -t.y))
            + post_sample(push.lower, uv + float2(t.x, -t.y))
            + 2.0 * post_sample(push.lower, uv + float2(-t.x, 0.0))
            + 4.0 * post_sample(push.lower, uv)
            + 2.0 * post_sample(push.lower, uv + float2(t.x, 0.0))
            + post_sample(push.lower, uv + float2(-t.x, t.y))
            + 2.0 * post_sample(push.lower, uv + float2(0.0, t.y))
            + post_sample(push.lower, uv + float2(t.x, t.y)))
        / 16.0;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    float2 uv = (float2(texel) + 0.5) / float2(push.size);
    float4 current = post_fetch(push.current, texel);
    post_store(push.destination, texel, lerp(current, tent(uv), push.spread));
}
