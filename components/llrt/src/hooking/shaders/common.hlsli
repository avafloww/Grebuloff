SamplerState PointSampler : register(s0);
Texture2D<float4> Texture : register(t0);

struct Interpolators
{
    float4 Position : SV_Position;
    float2 TexCoord : TEXCOORD0;
};
