import fluid;
import fluid.bonds;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadUints phases;
    Uints bond_counts;
    Uints partners;
    Floats rests;
    ReadFloats nearest;
    uint count;
    uint stride;
    float dt;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float ISOLATED = 1.25;
static const float TIGHTENING = 20.0;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint bond_count = pc.bond_counts[i];
    if (bond_count == 0u) {
        return;
    }
    uint phase = pc.phases[i];
    if (is_dead(phase)) {
        pc.bond_counts[i] = 0u;
        return;
    }
    Settings s = pc.settings;
    Substance kind = s.substances[substance_of(phase)];
    float longest = s.bounds_hi.w;
    float snap = s.solver.w;
    float spacing = 2.0 * s.bounds_lo.w;
    float rate = min(kind.bond.y * pc.dt, 1.0);
    float yield_ratio = kind.bond.z;
    float3 xi = pc.positions[i].xyz;
    float nearest_i = pc.nearest[i];
    uint kept = 0u;
    for (uint k = 0u; k < bond_count; ++k) {
        uint slot = k * pc.stride + i;
        uint word = pc.partners[slot];
        uint j = bond_partner(word);
        uint partner = pc.phases[j];
        if (is_dead(partner)) {
            continue;
        }
        float distance = length(xi - pc.positions[j].xyz);
        float rest = pc.rests[slot];
        float tolerated = yield_ratio * rest;
        bool footed = ((phase | partner) & PINNED) != 0u;
        bool isolated = min(nearest_i, pc.nearest[j]) > ISOLATED * spacing;
        if (isolated) {
            rest -= min(TIGHTENING * pc.dt, 1.0) * (rest - spacing);
        } else if (distance > rest + tolerated && !footed) {
            rest += rate * (distance - rest - tolerated);
        } else if (distance < rest - tolerated) {
            rest -= rate * (rest - tolerated - distance);
        }
        rest = max(rest, spacing);
        if (rest > longest || distance > snap) {
            continue;
        }
        uint into = kept * pc.stride + i;
        pc.partners[into] = word;
        pc.rests[into] = rest;
        kept += 1u;
    }
    pc.bond_counts[i] = kept;
}
