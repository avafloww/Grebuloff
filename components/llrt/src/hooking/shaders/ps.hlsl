sampler TextureSampler : register(s0);
Texture2D<float4> Texture : register(t0);

float4 main(float2 texCoord : TEXCOORD0) : SV_Target0
{
    // get the texture width and height
    // float w;
    // float h;
    // Texture.GetDimensions(w, h);

    return Texture.Sample(TextureSampler, texCoord/* * float2(1.0f / w, 1.0f / h)*/);
}
