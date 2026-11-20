import rikka.post;
import rikka.post.focus;

struct Push {
    uint source;
    uint destination;
    uint depth;
    uint far;
    uint near;
    uint coverage;
    uint coc;
    uint2 size;
    uint2 half_size;
    float scale;
    float ratio;
    float limit;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float4 smooth_sample(uint image, float2 uv, float2 texel) {
    return 0.25 * (post_sample(image, uv + float2(-0.5, -0.5) * texel)
        + post_sample(image, uv + float2(0.5, -0.5) * texel)
        + post_sample(image, uv + float2(-0.5, 0.5) * texel)
        + post_sample(image, uv + float2(0.5, 0.5) * texel));
}

float4 sample_far_field(float2 uv, float coc) {
    float2 position = uv * float2(push.half_size) - 0.5;
    int2 base = int2(floor(position));
    float2 blend = fract(position);
    int2 limit = int2(push.half_size) - 1;
    float4 sum = float4(0.0);
    float total = 0.0;
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            int2 at = clamp(base + int2(x, y), int2(0), limit);
            float bilinear = (x == 0 ? 1.0 - blend.x : blend.x) * (y == 0 ? 1.0 - blend.y : blend.y);
            float other = post_fetch(push.coc, at).r * 2.0;
            float alike = 1.0 / (1.0 + 4.0 * abs(other - coc));
            float weight = bilinear * alike + 1e-4;
            sum += post_fetch(push.far, at) * weight;
            total += weight;
        }
    }
    return sum / total;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    if (any(pixel >= int2(push.size))) {
        return;
    }
    float4 sharp = post_fetch(push.source, pixel);
    float depth = post_fetch(push.depth, pixel).r;
    float coc = post_circle_of_confusion(depth, push.scale, push.ratio, push.limit);
    float2 uv = (float2(pixel) + 0.5) / float2(push.size);
    float2 texel = 1.0 / float2(push.half_size);

    float4 result = sharp;
    float far_blend = smoothstep(0.75, 2.5, coc);
    if (far_blend > 0.0) {
        result = lerp(sharp, sample_far_field(uv, coc), far_blend);
    }
    float cover = max(smooth_sample(push.coverage, uv, texel).r, smoothstep(1.5, 3.5, -coc));
    if (cover > 0.0) {
        result = lerp(result, smooth_sample(push.near, uv, texel), cover);
    }
    post_store(push.destination, pixel, result);
}
