import plane;
import color;
import workgroup;

struct Push {
    Head head;
    uint* counts;
    float dark;
    float light;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint BINS = 1024u;
static const uint COUNT = 0u;
static const uint CLASSES = 1u;

groupshared uint bins[4u * BINS];
groupshared uint sums[12];

uint bin_of(float v) {
    return min(uint(clamp(v, 0.0, 1.0) * float(BINS)), BINS - 1u);
}

void add_wide(uint at, uint value) {
    uint previous;
    InterlockedAdd(pc.counts[at], value, previous);
    if (previous > 0xffffffffu - value) {
        InterlockedAdd(pc.counts[at + 1u], 1u);
    }
}

[shader("compute")]
[numthreads(16, 16, 1)]
void histogram(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex) {
    if (pc.head.mode == COUNT) {
        for (uint i = lane; i < 4u * BINS; i += 256u) {
            shared_store(__getAddress(bins[i]), 0u);
        }
    } else if (lane < 12u) {
        shared_store(__getAddress(sums[lane]), 0u);
    }
    GroupMemoryBarrierWithGroupSync();
    int2 pixel = pc.head.pixel(id);
    if (pc.head.src.contains(pixel)) {
        float4 c = pc.head.src.load(pixel);
        if (c.a > 0.0) {
            float y = lum(c.rgb);
            if (pc.head.mode == COUNT) {
                InterlockedAdd(bins[bin_of(c.r)], 1u);
                InterlockedAdd(bins[BINS + bin_of(c.g)], 1u);
                InterlockedAdd(bins[2u * BINS + bin_of(c.b)], 1u);
                InterlockedAdd(bins[3u * BINS + bin_of(y)], 1u);
            } else {
                uint3 v = uint3(clamp(c.rgb, 0.0, 1.0) * 255.0 + 0.5);
                uint group = 3u;
                if (y <= pc.dark) {
                    group = 0u;
                } else if (y >= pc.light) {
                    group = 1u;
                } else if (y >= 0.25 && y <= 0.75 && sat(c.rgb) <= 0.12) {
                    group = 2u;
                }
                if (group < 3u) {
                    InterlockedAdd(sums[group * 4u], v.r);
                    InterlockedAdd(sums[group * 4u + 1u], v.g);
                    InterlockedAdd(sums[group * 4u + 2u], v.b);
                    InterlockedAdd(sums[group * 4u + 3u], 1u);
                }
            }
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (pc.head.mode == COUNT) {
        for (uint i = lane; i < 4u * BINS; i += 256u) {
            uint count = shared_load(__getAddress(bins[i]));
            if (count != 0u) {
                InterlockedAdd(pc.counts[i], count);
            }
        }
    } else if (lane < 12u) {
        uint sum = shared_load(__getAddress(sums[lane]));
        if (sum != 0u) {
            add_wide(lane * 2u, sum);
        }
    }
}
