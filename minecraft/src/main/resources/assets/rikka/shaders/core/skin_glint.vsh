#version 330
#extension GL_ARB_separate_shader_objects : require

#include <minecraft:fog.glsl>
#include <minecraft:dynamictransforms.glsl>
#include <minecraft:projection.glsl>
#include <rikka:skin.glsl>

layout(location = 0) out float sphericalVertexDistance;
layout(location = 1) out float cylindricalVertexDistance;
layout(location = 2) out vec2 texCoord0;

invariant gl_Position;

void main() {
    vec3 position = skin_position();

    gl_Position = skin_clip_position(position);

    sphericalVertexDistance = fog_spherical_distance(position);
    cylindricalVertexDistance = fog_cylindrical_distance(position);
    texCoord0 = (TextureMat * vec4(skin_uv(), 0.0, 1.0)).xy;
}
