import fluid;

struct Push {
    Settings settings;
    ReadVec4s skin_from;
    ReadVec4s skin_to;
    ReadUvec4s triangles;
    ReadUints offsets;
    Uints entry_bins;
    Uints entry_slots;
    Uints entry_tris;
    Uints bin_counts;
    Uints placing;
    uint count;
    uint capacity;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

bool emit_entry(inout uint e, uint t, int3 cell, uint4 grid) {
    if (e >= pc.capacity) {
        return false;
    }
    uint bin = bin_of(cell, grid);
    pc.entry_bins[e] = bin;
    pc.entry_tris[e] = t;
    uint slot;
    InterlockedAdd(*pc.bin_counts.at(bin), 1u, slot);
    pc.entry_slots[e] = slot;
    e += 1u;
    return true;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint t = thread.x;
    if (t == 0u) {
        uint entries = min(pc.offsets[pc.count], pc.capacity);
        pc.placing[0] = (entries + 127u) / 128u;
        pc.placing[1] = 1u;
        pc.placing[2] = 1u;
    }
    if (t >= pc.count) {
        return;
    }
    Slab slab = triangle_slab(pc.settings, pc.skin_from, pc.skin_to, pc.triangles[t]);
    uint e = pc.offsets[t];
    uint4 grid = pc.settings.tri_grid;
    if (!slab.culled) {
        for (int dz = 0; dz < slab.extent.z; ++dz) {
            for (int dy = 0; dy < slab.extent.y; ++dy) {
                for (int dx = 0; dx < slab.extent.x; ++dx) {
                    if (!emit_entry(e, t, slab.low + int3(dx, dy, dz), grid)) {
                        return;
                    }
                }
            }
        }
        return;
    }
    SlabWalk cells = SlabWalk(slab);
    int3 cell;
    while (cells.next(cell)) {
        if (!emit_entry(e, t, cell, grid)) {
            return;
        }
    }
}
