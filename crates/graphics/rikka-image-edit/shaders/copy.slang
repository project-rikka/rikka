import plane;

struct Push {
    Head head;
    int2 from;
    uint quarter;
    uint mirror;
    float4 fill;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint COPY = 0u;
static const uint REMAP = 1u;
static const uint GRAY = 2u;
static const uint FILL = 3u;
static const uint PASTE = 4u;

int2 source_pixel(int2 at) {
    int2 size = pc.head.src.size();
    int2 turned = at;
    if (pc.quarter == 1u) {
        turned = int2(at.y, size.y - 1 - at.x);
    } else if (pc.quarter == 2u) {
        turned = size - 1 - at;
    } else if (pc.quarter == 3u) {
        turned = int2(size.x - 1 - at.y, at.x);
    }
    if ((pc.mirror & 1u) != 0u) {
        turned.x = size.x - 1 - turned.x;
    }
    if ((pc.mirror & 2u) != 0u) {
        turned.y = size.y - 1 - turned.y;
    }
    return turned;
}

[shader("compute")]
[numthreads(16, 16, 1)]
void copy(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 value;
    if (pc.head.mode == FILL) {
        value = pc.fill;
    } else if (pc.head.mode == PASTE) {
        int2 at = pixel + pc.from;
        if (!pc.head.src.contains(at)) {
            return;
        }
        value = pc.head.src.load(at);
    } else if (pc.head.mode == REMAP) {
        int2 at = pixel + pc.from;
        int2 bounds = (pc.quarter & 1u) != 0u ? pc.head.src.size().yx : pc.head.src.size();
        bool inside = at.x >= 0 && at.y >= 0 && at.x < bounds.x && at.y < bounds.y;
        value = inside ? pc.head.src.load(source_pixel(at)) : pc.fill;
    } else {
        value = pc.head.src.load(pixel + pc.from);
        if (pc.head.mode == GRAY) {
            value.rgb = float3(dot(value.rgb, float3(0.3, 0.59, 0.11)));
        }
    }
    pc.head.dst.store(pixel, pc.head.clamped(value));
}
