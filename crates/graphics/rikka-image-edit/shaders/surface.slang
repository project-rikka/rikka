import plane;

struct Push {
    Head head;
    int radius;
    float threshold;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(16, 16, 1)]
void surface(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 center = pc.head.src.load(pixel);
    float scale = 1.0 / max(2.5 * pc.threshold, 1e-6);
    float4 sum = float4(0.0);
    float4 total = float4(0.0);
    for (int y = -pc.radius; y <= pc.radius; y++) {
        for (int x = -pc.radius; x <= pc.radius; x++) {
            float4 v = pc.head.src.load(pixel + int2(x, y));
            float4 w = max(1.0 - abs(v - center) * scale, 0.0);
            sum += w * v;
            total += w;
        }
    }
    float4 result = sum / max(total, float4(1e-6));
    result.a = center.a;
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
