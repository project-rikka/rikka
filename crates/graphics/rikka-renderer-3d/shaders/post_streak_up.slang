import rikka.post;

struct Push {
    uint lower;
    uint current;
    uint destination;
    uint pad;
    uint2 size;
    float2 texel;
    float stretch;
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
    float2 step = float2(push.texel.x * 0.75, 0.0);
    float4 lower = 0.5 * (post_sample(push.lower, uv - step) + post_sample(push.lower, uv + step));
    float4 current = post_fetch(push.current, texel);
    post_store(push.destination, texel, lerp(current, lower, push.stretch));
}
