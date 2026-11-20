import plane;

struct Push {
    Head head;
    float sigma;
    int radius;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(16, 16, 1)]
void gauss(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    int2 step = pc.head.mode == 0u ? int2(1, 0) : int2(0, 1);
    float spread = 2.0 * pc.sigma * pc.sigma;
    float4 sum = pc.head.src.load(pixel);
    float total = 1.0;
    for (int i = 1; i <= pc.radius; i++) {
        float w = exp(-float(i * i) / spread);
        sum += w * (pc.head.src.load(pixel + step * i) + pc.head.src.load(pixel - step * i));
        total += 2.0 * w;
    }
    pc.head.dst.store(pixel, pc.head.clamped(sum / total));
}
