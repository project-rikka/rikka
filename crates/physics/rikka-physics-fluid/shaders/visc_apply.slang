import fluid;
import fluid.reduction;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    ReadUvec2s normals;
    ReadVec4s direction;
    Vec4s product;
    Sums sums;
    ReadVec4s contacts;
    Floats couplings;
    uint count;
    uint stride;
    float dt;
    uint first;
    uint boundaries;
    uint now;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

struct ViscApplyFirst : IListedVisitor {
    Settings s;
    uint i;
    float3 xi;
    float3 p;
    float rho_i;
    float nu_i;
    float3 q;

    [mutating]
    void visit(uint slot, uint j) {
        Coupling coupling = viscous_coupling(s, pc.normals, pc.phases, pc.positions, xi, rho_i, nu_i, j, pc.dt);
        pc.couplings[slot * pc.stride + i] = coupling.k;
        if (coupling.k > 0.0) {
            q += coupling.k * coupling.d * dot(coupling.d, p - pc.direction[j].xyz);
        }
    }
}

struct ViscApplyCached : IListedVisitor {
    uint i;
    float3 xi;
    float3 p;
    float3 q;

    [mutating]
    void visit(uint slot, uint j) {
        float k = pc.couplings[slot * pc.stride + i];
        if (k > 0.0) {
            float3 d = xi - pc.positions[j].xyz;
            q += k * d * dot(d, p - pc.direction[j].xyz);
        }
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID, uint3 local : SV_GroupThreadID) {
    float rz_now = reduced(pc.sums, pc.now);
    if (rz_now <= VISCOUS_CONVERGED) {
        return;
    }
    uint i = thread.x;
    uint t = local.x;
    float pq = 0.0;
    if (i < pc.count) {
        uint phase = pc.phases[i];
        float3 p = pc.direction[i].xyz;
        float3 q = p;
        if (!is_dead(phase)) {
            float3 xi = pc.positions[i].xyz;
            if (pc.first != 0u || pc.boundaries != 0u) {
                Settings s = pc.settings;
                float nu_i = s.substances[substance_of(phase)].flow.x;
                if (pc.boundaries != 0u) {
                    float3 surface;
                    q += boundary_coupling(s, pc.contacts, i, nu_i, pc.dt, surface) * p;
                }
                if (pc.first != 0u) {
                    float rho_i = normal_of(pc.normals, i).w;
                    ViscApplyFirst walk = { s, i, xi, p, rho_i, nu_i, q };
                    for_listed(pc.list, pc.listed, pc.stride, i, walk);
                    q = walk.q;
                } else {
                    ViscApplyCached walk = { i, xi, p, q };
                    for_listed(pc.list, pc.listed, pc.stride, i, walk);
                    q = walk.q;
                }
            } else {
                ViscApplyCached walk = { i, xi, p, q };
                for_listed(pc.list, pc.listed, pc.stride, i, walk);
                q = walk.q;
            }
        } else {
            q = float3(0.0);
        }
        pc.product[i] = float4(q, 0.0);
        pq = dot(p, q);
    }
    reduce_groups(pc.sums, 2u, pq, t);
}
