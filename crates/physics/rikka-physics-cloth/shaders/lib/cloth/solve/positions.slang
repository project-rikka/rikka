__exported import cloth.solve.state;

interface IPositions {
    static float3 position(uint i);
    static void place(uint i, float3 p);
    static void load();
    static void store(uint i, float3 x);
}

struct CachedPositions<let N : int> : IPositions {
    static groupshared float cached_x[N];
    static groupshared float cached_y[N];
    static groupshared float cached_z[N];

    static float3 position(uint i) {
        uint at = i - first;
        return float3(
            shared_load(__getAddress(cached_x[at])),
            shared_load(__getAddress(cached_y[at])),
            shared_load(__getAddress(cached_z[at])));
    }

    static void place(uint i, float3 p) {
        uint at = i - first;
        shared_store(__getAddress(cached_x[at]), p.x);
        shared_store(__getAddress(cached_y[at]), p.y);
        shared_store(__getAddress(cached_z[at]), p.z);
    }

    static void load() {
        for (uint i = first + lane; i < first + count; i += THREADS) {
            place(i, job.positions[i].xyz);
        }
    }

    static void store(uint i, float3 x) {
        job.positions[i] = float4(x, 0.0);
    }
}

struct GlobalPositions : IPositions {
    static float3 position(uint i) {
        return job.positions[i].xyz;
    }

    static void place(uint i, float3 p) {
        job.positions[i] = float4(p, 0.0);
    }

    static void load() {}

    static void store(uint i, float3 x) {}
}

extern struct Positions : IPositions;

float3 position(uint i) {
    return Positions.position(i);
}

void place(uint i, float3 p) {
    Positions.place(i, p);
}

float3 cloth_point(uint figure, uint particle, float alpha) {
    bool own = figure == job.shapes_figure.z;
    if (own && mine(particle)) {
        return position(particle);
    }
    if (own && particle < job.colours.w) {
        return lerp(job.epoch_start[particle].xyz, job.positions[particle].xyz, alpha);
    }
    ReadVec4s start = own ? job.epoch_start : job.figures[figure].epoch_start;
    ReadVec4s motion = own ? job.epoch_motion : job.figures[figure].epoch_motion;
    return start[particle].xyz + motion[particle].xyz * (alpha * job.step.w);
}

void cloth_triangle(uint id, float alpha, out float3 a, out float3 b, out float3 c, out uint4 corners) {
    uint figure = obstacle_figure(id);
    corners = figure == job.shapes_figure.z
        ? job.triangles[obstacle_index(id)]
        : job.figures[figure].triangles[obstacle_index(id)];
    a = cloth_point(figure, corners.x, alpha);
    b = cloth_point(figure, corners.y, alpha);
    c = cloth_point(figure, corners.z, alpha);
}

float snug_thickness(uint i, uint4 corners, float thick) {
    Snug snug = snug_layer(job.rest, corners, job.rest[i].xyz, thick);
    return snug.in_range ? min(thick, snug.apart) : thick;
}

bool inner_sheet(uint entry, uint inner) {
    uint idx = obstacle_index(entry);
    return obstacle_figure(entry) == job.shapes_figure.z
        && idx < inner
        && job.pieces[job.triangles[idx].w].contact.x > 0.5;
}

float drawn_cover(uint id, uint4 corners, Gap g, float3 a, float3 b, float3 c) {
    uint figure = obstacle_figure(id);
    bool own = figure == job.shapes_figure.z;
    if ((own && mine(corners.x)) || dot(g.normal, cross(b - a, c - a)) <= 0.0) {
        return 0.0;
    }
    ReadVec4s rest = own ? job.rest : job.figures[figure].rest;
    return dot(g.weights, float3(relief_above(rest[corners.x]), relief_above(rest[corners.y]), relief_above(rest[corners.z])));
}
