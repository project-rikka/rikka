import plane;

struct Push {
    Head head;
    int cell;
    int pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint REDUCE = 0u;
static const uint EXPAND = 1u;

[shader("compute")]
[numthreads(16, 16, 1)]
void mosaic(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    if (pc.head.mode == EXPAND) {
        pc.head.dst.store(pixel, pc.head.src.load(pixel / pc.cell));
        return;
    }
    int2 start = pixel * pc.cell;
    int2 end = min(start + pc.cell, pc.head.src.size());
    float4 sum = float4(0.0);
    for (int y = start.y; y < end.y; y++) {
        for (int x = start.x; x < end.x; x++) {
            sum += pc.head.src.load(int2(x, y));
        }
    }
    int2 span = end - start;
    pc.head.dst.store(pixel, pc.head.clamped(sum / float(span.x * span.y)));
}
