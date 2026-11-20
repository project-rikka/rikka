import plane;

struct Push {
    Head head;
    int radius;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(64, 1, 1)]
void box(uint3 id : SV_DispatchThreadID) {
    bool across = pc.head.mode == 0u;
    int line = int(id.x) + (across ? pc.head.origin[1] : pc.head.origin[0]);
    int lines = across ? int(pc.head.dst.height) : int(pc.head.dst.width);
    int length = across ? int(pc.head.dst.width) : int(pc.head.dst.height);
    if (line >= lines) {
        return;
    }
    int2 step = across ? int2(1, 0) : int2(0, 1);
    int2 start = across ? int2(0, line) : int2(line, 0);
    int r = pc.radius;
    float width = float(2 * r + 1);
    float4 sum = float4(0.0);
    for (int i = -r; i <= r; i++) {
        sum += pc.head.src.load(start + step * i);
    }
    for (int i = 0; i < length; i++) {
        int2 at = start + step * i;
        pc.head.dst.store(at, pc.head.clamped(sum / width));
        sum += pc.head.src.load(at + step * (r + 1)) - pc.head.src.load(at - step * r);
    }
}
