import plane;

struct Push {
    Head head;
    int radius;
    uint steps;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(16, 16, 1)]
void median(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    int r = pc.radius;
    float reach = (float(r) + 0.5) * (float(r) + 0.5);
    float4 low = float4(1e30);
    float4 high = float4(-1e30);
    float members = 0.0;
    for (int y = -r; y <= r; y++) {
        for (int x = -r; x <= r; x++) {
            if (float(x * x + y * y) > reach) {
                continue;
            }
            float4 v = pc.head.src.load(pixel + int2(x, y));
            low = min(low, v);
            high = max(high, v);
            members += 1.0;
        }
    }
    float half_count = members * 0.5;
    for (uint pass_index = 0u; pass_index < pc.steps; pass_index++) {
        float4 middle = (low + high) * 0.5;
        float4 below = float4(0.0);
        for (int y = -r; y <= r; y++) {
            for (int x = -r; x <= r; x++) {
                if (float(x * x + y * y) > reach) {
                    continue;
                }
                below += step(pc.head.src.load(pixel + int2(x, y)), middle);
            }
        }
        float4 enough = float4(below >= float4(half_count));
        high = lerp(high, middle, enough);
        low = lerp(middle, low, enough);
    }
    pc.head.dst.store(pixel, pc.head.clamped(high));
}
