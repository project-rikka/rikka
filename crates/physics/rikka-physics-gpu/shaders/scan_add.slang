import physics.memory;

struct Push {
    Uints values;
    ReadUints sums;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(256, 1, 1)]
void main(uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    uint block = group.x + 1u;
    uint offset = pc.sums[block];
    if (offset == 0u) {
        return;
    }
    uint base = block * 1024u + local.x * 4u;
    if (base + 3u < pc.count) {
        pc.values[base] += offset;
        pc.values[base + 1u] += offset;
        pc.values[base + 2u] += offset;
        pc.values[base + 3u] += offset;
    } else {
        if (base < pc.count) {
            pc.values[base] += offset;
        }
        if (base + 1u < pc.count) {
            pc.values[base + 1u] += offset;
        }
        if (base + 2u < pc.count) {
            pc.values[base + 2u] += offset;
        }
    }
}
