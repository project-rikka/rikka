import rikka.post;

struct Push {
    uint source;
    uint destination;
    uint state;
    float exposure;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    float exposure = post_exposure_or(push.state, push.exposure);
    float4 color = post_fetch(push.source, texel);
    post_store(push.destination, texel, float4(color.rgb * exposure, color.a));
}
