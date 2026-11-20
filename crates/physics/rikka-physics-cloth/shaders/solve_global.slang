import cloth.solve;
import cloth.solve.positions;

export struct Positions : IPositions = GlobalPositions;

[shader("compute")]
[numthreads(256, 1, 1)]
void main(uint3 group : SV_GroupID, uint3 local : SV_GroupThreadID) {
    solve(group.x, local.x);
}
