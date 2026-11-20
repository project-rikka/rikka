import rikka.effect;

struct Parameters {
    [Default(0.6)] [Range(0.0, 1.0)] float distance;
    [Default(1.0)] [Range(0.0, 1.0)] float crop;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float2 panini_project(float2 view, float d) {
    float distance = 1.0 + d;
    float hypotenuse = view.x * view.x + distance * distance;
    float intersect = view.x * d;
    float discriminant = hypotenuse - intersect * intersect;
    float cylinder = (-intersect * view.x + distance * sqrt(max(discriminant, 0.0))) / hypotenuse + d;
    float2 position = view * (cylinder / distance);
    return position / (cylinder - d);
}

float2 panini_crop(float2 extents, float d) {
    float distance = 1.0 + d;
    float hypotenuse = sqrt(extents.x * extents.x + 1.0);
    float cylinder_minus = 1.0 / hypotenuse;
    float cylinder = cylinder_minus + d;
    return extents * cylinder_minus * (distance / cylinder);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void project(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4x4 projection = push.frame.view.projection;
    float2 extents = float2(1.0 / abs(projection[0][0]), 1.0 / abs(projection[1][1]));
    float d = clamp(parameters.distance, 0.0, 1.0);
    float2 cropped = panini_crop(extents, d);
    float fit = min(cropped.x / extents.x, cropped.y / extents.y);
    float scale = lerp(1.0, clamp(fit, 0.0, 1.0), clamp(parameters.crop, 0.0, 1.0));
    float2 uv = rikka_effect_uv(pixel, push.size);
    float2 view = (uv * 2.0 - 1.0) * extents * scale;
    float2 projected = panini_project(view, d) / extents;
    float2 source = projected * 0.5 + 0.5;
    bool inside = all(source >= 0.0) && all(source <= 1.0);
    float4 color = inside ? rikka_effect_sample(push.source, source) : float4(0.0);
    rikka_effect_store(push.target, pixel, color);
}
