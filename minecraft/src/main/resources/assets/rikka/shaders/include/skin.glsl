#ifndef RIKKA_SKIN_GLSL
#define RIKKA_SKIN_GLSL

layout(location = 0) in vec3 Position;
layout(location = 1) in vec2 UV0;
layout(location = 2) in vec4 Normal;
layout(location = 3) in ivec4 Slots;
layout(location = 4) in vec4 Color;
layout(location = 5) in ivec2 UV1;
layout(location = 6) in ivec2 UV2;
layout(location = 7) in int PaletteBase;
layout(location = 8) in vec4 UvTransform;

layout(std140) uniform SkinPalette {
    vec4 SkinRows[1024];
};

vec3 skin_position() {
    int row = PaletteBase + Slots.x;
    vec4 local = vec4(Position, 1.0);
    return vec3(dot(SkinRows[row], local), dot(SkinRows[row + 1], local), dot(SkinRows[row + 2], local));
}

vec3 skin_face_normal() {
    int row = PaletteBase + Slots.x + 3;
    return vec3(dot(SkinRows[row].xyz, Normal.xyz), dot(SkinRows[row + 1].xyz, Normal.xyz), dot(SkinRows[row + 2].xyz, Normal.xyz));
}

vec3 skin_normal() {
    vec3 normal = skin_face_normal();
    if (SkinRows[PaletteBase + Slots.x + 3].w != 0.0) {
        normal = normalize(normal);
    }
    return trunc(clamp(normal, -1.0, 1.0) * 127.0) / 127.0;
}

vec4 skin_clip_position(vec3 position) {
#if defined(SKIN_BACK_FACES) || defined(SKIN_FRONT_FACES)
    vec3 normal = skin_face_normal();
    float facing = ProjMat[3][3] == 0.0 ? dot(normal, position) : -(mat3(ModelViewMat) * normal).z;
#ifdef SKIN_BACK_FACES
    if (facing < 0.0) {
#else
    if (facing >= 0.0) {
#endif
        return vec4(2.0, 2.0, 2.0, 1.0);
    }
#endif
    return ProjMat * ModelViewMat * vec4(position, 1.0);
}

vec4 skin_color() {
    return Slots.y < 0 ? Color : SkinRows[PaletteBase + Slots.y];
}

ivec2 skin_light() {
    return Slots.z == 0 ? UV2 : max((UV2 >> 4) & 15, ivec2(Slots.z)) << 4;
}

vec2 skin_uv() {
    return UvTransform.zw + UvTransform.xy * UV0;
}

#endif
