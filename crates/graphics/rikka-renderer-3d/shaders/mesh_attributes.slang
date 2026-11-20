import rikka.common;

static const uint FIELD_NORMAL = 1u << 0;
static const uint FIELD_TANGENT = 1u << 1;
static const uint FIELD_UV = 1u << 2;
static const uint FIELD_COLOR = 1u << 3;

struct Push {
    RikkaAttributeRef target;
    uint* source;
    uint count;
    uint fields;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 id: SV_DispatchThreadID) {
    uint count = push.count;
    uint vertex = id.x;
    if (vertex >= count) {
        return;
    }
    uint fields = push.fields;
    uint* source = push.source;
    RikkaAttributeRef target = push.target;
    uint at = vertex;
    if ((fields & FIELD_NORMAL) != 0u) {
        target[vertex].normal = source[at];
        at += count;
    }
    if ((fields & FIELD_TANGENT) != 0u) {
        target[vertex].tangent = source[at];
        at += count;
    }
    if ((fields & FIELD_UV) != 0u) {
        target[vertex].uv = source[at];
        at += count;
    }
    if ((fields & FIELD_COLOR) != 0u) {
        target[vertex].color = source[at];
    }
}
