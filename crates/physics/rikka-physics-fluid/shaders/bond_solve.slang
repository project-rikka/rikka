import fluid;
import fluid.bonds;

struct Push {
    Settings settings;
    Vec4s predicted;
    ReadVec4s positions;
    ReadUints phases;
    ReadUints owned;
    ReadUints partners;
    ReadFloats rests;
    Floats bond_lambdas;
    Floats clearances;
    uint count;
    uint stride;
    uint colour;
    uint first;
    uint walled;
    float dt;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

float mobility(uint phase) {
    return (phase & PINNED) != 0u ? 0.0 : 1.0;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint owned_mask = pc.owned[i];
    uint bit = 1u << pc.colour;
    if ((owned_mask & bit) == 0u) {
        return;
    }
    uint slot = countbits(owned_mask & (bit - 1u)) * pc.stride + i;
    uint j = bond_partner(pc.partners[slot]);
    uint phase = pc.phases[i];
    float lambda = pc.first != 0u ? 0.0 : pc.bond_lambdas[slot];
    float wi = mobility(phase);
    float wj = mobility(pc.phases[j]);
    float4 a = pc.predicted[i];
    float4 b = pc.predicted[j];
    float rest = pc.rests[slot];
    bool walled = pc.walled != 0u;
    float ci = walled && wi > 0.0 ? pc.clearances[i] : 0.0;
    float cj = walled && wj > 0.0 ? pc.clearances[j] : 0.0;
    float3 d = a.xyz - b.xyz;
    float distance = length(d);
    if (wi + wj <= 0.0 || distance <= 0.0) {
        pc.bond_lambdas[slot] = lambda;
        return;
    }
    Settings s = pc.settings;
    Substance kind = s.substances[substance_of(phase)];
    float3 n = d / distance;
    float reach = kernel_h(s);
    float strength = max(reach - rest, 0.0) / (reach - 2.0 * s.bounds_lo.w);
    if (strength <= 0.0) {
        pc.bond_lambdas[slot] = lambda;
        return;
    }
    float inv_dt = 1.0 / pc.dt;
    float softness = kind.bond.x * (inv_dt * inv_dt) / strength;
    float damping = kind.bond.w * inv_dt;
    float rate = damping > 0.0 ? dot((a.xyz - pc.positions[i].xyz) - (b.xyz - pc.positions[j].xyz), n) : 0.0;
    float step = (rest - distance - softness * lambda - damping * rate) / ((1.0 + damping) * (wi + wj) + softness);
    pc.bond_lambdas[slot] = lambda + step;
    if (step != 0.0) {
        float3 move = step * n;
        float moved = abs(step);
        if (wi > 0.0) {
            pc.predicted[i] = float4(a.xyz + move, a.w);
            if (walled) {
                pc.clearances[i] = ci - moved;
            }
        }
        if (wj > 0.0) {
            pc.predicted[j] = float4(b.xyz - move, b.w);
            if (walled) {
                pc.clearances[j] = cj - moved;
            }
        }
    }
}
