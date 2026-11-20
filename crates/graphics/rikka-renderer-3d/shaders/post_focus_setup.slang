import rikka.post;
import rikka.post.focus;

struct Push {
    uint source;
    uint depth;
    uint color;
    uint coc;
    uint2 size;
    uint2 full;
    float scale;
    float ratio;
    float limit;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    if (any(texel >= int2(push.size))) {
        return;
    }
    int2 full = int2(push.full);
    float4 colors[4];
    float radii[4];
    float chosen = 0.0;
    for (int i = 0; i < 4; ++i) {
        int2 pixel = min(texel * 2 + int2(i & 1, i >> 1), full - 1);
        float depth = post_fetch(push.depth, pixel).r;
        radii[i] = post_circle_of_confusion(depth, push.scale, push.ratio, push.limit) * 0.5;
        colors[i] = post_fetch(push.source, pixel);
        if (abs(radii[i]) > abs(chosen)) {
            chosen = radii[i];
        }
    }
    float4 color = float4(0.0);
    float coc = 0.0;
    float kept = 0.0;
    for (int i = 0; i < 4; ++i) {
        float weight = 1.0 - smoothstep(0.5, 1.0, abs(radii[i] - chosen));
        color += colors[i] * weight;
        coc += radii[i] * weight;
        kept += weight;
    }
    post_store(push.color, texel, color / kept);
    post_store_scalar(push.coc, texel, coc / kept);
}
