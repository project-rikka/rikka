uint hash32(uint x) {
    x ^= x >> 16;
    x *= 0x7feb352du;
    x ^= x >> 15;
    x *= 0x846ca68bu;
    x ^= x >> 16;
    return x;
}

float unit_random(uint seed) {
    return float(hash32(seed) >> 8) * (1.0 / 16777216.0);
}
