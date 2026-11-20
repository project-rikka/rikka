import fluid;

struct Push {
    Settings settings;
    ReadUints perm;
    ReadVec4s positions_in;
    ReadVec4s velocities_in;
    ReadUints phases_in;
    ReadVec4s warm_in;
    Vec4s positions_out;
    Vec4s velocities_out;
    Uints phases_out;
    Vec4s warm_out;
    Vec4s predicted_out;
    ReadUvec4s anchors_in;
    Uvec4s anchors_out;
    uint count;
    float dt;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint old = pc.perm[i];
    uint phase = pc.phases_in[old];
    float4 x = pc.positions_in[old];
    float4 v = pc.velocities_in[old];
    float4 predicted_x = x;
    if (!is_dead(phase)) {
        predicted_x = float4(floored(pc.settings, x.xyz, x.xyz + v.xyz * pc.dt, phase), 0.0);
    }
    pc.positions_out[i] = x;
    pc.velocities_out[i] = v;
    pc.phases_out[i] = phase;
    pc.warm_out[i] = pc.warm_in[old];
    pc.predicted_out[i] = predicted_x;
    pc.anchors_out[i] = is_dead(phase) ? uint4(NONE) : pc.anchors_in[old];
}
