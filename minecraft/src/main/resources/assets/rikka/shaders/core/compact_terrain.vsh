#version 330
#extension GL_ARB_separate_shader_objects : require

#include <minecraft:fog.glsl>
#include <minecraft:globals.glsl>
#include <minecraft:projection.glsl>
#include <minecraft:sample_lightmap.glsl>
#include <minecraft:terrainglobals.glsl>

layout(location = 0) in uvec4 PositionLight;
layout(location = 1) in vec4 Color;
layout(location = 2) in uvec2 UV0;
layout(location = 3) in ivec3 ChunkPosition;
layout(location = 4) in float ChunkVisibility;

#ifndef OIT_ALPHA_ONLY
uniform sampler2D Sampler2;
#endif

layout(location = 0) out float sphericalVertexDistance;
layout(location = 1) out float cylindricalVertexDistance;
layout(location = 2) out vec4 vertexColor;
layout(location = 3) out vec2 texCoord0;
layout(location = 4) out float chunkVisibility;

const float POSITION_ORIGIN = 8.0;
const float POSITION_STEP = 1.0 / 2048.0;
const float UV_STEP = 1.0 / 32768.0;

void main() {
    vec3 position = vec3(PositionLight.xyz) * POSITION_STEP - POSITION_ORIGIN;
    vec3 pos = position + (ChunkPosition - CameraBlockPos) + CameraOffset;
    gl_Position = ProjMat * ModelViewMat * vec4(pos, 1.0);

    sphericalVertexDistance = fog_spherical_distance(pos);
    cylindricalVertexDistance = fog_cylindrical_distance(pos);
    #ifndef OIT_ALPHA_ONLY
    ivec2 light = ivec2(PositionLight.w & 0xFFu, PositionLight.w >> 8u);
    vertexColor = Color * sample_lightmap(Sampler2, light);
    #else
    vertexColor = Color;
    #endif
    texCoord0 = vec2(UV0) * UV_STEP;

    const float chunkFullyVisibleRange = 16.0;
    float dist = length(pos);
    chunkVisibility = mix(1.0, ChunkVisibility, clamp((dist - chunkFullyVisibleRange) / chunkFullyVisibleRange, 0.0, 1.0));
}
