import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadVec4s velocities;
    ReadUints phases;
    Uints keys;
    Uints slots;
    Uints counts;
    uint count;
    float dt;
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
        key = s.grid.w;
        uint phase = pc.phases[p];
        if (!is_dead(phase)) {
            float3 x0 = pc.positions[p].xyz;
            float3 x = floored(s, x0, x0 + pc.velocities[p].xyz * pc.dt, phase);
            key = bin_of(cell_of(x, s.grid_cell.y), s.grid);
        }
    }
    record_bin(pc.keys, pc.slots, pc.counts, p, active, key);
}
