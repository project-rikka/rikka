import fluid;
import fluid.bonds;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadUints phases;
    ReadUints list;
    ReadUints listed;
    ReadUints bond_counts;
    ReadUints partners;
    ReadFloats rests;
    Uints proposals;
    uint count;
    uint stride;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float KEPT = 0.8;

static uint chosen[MAX_BONDS];
static float keys[MAX_BONDS];
static uint existing[MAX_BONDS];
static uint found = 0u;

void offer(uint j, float key) {
    if (found == MAX_BONDS && (key > keys[MAX_BONDS - 1u] || (key == keys[MAX_BONDS - 1u] && j > chosen[MAX_BONDS - 1u]))) {
        return;
    }
    uint at = found < MAX_BONDS ? found : MAX_BONDS - 1u;
    for (; at > 0u; at -= 1u) {
        if (!(keys[at - 1u] > key || (keys[at - 1u] == key && chosen[at - 1u] > j))) {
            break;
        }
        keys[at] = keys[at - 1u];
        chosen[at] = chosen[at - 1u];
    }
    keys[at] = key;
    chosen[at] = j;
    found = min(found + 1u, MAX_BONDS);
}

struct ProposeWalk : IListedVisitor {
    uint i;
    uint substance;
    bool pinned;
    uint bond_count;
    float reach2;
    float3 xi;

    [mutating]
    void visit(uint slot, uint j) {
        uint other = pc.phases[j];
        if (j == i || substance_of(other) != substance || (pinned && (other & PINNED) != 0u)) {
            return;
        }
        float3 d = xi - pc.positions[j].xyz;
        float r2 = dot(d, d);
        if (r2 >= reach2) {
            return;
        }
        for (uint k = 0u; k < bond_count; ++k) {
            if (existing[k] == j) {
                return;
            }
        }
        offer(j, sqrt(r2));
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint phase = pc.phases[i];
    Settings s = pc.settings;
    uint substance = substance_of(phase);
    if (is_dead(phase) || s.substances[substance].bond.x <= 0.0) {
        pc.proposals[i] = NONE;
        return;
    }
    bool pinned = (phase & PINNED) != 0u;
    uint bond_count = pc.bond_counts[i];
    float reach2 = s.solver.z * s.solver.z;
    float3 xi = pc.positions[i].xyz;
    uint slot = i;
    for (uint k = 0u; k < bond_count; ++k) {
        uint j = bond_partner(pc.partners[slot]);
        existing[k] = j;
        if (!(pinned && (pc.phases[j] & PINNED) != 0u)) {
            float distance = length(xi - pc.positions[j].xyz);
            float rest = pc.rests[slot];
            bool holding = distance <= rest * (1.0 + s.substances[substance].bond.z);
            offer(j, holding ? 0.0 : KEPT * distance);
        }
        slot += pc.stride;
    }
    ProposeWalk walk = { i, substance, pinned, bond_count, reach2, xi };
    for_listed(pc.list, pc.listed, pc.stride, i, walk);
    uint out_slot = i;
    for (uint k = 0u; k < MAX_BONDS; ++k) {
        pc.proposals[out_slot] = k < found ? chosen[k] : NONE;
        if (k >= found) {
            break;
        }
        out_slot += pc.stride;
    }
}
