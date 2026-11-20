import fluid;

struct Push {
    ReadUints offsets;
    ReadUints entry_bins;
    ReadUints entry_slots;
    ReadUints entry_tris;
    ReadUints bin_starts;
    Uints items;
    uint triangles;
    uint capacity;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint e = thread.x;
    if (e >= min(pc.offsets[pc.triangles], pc.capacity)) {
        return;
    }
    pc.items[pc.bin_starts[pc.entry_bins[e]] + pc.entry_slots[e]] = pc.entry_tris[e];
}
