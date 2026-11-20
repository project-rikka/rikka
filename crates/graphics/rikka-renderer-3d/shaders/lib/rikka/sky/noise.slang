__exported import rikka.common;

uint rikka_hash3(uint3 v) {
    return rikka_hash(v.x ^ rikka_hash(v.y ^ rikka_hash(v.z)));
}

float rikka_lattice(int2 cell) {
    return rikka_unit(rikka_hash3(uint3(uint(cell.x), uint(cell.y), 0x9e37u)));
}

float rikka_value_noise(float2 p) {
    float2 cell = floor(p);
    float2 f = p - cell;
    float2 s = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    int2 c = int2(cell);
    float a = rikka_lattice(c);
    float b = rikka_lattice(c + int2(1, 0));
    float d = rikka_lattice(c + int2(0, 1));
    float e = rikka_lattice(c + int2(1, 1));
    return lerp(lerp(a, b, s.x), lerp(d, e, s.x), s.y);
}

float rikka_fbm(float2 p, int octaves) {
    const float2x2 turn = float2x2(1.6, -1.2, 1.2, 1.6);
    float sum = 0.0;
    float amplitude = 0.5;
    float total = 0.0;
    for (int i = 0; i < octaves; ++i) {
        sum += amplitude * rikka_value_noise(p);
        total += amplitude;
        p = mul(turn, p) + float2(17.3, -9.1);
        amplitude *= 0.5;
    }
    return sum / total;
}
