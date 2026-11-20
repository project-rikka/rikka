import fluid;
import fluid.bonds;

struct Push {
    ReadVec4s positions;
    Uints bond_counts;
    Uints partners;
    Floats rests;
    ReadUints proposals;
    Uints used;
    Uints top;
    uint count;
    uint stride;
    float spacing;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

bool desires(uint j, uint i) {
    for (uint q = 0u; q < MAX_BONDS; ++q) {
        uint wanted = pc.proposals[q * pc.stride + j];
        if (wanted == NONE) {
            return false;
        }
        if (wanted == i) {
            return true;
        }
    }
    return false;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint bond_count = pc.bond_counts[i];
    uint words[MAX_BONDS];
    float rests[MAX_BONDS];
    for (uint k = 0u; k < bond_count; ++k) {
        uint slot = k * pc.stride + i;
        words[k] = pc.partners[slot];
        rests[k] = pc.rests[slot];
    }
    float3 xi = pc.positions[i].xyz;
    Tally tally = NO_TALLY;
    uint kept = 0u;
    for (uint k = 0u; k < MAX_BONDS; ++k) {
        uint j = pc.proposals[k * pc.stride + i];
        if (j == NONE) {
            break;
        }
        if (!desires(j, i)) {
            continue;
        }
        uint word = bond_word(j, UNCOLOURED);
        float rest = max(length(xi - pc.positions[j].xyz), pc.spacing);
        for (uint q = 0u; q < bond_count; ++q) {
            if (bond_partner(words[q]) == j) {
                word = words[q];
                rest = rests[q];
                break;
            }
        }
        uint slot = kept * pc.stride + i;
        pc.partners[slot] = word;
        pc.rests[slot] = rest;
        tally_word(tally, i, word);
        kept += 1u;
    }
    pc.bond_counts[i] = kept;
    pc.used[i] = tally.colours;
    pc.top[i] = tally.best;
}
