static const float3x3 IDENTITY3 = float3x3(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);

float3x3 diagonal3(float value) {
    return float3x3(value, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0, value);
}

float3x3 outer_product(float3 column, float3 row) {
    return spirv_asm { result:$$float3x3 = OpOuterProduct $column $row };
}

float3x3 scaled(float3x3 matrix, float scale) {
    return spirv_asm { result:$$float3x3 = OpMatrixTimesScalar $matrix $scale };
}

float4x4 scaled(float4x4 matrix, float scale) {
    return spirv_asm { result:$$float4x4 = OpMatrixTimesScalar $matrix $scale };
}
