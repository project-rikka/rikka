import cloth.common;
import cloth.world;

struct Trait : INonWritable {
    float4 a;
    float4 b;
    float4 c;
    uint4 d;

    static Trait load_from<let ALIGN : int>(Trait* address) {
        return {
            load_non_writable<16>(__getAddress(address->a)),
            load_non_writable<16>(__getAddress(address->b)),
            load_non_writable<16>(__getAddress(address->c)),
            load_non_writable<16>(__getAddress(address->d)),
        };
    }
};

typealias Traits = NonWritableBuffer<Trait, 16>;

struct Edge : INonWritable {
    uint4 ends;
    float4 params;

    static Edge load_from<let ALIGN : int>(Edge* address) {
        return {
            load_non_writable<16>(__getAddress(address->ends)),
            load_non_writable<16>(__getAddress(address->params)),
        };
    }
};

typealias Edges = NonWritableBuffer<Edge, 16>;

struct Hinge : INonWritable {
    uint4 ids;
    float4 params;
    float4 weights;
    float4 applied;
    uint4 dests;

    static Hinge load_from<let ALIGN : int>(Hinge* address) {
        return {
            load_non_writable<16>(__getAddress(address->ids)),
            load_non_writable<16>(__getAddress(address->params)),
            load_non_writable<16>(__getAddress(address->weights)),
            load_non_writable<16>(__getAddress(address->applied)),
            load_non_writable<16>(__getAddress(address->dests)),
        };
    }
};

typealias Hinges = NonWritableBuffer<Hinge, 16>;

struct Tether : INonWritable {
    uint anchor;
    float longest;
    float shortest;
    uint padding;

    static Tether load_from<let ALIGN : int>(Tether* address) {
        return {
            load_non_writable<16>(__getAddress(address->anchor)),
            load_non_writable<4>(__getAddress(address->longest)),
            load_non_writable<8>(__getAddress(address->shortest)),
            load_non_writable<4>(__getAddress(address->padding)),
        };
    }
};

typealias Tethers = NonWritableBuffer<Tether, 16>;

struct Piece : INonWritable {
    float4 centre;
    float4 scaled;
    float4 turn;
    float4 fixed_shift;
    float4 spin;
    float4 response;
    float4 flutter;
    float4 contact;
    float4 settling;
    uint4 layer_triangles;
    float4 grab;

    static Piece load_from<let ALIGN : int>(Piece* address) {
        return {
            load_non_writable<16>(__getAddress(address->centre)),
            load_non_writable<16>(__getAddress(address->scaled)),
            load_non_writable<16>(__getAddress(address->turn)),
            load_non_writable<16>(__getAddress(address->fixed_shift)),
            load_non_writable<16>(__getAddress(address->spin)),
            load_non_writable<16>(__getAddress(address->response)),
            load_non_writable<16>(__getAddress(address->flutter)),
            load_non_writable<16>(__getAddress(address->contact)),
            load_non_writable<16>(__getAddress(address->settling)),
            load_non_writable<16>(__getAddress(address->layer_triangles)),
            load_non_writable<16>(__getAddress(address->grab)),
        };
    }
};

typealias Pieces = NonWritableBuffer<Piece, 16>;

struct Job : INonWritable {
    Exchanged4s positions;
    Exchanged4s velocities;
    Vec4s drift;
    Exchanged4s starts;
    Vec4s preds;
    ReadVec4s epoch_start;
    ExchangedUvec4s targets_from;
    ExchangedUvec4s targets_to;
    ReadVec4s rest;
    Traits traits;
    Edges edges;
    Hinges bends;
    Hinges shapes;
    ReadUints tether_starts;
    Tethers tethers;
    ReadUints plan;
    ReadUvec4s triangles;
    ReadUints incident_starts;
    ReadUints incident;
    ReadVec4s normals;
    Pieces pieces;
    Uints stats;
    Exchanged4s slots;
    ReadUints gather_starts;
    ReadUvec4s wrap_dests;
    Vec4s geometry;
    ExchangedUints contacts;
    Uints wraps;
    Exchanged4s selfs;
    Figures figures;
    ReadVec4s body_from;
    ReadVec4s body_to;
    ReadUvec4s body_triangles;
    Colliders colliders_from;
    Colliders colliders_to;
    ReadVec4s planes_from;
    ReadVec4s planes_to;
    ReadUints cells;
    ReadUints items;
    ReadVec4s spheres;
    ReadUints large;
    Uints nearest;
    ReadVec4s epoch_motion;
    ReadUvec4s grabbed;
    ReadVec4s grabs_from;
    ReadVec4s grabs_to;
    uint4 range;
    uint4 colours;
    uint4 items_range;
    uint4 shapes_figure;
    uint4 steps;
    uint4 flags;
    float4 step;
    float4 gravity;
    float4 wind;
    float4 rest_terms;

