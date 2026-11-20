struct Instance {
    float4 bounds;
    float4 radii;
    float4 fill;
    float4 stroke;
    float4 uv;
    float2 turn;
    float stroke_width;
    float opacity;
    uint meta;
    uint tex;
    uint group;
    uint clip_group;
    uint shown;
    uint clip_index;
    uint cut_index;
    uint padding;
}

struct Push {
    float4 target;
    float4 glass_uv;
    float4 glass_bounds;
    float adapt;
    uint glass_tex;
}

[[vk::binding(0, 0)]] StructuredBuffer<Instance> instances;

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint NO_GLASS = 0xffffffffu;

static const uint KIND_MASK = 0x7u;
static const uint FLAGS_SHIFT = 3u;
static const uint FLAGS_MASK = 0x1FFFu;
static const uint COLOR_SHIFT = 16u;

static const uint KIND_SHAPE = 0u;
static const uint KIND_GLYPH_MASK = 1u;
static const uint KIND_TEXTURED = 2u;
static const uint KIND_BACKDROP = 3u;
static const uint KIND_GLYPH_COLOR = 4u;
static const uint KIND_GLYPH_CURVES = 5u;

static const uint FLAG_CIRCLE = 1u;
static const uint FLAG_CLIP = 2u;
static const uint FLAG_ARC = 4u;
static const uint FLAG_CUT = 8u;
static const uint FLAG_LENS = 16u;
static const uint FLAG_UV_SWAP = 32u;
static const uint FLAG_OUTSIDE_GROUP_CLIP = 64u;
static const uint FLAG_PIXELATED = 128u;
static const uint FLAG_TURNED = 256u;

struct Varyings {
    float4 position : SV_Position;
    [[vk::location(0)]] float4 pos : POSITION0;
    [[vk::location(1)]] float2 uv : TEXCOORD0;
    [[vk::location(2)]] nointerpolation uint4 what : WHAT;
    [[vk::location(3)]] nointerpolation float4 shape : SHAPE;
    [[vk::location(4)]] nointerpolation float4 radii : RADII;
    [[vk::location(5)]] nointerpolation float4 fill : FILL;
    [[vk::location(6)]] nointerpolation float4 stroke : STROKE;
    [[vk::location(7)]] nointerpolation float2 clip_shift : CLIP_SHIFT;
    [[vk::location(8)]] nointerpolation float4 clip : CLIP;
    [[vk::location(9)]] nointerpolation float4 clip_radii : CLIP_RADII;
}
