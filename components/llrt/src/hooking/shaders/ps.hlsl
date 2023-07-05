#include "common.hlsli"

float4 main(Interpolators In) : SV_Target0
{
    return Texture.Sample(PointSampler, In.TexCoord);
}
