import fluid.settings;

static const uint MAX_NEIGHBOURS = 48u;
static const uint NEIGHBOUR_INDEX = 0x00ffffffu;
static const uint COMPACT = 0x80000000u;
static const uint MAX_CANDIDATES = 8u;

interface IListedVisitor {
    [mutating]
    void visit(uint slot, uint j);
}

void for_listed<V : IListedVisitor>(ReadUints list, ReadUints counts, uint stride, uint i, inout V visitor) {
    uint listed = counts[i];
    uint total = listed & ~COMPACT;
    uint cursor = i;
    if ((listed & COMPACT) != 0u) {
        uint word = 0u;
        for (uint slot = 0u; slot < total; ++slot) {
            int offset;
            if ((slot & 1u) == 0u) {
                word = list[cursor];
                cursor += stride;
                offset = bitfieldExtract(int(word), 0, 16);
            } else {
                offset = int(word) >> 16;
            }
            visitor.visit(slot, uint(int(i) + offset));
        }
    } else {
        for (uint slot = 0u; slot < total; ++slot) {
            uint j = list[cursor];
            cursor += stride;
            visitor.visit(slot, j);
        }
    }
}
