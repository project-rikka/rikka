__exported import rikka.effect;

static const float RIKKA_ACESCCT_LINEAR_BREAK = 0.0078125;
static const float RIKKA_ACESCCT_LOG_BREAK = 0.155251141552511;
static const float RIKKA_ACESCCT_SLOPE = 10.5402377416545;
static const float RIKKA_ACESCCT_OFFSET = 0.0729055341958355;

float3 rikka_to_acescct(float3 value) {
    float3 logged = (log2(max(value, float3(1e-10))) + 9.72) / 17.52;
    float3 straight = RIKKA_ACESCCT_SLOPE * value + RIKKA_ACESCCT_OFFSET;
    return select(value <= RIKKA_ACESCCT_LINEAR_BREAK, straight, logged);
}

float3 rikka_from_acescct(float3 value) {
    float3 straight = (value - RIKKA_ACESCCT_OFFSET) / RIKKA_ACESCCT_SLOPE;
    float3 logged = exp2(value * 17.52 - 9.72);
    return select(value <= RIKKA_ACESCCT_LOG_BREAK, straight, logged);
}

static const float RIKKA_LOGC_CUT = 0.010591;
static const float RIKKA_LOGC_A = 5.555556;
static const float RIKKA_LOGC_B = 0.052272;
static const float RIKKA_LOGC_C = 0.247190;
static const float RIKKA_LOGC_D = 0.385537;
static const float RIKKA_LOGC_E = 5.367655;
static const float RIKKA_LOGC_F = 0.092809;

float3 rikka_to_logc(float3 value) {
    float3 logged = RIKKA_LOGC_C * log10(max(RIKKA_LOGC_A * value + RIKKA_LOGC_B, float3(1e-10))) + RIKKA_LOGC_D;
    float3 straight = RIKKA_LOGC_E * value + RIKKA_LOGC_F;
    return select(value > RIKKA_LOGC_CUT, logged, straight);
}

float3 rikka_from_logc(float3 value) {
    float3 logged = (pow(float3(10.0), (value - RIKKA_LOGC_D) / RIKKA_LOGC_C) - RIKKA_LOGC_B) / RIKKA_LOGC_A;
    float3 straight = (value - RIKKA_LOGC_F) / RIKKA_LOGC_E;
    return select(value > RIKKA_LOGC_E * RIKKA_LOGC_CUT + RIKKA_LOGC_F, logged, straight);
}