    static Job load_from<let ALIGN : int>(Job* address) {
        return {
            load_non_writable<8>(__getAddress(address->positions)),
            load_non_writable<8>(__getAddress(address->velocities)),
            load_non_writable<8>(__getAddress(address->drift)),
            load_non_writable<8>(__getAddress(address->starts)),
            load_non_writable<8>(__getAddress(address->preds)),
            load_non_writable<8>(__getAddress(address->epoch_start)),
            load_non_writable<8>(__getAddress(address->targets_from)),
            load_non_writable<8>(__getAddress(address->targets_to)),
            load_non_writable<8>(__getAddress(address->rest)),
            load_non_writable<8>(__getAddress(address->traits)),
            load_non_writable<8>(__getAddress(address->edges)),
            load_non_writable<8>(__getAddress(address->bends)),
            load_non_writable<8>(__getAddress(address->shapes)),
            load_non_writable<8>(__getAddress(address->tether_starts)),
            load_non_writable<8>(__getAddress(address->tethers)),
            load_non_writable<8>(__getAddress(address->plan)),
            load_non_writable<8>(__getAddress(address->triangles)),
            load_non_writable<8>(__getAddress(address->incident_starts)),
            load_non_writable<8>(__getAddress(address->incident)),
            load_non_writable<8>(__getAddress(address->normals)),
            load_non_writable<8>(__getAddress(address->pieces)),
            load_non_writable<8>(__getAddress(address->stats)),
            load_non_writable<8>(__getAddress(address->slots)),
            load_non_writable<8>(__getAddress(address->gather_starts)),
            load_non_writable<8>(__getAddress(address->wrap_dests)),
            load_non_writable<8>(__getAddress(address->geometry)),
            load_non_writable<8>(__getAddress(address->contacts)),
            load_non_writable<8>(__getAddress(address->wraps)),
            load_non_writable<8>(__getAddress(address->selfs)),
            load_non_writable<8>(__getAddress(address->figures)),
            load_non_writable<8>(__getAddress(address->body_from)),
            load_non_writable<8>(__getAddress(address->body_to)),
            load_non_writable<8>(__getAddress(address->body_triangles)),
            load_non_writable<8>(__getAddress(address->colliders_from)),
            load_non_writable<8>(__getAddress(address->colliders_to)),
            load_non_writable<8>(__getAddress(address->planes_from)),
            load_non_writable<8>(__getAddress(address->planes_to)),
            load_non_writable<8>(__getAddress(address->cells)),
            load_non_writable<8>(__getAddress(address->items)),
            load_non_writable<8>(__getAddress(address->spheres)),
            load_non_writable<8>(__getAddress(address->large)),
            load_non_writable<8>(__getAddress(address->nearest)),
            load_non_writable<8>(__getAddress(address->epoch_motion)),
            load_non_writable<8>(__getAddress(address->grabbed)),
            load_non_writable<8>(__getAddress(address->grabs_from)),
            load_non_writable<8>(__getAddress(address->grabs_to)),
            load_non_writable<16>(__getAddress(address->range)),
            load_non_writable<16>(__getAddress(address->colours)),
            load_non_writable<16>(__getAddress(address->items_range)),
            load_non_writable<16>(__getAddress(address->shapes_figure)),
            load_non_writable<16>(__getAddress(address->steps)),
            load_non_writable<16>(__getAddress(address->flags)),
            load_non_writable<16>(__getAddress(address->step)),
            load_non_writable<16>(__getAddress(address->gravity)),
            load_non_writable<16>(__getAddress(address->wind)),
            load_non_writable<16>(__getAddress(address->rest_terms)),
        };
    }
};

typealias Jobs = NonWritableBuffer<Job, 16>;

float3 skinned_body(Job job, uint vertex, float alpha) {
    return lerp(job.body_from[vertex].xyz, job.body_to[vertex].xyz, alpha);
}

void body_corners(Figures figures, uint figure, uint triangle, float alpha, out float3 a, out float3 b, out float3 c) {
    ReadVec4s from = figures[figure].body_from;
    ReadVec4s to = figures[figure].body_to;
    uint4 corners = figures[figure].body_triangles[triangle];
    a = lerp(from[corners.x].xyz, to[corners.x].xyz, alpha);
    b = lerp(from[corners.y].xyz, to[corners.y].xyz, alpha);
    c = lerp(from[corners.z].xyz, to[corners.z].xyz, alpha);
}

void body_endpoints(Figures figures, uint figure, uint triangle, out float3 a0, out float3 b0, out float3 c0, out float3 a1, out float3 b1, out float3 c1) {
    Figure body = figures[figure];
    uint4 corners = body.body_triangles[triangle];
    a0 = body.body_from[corners.x].xyz;
    b0 = body.body_from[corners.y].xyz;
    c0 = body.body_from[corners.z].xyz;
    a1 = body.body_to[corners.x].xyz;
    b1 = body.body_to[corners.y].xyz;
    c1 = body.body_to[corners.z].xyz;
}
