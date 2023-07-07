#include "common.hlsli"

float4 main(Interpolators In) : SV_Target0
{
    float4 col = Texture.Sample(PointSampler, In.TexCoord);

    // Electron for Windows uses RGBA, and even though we can specify the texture as BGRA, it still
    // seems to perform faster when we swizzle the colour here instead of changing the D3D11_TEXTURE2D_DESC.
    return float4(col.b, col.g, col.r, col.a);
}
