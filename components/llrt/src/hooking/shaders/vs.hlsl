#include "common.hlsli"

Interpolators main(uint vI : SV_VertexId)
{
    Interpolators output;

    float2 texcoord = float2((vI << 1) & 2, vI & 2);
    output.TexCoord = texcoord;
    output.Position = float4(texcoord.x * 2 - 1, -texcoord.y * 2 + 1, 0, 1);

    return output;
}
