import plane;

struct Push {
    Head head;
    float luma_strength;
    float color_strength;
    float preserve;
    int search;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float3 LUMA = float3(0.299, 0.587, 0.114);

float3 opponent(float3 c) {
    float y = dot(c, LUMA);
    return float3(y, c.b - y, c.r - y);
}

float3 from_opponent(float3 o) {
    float r = o.z + o.x;
    float b = o.y + o.x;
    float g = (o.x - LUMA.r * r - LUMA.b * b) / LUMA.g;
    return float3(r, g, b);
}

float luma_at(int2 at) {
    return dot(pc.head.src.load(at).rgb, LUMA);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void denoise(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 center = pc.head.src.load(pixel);
    float3 here = opponent(center.rgb);
    float h_luma = max(pc.luma_strength * pc.luma_strength, 1e-8);
    float h_color = max(pc.color_strength * pc.color_strength, 1e-8);
    float luma_sum = 0.0;
    float luma_total = 0.0;
    float2 color_sum = float2(0.0);
    float color_total = 0.0;
    for (int y = -pc.search; y <= pc.search; y++) {
        for (int x = -pc.search; x <= pc.search; x++) {
            int2 there = pixel + int2(x, y);
            float distance = 0.0;
            for (int py = -1; py <= 1; py++) {
                for (int px = -1; px <= 1; px++) {
                    float d = luma_at(pixel + int2(px, py)) - luma_at(there + int2(px, py));
                    distance += d * d;
                }
            }
            distance /= 9.0;
            float3 other = opponent(pc.head.src.load(there).rgb);
            float w = exp(-distance / h_luma);
            luma_sum += w * other.x;
            luma_total += w;
            float2 chroma = other.yz - here.yz;
            float wc = exp(-(dot(chroma, chroma) + distance) / h_color);
            color_sum += wc * other.yz;
            color_total += wc;
        }
    }
    float luma = lerp(luma_sum / luma_total, here.x, pc.preserve);
    float2 chroma = color_sum / color_total;
    float3 result = from_opponent(float3(luma, chroma));
    pc.head.dst.store(pixel, pc.head.clamped(float4(result, center.a)));
}
