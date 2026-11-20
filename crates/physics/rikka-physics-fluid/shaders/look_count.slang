import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    Uints keys;
    Uints slots;
    Uints counts;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint p = thread.x;
    bool active = p < pc.count;
    uint key = NONE;
    if (active) {
        Settings s = pc.settings;
        float4 x = pc.positions[p];
        key = x.w < 0.0 ? s.grid.w : bin_of(cell_of(x.xyz, s.grid_cell.y), s.grid);
    }
    record_bin(pc.keys, pc.slots, pc.counts, p, active, key);
}
