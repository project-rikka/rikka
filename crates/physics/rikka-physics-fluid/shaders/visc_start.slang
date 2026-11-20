import glsl;
import fluid;
import fluid.reduction;
import physics.matrix;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    ReadUvec2s normals;
    ReadVec4s targets;
    ReadVec4s warm;
    Vec4s iterate;
    Vec4s preconditioned;
    Vec4s direction;
    Floats diagonal;
    Sums sums;
    ReadVec4s contacts;
    uint count;
    uint stride;
    float dt;
    uint boundaries;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

struct ViscStartWalk : IListedVisitor {
    Settings s;
    float3 xi;
    float rho_i;
    float nu_i;
    float3x3 system;
    float3 coupled;

    [mutating]
    void visit(uint slot, uint j) {
        Coupling coupling = viscous_coupling(s, pc.normals, pc.phases, pc.positions, xi, rho_i, nu_i, j, pc.dt);
        if (coupling.k > 0.0) {
            float3x3 block = scaled(outer_product(coupling.d, coupling.d), coupling.k);
            system += block;
            coupled += mul(pc.targets[j].xyz + pc.warm[j].xyz, block);
        }
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID, uint3 local : SV_GroupThreadID) {
    uint i = thread.x;
    uint t = local.x;
    float rz = 0.0;
    if (i < pc.count) {
        uint phase = pc.phases[i];
        float3 b = pc.targets[i].xyz;
        float3 x = b + pc.warm[i].xyz;
        float3 r = float3(0.0);
        float3 z = float3(0.0);
        float3x3 inverse_diagonal = IDENTITY3;
        if (!is_dead(phase)) {
            Settings s = pc.settings;
            float nu_i = s.substances[substance_of(phase)].flow.x;
            float3 xi = pc.positions[i].xyz;
            float rho_i = normal_of(pc.normals, i).w;
            float3x3 system = IDENTITY3;
            if (pc.boundaries != 0u) {
                float3 surface;
                float coupling = boundary_coupling(s, pc.contacts, i, nu_i, pc.dt, surface);
                system += diagonal3(coupling);
                b += coupling * surface;
            }
            ViscStartWalk walk = { s, xi, rho_i, nu_i, system, float3(0.0) };
            for_listed(pc.list, pc.listed, pc.stride, i, walk);
            inverse_diagonal = inverse(walk.system);
            r = b - (mul(x, walk.system) - walk.coupled);
            z = mul(r, inverse_diagonal);
            rz = dot(r, z);
        }
        pc.iterate[i] = float4(x, 0.0);
        pc.iterate[pc.stride + i] = float4(r, 0.0);
        pc.preconditioned[i] = float4(z, 0.0);
        pc.direction[i] = float4(z, 0.0);
        store_symmetric(pc.diagonal, pc.stride, i, inverse_diagonal);
    }
    reduce_groups(pc.sums, 0u, rz, t);
}
