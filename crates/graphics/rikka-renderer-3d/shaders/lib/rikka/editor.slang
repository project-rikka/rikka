__exported import rikka.common;

static const uint RIKKA_CLIP_PLANES = 6u;
static const uint RIKKA_STUDIO_LIGHTS = 4u;
static const uint RIKKA_OUTLINE_OBJECT_MASK = 0x0fffffffu;
static const uint RIKKA_OUTLINE_COLOR_SHIFT = 28u;

[vk::constant_id(2)] const bool RIKKA_CLIPPING = false;

struct RikkaStudioLight {
    float4 direction;
    float4 diffuse;
    float4 specular;
};

struct RikkaGrid {
    float steps[8];
    float2 offset;
    float clip;
    float level;
    uint lines;
    uint grid_flags;
    uint axis_flags;
    float pixel_fac;
    uint axes[4];
    uint colors[5];
    float far_clip;
    float width;
    uint pad;
};

struct RikkaEditor {
    float4 clip[6];
    float4 ambient;
    RikkaStudioLight lights[4];
    float4 shadow;
    uint studio;
    uint shadowed;
    float shadow_shift;
    float shadow_focus;
    RikkaGrid grid;
};

typealias RikkaEditorRef = RikkaEditor*;

RikkaEditorRef rikka_editor(RikkaFrameRef frame) {
    return (RikkaEditorRef)frame.editor;
}

bool rikka_clipped(RikkaFrameRef frame, float3 world) {
    if (!RIKKA_CLIPPING) {
        return false;
    }
    RikkaEditorRef editor = rikka_editor(frame);
    for (uint plane = 0u; plane < RIKKA_CLIP_PLANES; plane++) {
        float4 clip = editor.clip[plane];
        if (dot(clip.xyz, world) + clip.w < 0.0) {
            return true;
        }
    }
    return false;
}

void rikka_clip(RikkaFrameRef frame, float3 world) {
    if (rikka_clipped(frame, world)) {
        discard;
    }
}
