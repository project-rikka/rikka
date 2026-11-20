import glsl;
import fluid.settings;
static const float NEAR_SLACK = 1.0001;

int3 cell_of(float3 x, float inv_cell) {
    return int3(floor(x * inv_cell));
}

uint bin_of(int3 c, uint4 grid) {
    uint x = uint(c.x) & ((1u << grid.x) - 1u);
    uint y = uint(c.y) & ((1u << grid.y) - 1u);
    uint z = uint(c.z) & ((1u << grid.z) - 1u);
    return x | (y << grid.x) | (z << (grid.x + grid.y));
}

interface ICellVisitor {
    [mutating]
    void visit(uint first, uint last);
}

interface INeighbourVisitor {
    [mutating]
    void visit(uint j);
}

void for_cells<V : ICellVisitor>(Settings settings, ReadUints starts, float3 centre, inout V visitor) {
    float cell = settings.grid_cell.x;
    int3 home = cell_of(centre, settings.grid_cell.y);
    float3 inside = centre - float3(home) * cell;
    float3 below = inside * inside;
    float3 above = (cell - inside) * (cell - inside);
    float reach2 = kernel_h2(settings) * NEAR_SLACK;
    uint4 grid = settings.grid;
    uint mask_x = (1u << grid.x) - 1u;
    for (int dz = -1; dz <= 1; ++dz) {
        float gz = dz < 0 ? below.z : (dz > 0 ? above.z : 0.0);
        for (int dy = -1; dy <= 1; ++dy) {
            float gyz = gz + (dy < 0 ? below.y : (dy > 0 ? above.y : 0.0));
            if (gyz >= reach2) {
                continue;
            }
            int from = gyz + below.x < reach2 ? -1 : 0;
            uint span = uint((gyz + above.x < reach2 ? 1 : 0) - from);
            int3 row = home + int3(from, dy, dz);
            uint low = bin_of(row, grid);
            uint high = bin_of(row + int3(int(span), 0, 0), grid);
            if (high == low + span) {
                visitor.visit(starts[low], starts[high + 1u]);
            } else {
                uint row_end = low | mask_x;
                uint row_start = low & ~mask_x;
                visitor.visit(starts[low], starts[row_end + 1u]);
                visitor.visit(starts[row_start], starts[high + 1u]);
            }
        }
    }
}

struct NeighbourCells<V : INeighbourVisitor> : ICellVisitor {
    V neighbours;

    [mutating]
    void visit(uint first, uint last) {
        for (uint j = first; j < last; ++j) {
            neighbours.visit(j);
        }
    }
}

void for_neighbours<V : INeighbourVisitor>(Settings settings, ReadUints starts, float3 centre, inout V visitor) {
    NeighbourCells<V> cells = { visitor };
    for_cells(settings, starts, centre, cells);
    visitor = cells.neighbours;
}

void record_bin(Uints keys, Uints slots, Uints counts, uint p, bool active, uint key) {
    if (gl_SubgroupSize <= 32u) {
        uint lane = gl_SubgroupInvocationID;
        uint prev = subgroupShuffleUp(key, 1u);
        bool run_start = lane == 0u || key != prev;
        uint mask = subgroupBallot(run_start).x;
        uint leader = firstbithigh(mask & (0xffffffffu >> (31u - lane)));
        uint above = lane + 1u < gl_SubgroupSize ? (mask & (0xffffffffu << (lane + 1u))) : 0u;
        uint next_start = above != 0u ? firstbitlow(above) : gl_SubgroupSize;
        uint base = 0u;
        if (active && run_start) {
            InterlockedAdd(*counts.at(key), next_start - lane, base);
        }
        uint slot = subgroupShuffle(base, leader) + (lane - leader);
        if (active) {
            keys[p] = key;
            slots[p] = slot;
        }
    } else if (active) {
        keys[p] = key;
        uint slot;
        InterlockedAdd(*counts.at(key), 1u, slot);
        slots[p] = slot;
    }
}
