import plane;
import color;

struct Push {
    Head head;
    Plane base;
    float4 shadows;
    float4 highlights;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(16, 16, 1)]
void tonal(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    float4 soft = pc.base.load(pixel);
    float local = soft.a > 0.0 ? lum(soft.rgb / soft.a) : lum(c.rgb);
    float shadow_reach = max(pc.shadows[1], 1e-3);
    float highlight_reach = max(pc.highlights[1], 1e-3);
    float in_shadow = clamp(1.0 - local / shadow_reach, 0.0, 1.0);
    float in_highlight = clamp((local - (1.0 - highlight_reach)) / highlight_reach, 0.0, 1.0);
    in_shadow *= in_shadow;
    in_highlight *= in_highlight;
    float stops = 2.5 * (pc.shadows[0] * in_shadow - pc.highlights[0] * in_highlight);
    float y = lum(c.rgb);
    float3 linear = pow(max(c.rgb, 0.0), float3(2.2)) * exp2(stops);
    float3 lifted = pow(linear / (1.0 + max(linear - 1.0, 0.0)), float3(float(1.0l / 2.2l)));
    float lifted_y = lum(lifted);
    float saturation = lerp(1.0, pc.shadows[2], clamp(abs(stops) / 2.5, 0.0, 1.0));
    float3 result = lifted_y + (lifted - lifted_y) * saturation;
    float midtones = 4.0 * lifted_y * (1.0 - lifted_y);
    result += (lifted_y - 0.5) * pc.highlights[2] * midtones;
    pc.head.dst.store(pixel, pc.head.clamped(float4(result, c.a)));
}
