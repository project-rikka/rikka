import plane;

struct Push {
    Head head;
    uint* bounds;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint NONE = 0xffffffffu;

[shader("compute")]
[numthreads(16, 16, 1)]
void differ(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.src.contains(pixel)) {
        return;
    }
    if (all(pc.head.src.load(pixel) == pc.head.dst.load(pixel))) {
        return;
    }
    uint2 at = uint2(pixel);
    InterlockedMax(pc.bounds[0], NONE - at.x);
    InterlockedMax(pc.bounds[1], NONE - at.y);
    InterlockedMax(pc.bounds[2], at.x + 1u);
    InterlockedMax(pc.bounds[3], at.y + 1u);
}
