import plane;
import color;

struct Push {
    Head head;
    uint transfer;
    uint pad;
    float* tables;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint TO_LINEAR = 0u;
static const uint TO_PREMULTIPLIED = 1u;
static const uint FROM_LINEAR = 2u;
static const uint FROM_PREMULTIPLIED = 3u;

[shader("compute")]
[numthreads(16, 16, 1)]
void linearize(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    if (pc.head.mode == TO_LINEAR) {
        c.rgb = to_linear(c.rgb, pc.transfer, pc.tables) * c.a;
    } else if (pc.head.mode == TO_PREMULTIPLIED) {
        c.rgb *= c.a;
    } else {
        c.rgb = c.a > 0.0 ? c.rgb / c.a : float3(0.0);
        if (pc.head.mode == FROM_LINEAR) {
            c.rgb = from_linear(c.rgb, pc.transfer, pc.tables);
        }
    }
    pc.head.dst.store(pixel, pc.head.clamped(c));
}
