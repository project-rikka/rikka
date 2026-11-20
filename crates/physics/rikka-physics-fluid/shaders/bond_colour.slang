import fluid;
import fluid.bonds;

struct Push {
    ReadUints bond_counts;
    Uints partners;
    ReadUints used;
    ReadUints top;
    Uints next_used;
    Uints next_top;
    uint count;
    uint stride;
    uint tallied;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint j = pc.top[i];
    if (j == NONE || pc.top[j] != i) {
        if (pc.tallied != 0u) {
            pc.next_used[i] = pc.used[i];
            pc.next_top[i] = j;
        }
        return;
    }
    uint colour = firstbitlow(~(pc.used[i] | pc.used[j]));
    uint uncoloured = bond_word(j, UNCOLOURED);
    uint bond_count = pc.bond_counts[i];
    Tally tally = NO_TALLY;
    bool pending = true;
    for (uint k = 0u; k < bond_count; ++k) {
        uint slot = k * pc.stride + i;
        uint word = pc.partners[slot];
        if (pending && word == uncoloured) {
            word = bond_word(j, colour);
            pc.partners[slot] = word;
            pending = false;
        }
        if (pc.tallied == 0u && !pending) {
            return;
        }
        tally_word(tally, i, word);
    }
    if (pc.tallied != 0u) {
        pc.next_used[i] = tally.colours;
        pc.next_top[i] = tally.best;
    }
}
