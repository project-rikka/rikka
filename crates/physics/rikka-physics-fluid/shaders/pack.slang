import fluid;

struct Push {
    ReadVec4s positions;
    ReadVec4s velocities;
    ReadUints phases;
    Vec4s record_positions;
    Uvec2s record_velocities;
    ReadVec4s motion;
    uint count;
    uint moved;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint phase = pc.phases[i];
    if (is_dead(phase)) {
        pc.record_positions[i] = float4(0.0, 0.0, 0.0, -1.0);
        pc.record_velocities[i] = uint2(0u);
        return;
    }
    float3 v = pc.moved != 0u ? pc.motion[i].xyz : pc.velocities[i].xyz;
    pc.record_positions[i] = float4(pc.positions[i].xyz, float(substance_of(phase)));
    pc.record_velocities[i] = uint2(packHalf2x16(v.xy), packHalf2x16(float2(v.z, 0.0)));
}
