import fluid.settings;
import fluid.random;

static const uint BOND_PARTNER = 0x00ffffffu;
static const uint BOND_COLOUR_SHIFT = 24u;
static const uint UNCOLOURED = 0xffu;

uint bond_partner(uint word) {
    return word & BOND_PARTNER;
}

uint bond_colour(uint word) {
    return word >> BOND_COLOUR_SHIFT;
}

uint bond_word(uint partner, uint colour) {
    return partner | (colour << BOND_COLOUR_SHIFT);
}

uint3 bond_rank(uint i, uint j) {
    uint low = min(i, j);
    uint high = max(i, j);
    return uint3(hash32(low * 0x9e3779b9u ^ hash32(high)), low, high);
}

bool outranks(uint3 a, uint3 b) {
    if (a.x != b.x) {
        return a.x > b.x;
    }
    if (a.y != b.y) {
        return a.y > b.y;
    }
    return a.z > b.z;
}

struct Tally {
    uint colours;
    uint best;
    uint3 best_rank;
};

static const Tally NO_TALLY = Tally(0u, NONE, uint3(0u));

void tally_word(inout Tally tally, uint i, uint word) {
    uint colour = bond_colour(word);
    if (colour != UNCOLOURED) {
        tally.colours |= 1u << colour;
        return;
    }
    uint j = bond_partner(word);
    uint3 rank = bond_rank(i, j);
    if (tally.best == NONE || outranks(rank, tally.best_rank)) {
        tally.best = j;
        tally.best_rank = rank;
    }
}
