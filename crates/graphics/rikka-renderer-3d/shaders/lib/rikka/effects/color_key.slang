__exported import rikka.effect;

struct Parameters {
    [Default(0.75, 0.02, 0.03)] [Color] float3 key;
    [Options("isolate|replace|remove")] [Default(0)] uint mode;
    [Default(0.12)] [Range(0.0, 1.0)] float tolerance;
    [Default(0.08)] [Range(0.0, 1.0)] float softness;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
    [Default(0.02, 0.35, 0.8)] [Color] float3 replacement;
    [Default(0.5)] [Range(0.0, 1.0)] float spill;
};

static const uint RIKKA_KEY_REPLACE = 1u;
static const uint RIKKA_KEY_REMOVE = 2u;
static const float RIKKA_KEY_LIGHTNESS_WEIGHT = 0.35;

float rikka_key_matte(Parameters parameters, float3 lab, float3 key) {
    float3 difference = (lab - key) * float3(RIKKA_KEY_LIGHTNESS_WEIGHT, 1.0, 1.0);
    float distance = length(difference) / max(length(key.yz), 0.05);
    return smoothstep(parameters.tolerance, parameters.tolerance + max(parameters.softness, 1e-3), distance * 0.5);
}

float4 rikka_color_key(Parameters parameters, float4 color) {
    float3 encoded = rikka_effect_straight(color);
    float3 linear = rikka_decode_srgb(encoded);
    float3 lab = rikka_effect_to_oklab(max(linear, float3(0.0)));
    float3 key = rikka_effect_to_oklab(max(parameters.key, float3(0.0)));
    float outside = rikka_key_matte(parameters, lab, key);
    float amount = clamp(parameters.amount, 0.0, 1.0);
    float3 keyed;
    float alpha = color.a;
    if (parameters.mode == RIKKA_KEY_REPLACE) {
        float3 target = rikka_effect_to_oklab(max(parameters.replacement, float3(0.0)));
        float3 replaced = float3(lab.x + target.x - key.x, target.yz * (length(lab.yz) / max(length(key.yz), 1e-4)));
        keyed = lerp(rikka_effect_from_oklab(replaced), linear, outside);
    } else if (parameters.mode == RIKKA_KEY_REMOVE) {
        float2 direction = key.yz / max(length(key.yz), 1e-4);
        float along = max(dot(lab.yz, direction), 0.0);
        float3 suppressed = float3(lab.x, lab.yz - direction * along * parameters.spill * (1.0 - outside));
        keyed = rikka_effect_from_oklab(suppressed);
        alpha = color.a * lerp(1.0, outside, amount);
    } else {
        float grey = rikka_luma(linear);
        keyed = lerp(linear, float3(grey), outside);
    }
    float3 shown = rikka_effect_encode(lerp(linear, keyed, amount));
    return float4(shown * alpha, alpha);
}
