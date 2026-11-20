import fluid;
import fluid.bonds;

struct Push {
    ReadUints perm;
    ReadUints inverse;
    ReadUints phases_in;
    ReadUints counts_in;
    ReadUints partners_in;
    ReadFloats rests_in;
    Uints counts_out;
    Uints partners_out;
    Floats rests_out;
    Uints owned;
    Floats nearest;
    Floats dilations;
    ReadVec4s positions;
    uint count;
    uint stride;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static uint words[MAX_BONDS];
static float rests[MAX_BONDS];

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint old = pc.perm[i];
    uint bond_count = is_dead(pc.phases_in[old]) ? 0u : pc.counts_in[old];
    uint owned_count = 0u;
    uint other_count = 0u;
    uint colours = 0u;
    float closest = FOREVER;
    float stretch = 0.0;
    float3 xi = pc.positions[i].xyz;
    uint from = old;
    for (uint k = 0u; k < bond_count; ++k) {
        uint word = pc.partners_in[from];
        uint partner = bond_partner(word);
        if (!is_dead(pc.phases_in[partner])) {
            uint moved = pc.inverse[partner];
            uint colour = bond_colour(word);
            float distance = length(xi - pc.positions[moved].xyz);
            float rest = pc.rests_in[from];
            closest = min(closest, distance);
            stretch += distance / rest;
            uint packed = bond_word(moved, colour);
            if (colour != UNCOLOURED && i < moved) {
                colours |= 1u << colour;
                uint at = owned_count;
                for (; at > 0u; at -= 1u) {
                    if (bond_colour(words[at - 1u]) < colour) {
                        break;
                    }
                    words[at] = words[at - 1u];
                    rests[at] = rests[at - 1u];
                }
                words[at] = packed;
                rests[at] = rest;
                owned_count += 1u;
            } else {
                other_count += 1u;
                uint at = MAX_BONDS - other_count;
                words[at] = packed;
                rests[at] = rest;
            }
        }
        from += pc.stride;
    }
    uint into = i;
    for (uint k = 0u; k < owned_count; ++k) {
        pc.partners_out[into] = words[k];
        pc.rests_out[into] = rests[k];
        into += pc.stride;
    }
    for (uint k = 0u; k < other_count; ++k) {
        uint at = MAX_BONDS - 1u - k;
        pc.partners_out[into] = words[at];
        pc.rests_out[into] = rests[at];
        into += pc.stride;
    }
    uint kept = owned_count + other_count;
    pc.counts_out[i] = kept;
    pc.owned[i] = colours;
    pc.nearest[i] = closest;
    pc.dilations[i] = kept > 0u ? stretch / float(kept) : 1.0;
}
