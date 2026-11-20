import fluid;

struct Push {
    ReadUints offsets;
    ReadVec4s positions_in;
    ReadVec4s velocities_in;
    ReadUints meta_in;
    Vec4s positions_out;
    Vec4s velocities_out;
    Uints meta_out;
    Uints counters;
    uint resident;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i == 0u) {
        pc.counters[FOAM_COMPACTED] = pc.offsets[pc.resident];
    }
    if (i >= pc.resident) {
        return;
    }
    uint at = pc.offsets[i];
    if (pc.offsets[i + 1u] == at) {
        return;
    }
    pc.positions_out[at] = pc.positions_in[i];
    pc.velocities_out[at] = pc.velocities_in[i];
    pc.meta_out[at] = pc.meta_in[i];
}
