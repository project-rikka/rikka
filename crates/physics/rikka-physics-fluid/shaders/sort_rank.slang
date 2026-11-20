import fluid;

struct Push {
    ReadUints keys;
    ReadUints slots;
    ReadUints starts;
    ReadUints order;
    Uints perm;
    Uints inverse;
    ReadVec4s records;
    Vec4s sorted;
    uint count;
    uint dead_bin;
    uint carried;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint p = thread.x;
    if (p >= pc.count) {
        return;
    }
    uint key = pc.keys[p];
    uint first = pc.starts[key];
    uint rank = 0u;
    if (key == pc.dead_bin) {
        rank = pc.slots[p];
    } else {
        uint last = pc.starts[key + 1u];
        if (last - first > 1u) {
            for (uint at = first; at < last; ++at) {
                rank += pc.order[at] < p ? 1u : 0u;
            }
        }
    }
    uint dest = first + rank;
    pc.perm[dest] = p;
    pc.inverse[p] = dest;
    if (pc.carried != 0u) {
        pc.sorted[dest] = pc.records[p];
    }
}
