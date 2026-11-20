import plane;

struct Push {
    Head head;
    float amount;
    uint monochromatic;
    uint seed;
    uint pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint UNIFORM = 0u;
static const uint GAUSSIAN = 1u;

float draw(uint2 at, uint channel) {
    float u = hash01(uint3(at, pc.seed * 4u + channel));
    if (pc.head.mode == UNIFORM) {
        return (u * 2.0 - 1.0) * 1.7320508;
    }
    float v = hash01(uint3(at.yx ^ uint2(0x9e3779b9u), pc.seed * 4u + channel + 977u));
    return sqrt(-2.0 * log(max(u, 1e-7))) * cos(6.28318530718 * v);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void noise(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    uint2 at = uint2(pixel);
    float3 noise;
    if (pc.monochromatic != 0u) {
        noise = float3(draw(at, 0u));
    } else {
        noise = float3(draw(at, 0u), draw(at, 1u), draw(at, 2u));
    }
    c.rgb += noise * pc.amount;
    pc.head.dst.store(pixel, pc.head.clamped(c));
}
