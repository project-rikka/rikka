#version 330
#extension GL_ARB_separate_shader_objects : require

#include <minecraft:light.glsl>
#include <minecraft:fog.glsl>
#include <minecraft:dynamictransforms.glsl>
#include <minecraft:projection.glsl>
#include <minecraft:sample_lightmap.glsl>
#include <rikka:skin.glsl>

#ifndef OIT_ALPHA_ONLY
uniform sampler2D Sampler1;
uniform sampler2D Sampler2;

layout(location = 0) out float sphericalVertexDistance;
layout(location = 1) out float cylindricalVertexDistance;
#endif
layout(location = 2) out vec4 vertexColor;
#ifndef OIT_ALPHA_ONLY
layout(location = 3) out vec4 lightMapColor;
layout(location = 4) out vec4 overlayColor;
#endif

layout(location = 5) out vec2 texCoord0;
#ifdef GLINT
layout(location = 6) out vec2 texCoordGlint;
#endif

invariant gl_Position;

void main() {
    vec3 position = skin_position();
    vec2 uv = skin_uv();

    gl_Position = skin_clip_position(position);

    #ifndef OIT_ALPHA_ONLY
    sphericalVertexDistance = fog_spherical_distance(position);
    cylindricalVertexDistance = fog_cylindrical_distance(position);
    #endif
    vertexColor = minecraft_mix_light(Light0_Direction, Light1_Direction, skin_normal(), skin_color());
    #ifndef OIT_ALPHA_ONLY
    lightMapColor = sample_lightmap(Sampler2, skin_light());
    overlayColor = texelFetch(Sampler1, UV1, 0);
    #endif

    texCoord0 = uv;
    #ifdef GLINT
    texCoordGlint = (TextureMat * vec4(uv, 0.0, 1.0)).xy;
    #endif
}
