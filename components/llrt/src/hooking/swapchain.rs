use crate::{hooking::create_function_hook, webview::WebView};
use anyhow::Result;
use ffxiv_client_structs::generated::ffxiv::client::graphics::kernel::{
    Device, Device_Fn_Instance,
};
use grebuloff_macros::{function_hook, vtable_functions, VTable};
use log::{debug, trace, info};
use std::{
    cell::{OnceCell, RefCell},
    mem::MaybeUninit,
    ptr::addr_of_mut,
};
use windows::{
    s,
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::{
            Direct3D::{D3D11_SRV_DIMENSION_TEXTURE2D, D3D_PRIMITIVE_TOPOLOGY, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP},
            Direct3D11::*,
            Dxgi::{
                Common::{DXGI_FORMAT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_FORMAT_R16_UINT},
                IDXGISwapChain, DXGI_SWAP_CHAIN_DESC,
            },
        },
    },
};

thread_local! {
    static RENDER_DATA: RefCell<OnceCell<RenderData>> = RefCell::new(OnceCell::new());
}

#[derive(VTable)]
struct ResolvedSwapChain {
    #[vtable_base]
    base: *mut *mut IDXGISwapChain,
}

vtable_functions!(impl ResolvedSwapChain {
    #[vtable_fn(8)]
    unsafe fn present(&self, sync_interval: u32, present_flags: u32);

    #[vtable_fn(13)]
    unsafe fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: u32,
        swap_chain_flags: u32,
    );
});

unsafe fn resolve_swap_chain() -> ResolvedSwapChain {
    debug!("resolving swap chain");
    let device = loop {
        let device = ffxiv_client_structs::address::get::<Device_Fn_Instance>() as *mut Device;

        if device.is_null() {
            trace!("device is null, waiting");
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        break device;
    };

    debug!("device: {:p}", device);
    let swap_chain = (*device).swap_chain;
    debug!("swap chain: {:p}", swap_chain);
    let dxgi_swap_chain = (*swap_chain).dxgiswap_chain as *mut *mut *mut IDXGISwapChain;
    debug!("dxgi swap chain: {:p}", *dxgi_swap_chain);

    ResolvedSwapChain {
        base: *dxgi_swap_chain,
    }
}

pub unsafe fn hook_swap_chain() -> Result<()> {
    let resolved = resolve_swap_chain();

    create_function_hook!(present, *resolved.address_table().present()).enable()?;

    Ok(())
}

/// Stores data that is used for rendering our UI overlay.
struct RenderData {
    /// Used to sanity-check that we're rendering into the correct context.
    sc_addr: *const IDXGISwapChain,
    /// The render target view for the swap chain's back buffer.
    rtv: ID3D11RenderTargetView,
    /// The texture we render into.
    texture: ID3D11Texture2D,
    srv: ID3D11ShaderResourceView,
    pixel_shader: ID3D11PixelShader,
    vertex_shader: ID3D11VertexShader,
    sampler: ID3D11SamplerState,
    blend_state: ID3D11BlendState,
    depth_stencil_state: ID3D11DepthStencilState,
    rasterizer_state: ID3D11RasterizerState,
    viewport: D3D11_VIEWPORT,
    scissor_rect: RECT,
    last_frame_buffer: Box<[u8]>,
    buffer_width: u32,
    buffer_height: u32,
    window_handle: HWND,
}

#[repr(C, packed)]
struct DrawVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    colour: u32,
}
#[repr(C, packed)]
struct DrawIndex(u16);

#[function_hook]
unsafe extern "stdcall" fn present(
    this: IDXGISwapChain,
    sync_interval: u32,
    present_flags: u32,
) -> i32 {
    let device: ID3D11Device2 = this.GetDevice().unwrap();

    RENDER_DATA.with(move |cell| {
        let mut cell = cell.borrow_mut();
        let data = match cell.get_mut() {
            Some(data) => {
                // ensure we're rendering into the correct context
                if data.sc_addr != &this as *const _ {
                    debug!("IDXGISwapChain::Present called with a different swap chain than before, executing original function");
                    return original.call(this, sync_interval, present_flags);
                }

                data
            },
            None => {
                // initialize our render data for this thread (the render thread)
                trace!("initializing RenderData on IDXGISwapChain::Present");

                let sc_desc = {
                    let mut sc_desc = MaybeUninit::<DXGI_SWAP_CHAIN_DESC>::zeroed();
                    this.GetDesc(sc_desc.as_mut_ptr()).expect("failed to get DXGI_SWAP_CHAIN_DESC");
                    sc_desc.assume_init()
                };

                let texture = {
                    let texture_desc = D3D11_TEXTURE2D_DESC {
                        Width: sc_desc.BufferDesc.Width,
                        Height: sc_desc.BufferDesc.Height,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DYNAMIC,
                        BindFlags: D3D11_BIND_SHADER_RESOURCE,
                        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE,
                        ..Default::default()
                    };

                    let mut tex = MaybeUninit::<Option<ID3D11Texture2D>>::zeroed();
                    device.CreateTexture2D(&texture_desc, None, Some(tex.as_mut_ptr())).expect("CreateTexture2D failed");
                    tex.assume_init().expect("CreateTexture2D returned null")
                };

                // [DEBUG] back buffer desc: D3D11_TEXTURE2D_DESC { Width: 1920, Height: 1080, MipLevels: 1, ArraySize: 1, Format: DXGI_FORMAT(28), SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Usage: D3D11_USAGE(0), BindFlags: D3D11_BIND_FLAG(32), CPUAccessFlags: D3D11_CPU_ACCESS_FLAG(0), MiscFlags: D3D11_RESOURCE_MISC_FLAG(0) }
                // back buffer: D3D11_USAGE_DEFAULT, DXGI_FORMAT_R8G8B8A8_UNORM
                // let texture: ID3D11Texture2D = this.GetBuffer(0).expect("failed to get back buffer");
                // let texture_desc = {
                //     let mut texture_desc = MaybeUninit::<D3D11_TEXTURE2D_DESC>::zeroed();
                //     texture.GetDesc(texture_desc.as_mut_ptr());
                //     texture_desc.assume_init()
                // };

                // create the shader resource view
                let srv = {
                    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                        Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D11_TEX2D_SRV {
                                MostDetailedMip: 0,
                                MipLevels: 1,
                            },
                        },
                    };

                    let mut srv = MaybeUninit::<Option<ID3D11ShaderResourceView>>::zeroed();
                    device.CreateShaderResourceView(&texture, Some(&srv_desc), Some(srv.as_mut_ptr())).expect("CreateShaderResourceView failed");
                    srv.assume_init().expect("CreateShaderResourceView returned null")
                };
                
                // create the pixel shader
                let pixel_shader = {
                    let ps_bytecode = include_bytes!("shaders/ps.cso");
                    let mut ps = MaybeUninit::<Option<ID3D11PixelShader>>::zeroed();
                    device.CreatePixelShader(ps_bytecode, None, Some(ps.as_mut_ptr())).expect("CreatePixelShader failed");
                    ps.assume_init().expect("CreatePixelShader returned null")
                };

                // create the vertex shader
                let vertex_shader = {
                    let vs_bytecode = include_bytes!("shaders/vs.cso");
                    let mut vs = MaybeUninit::<Option<ID3D11VertexShader>>::zeroed();
                    device.CreateVertexShader(vs_bytecode, None, Some(vs.as_mut_ptr())).expect("CreateVertexShader failed");
                    vs.assume_init().expect("CreateVertexShader returned null")
                };

                // create the linear clamp sampler
                let sampler = {
                    let sampler_desc = D3D11_SAMPLER_DESC {
                        Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
                        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                        ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                        MinLOD: 0.0,
                        MaxLOD: 1.0,
                        MipLODBias: 0.0,
                        MaxAnisotropy: 0,
                        BorderColor: [0.0; 4],
                    };

                    let mut sampler = MaybeUninit::<Option<ID3D11SamplerState>>::zeroed();
                    device.CreateSamplerState(&sampler_desc, Some(sampler.as_mut_ptr())).expect("CreateSamplerState failed");
                    sampler.assume_init().expect("CreateSamplerState returned null")
                };

                // create alpha blend state
                let blend_state = {
                    let blend_desc = D3D11_BLEND_DESC {
                        AlphaToCoverageEnable: false.into(),
                        RenderTarget: [
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                BlendEnable: true.into(),
                                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                                BlendOp: D3D11_BLEND_OP_ADD,
                                SrcBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                                DestBlendAlpha: D3D11_BLEND_ZERO,
                                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                            D3D11_RENDER_TARGET_BLEND_DESC {
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    };

                    let mut blend_state = MaybeUninit::<Option<ID3D11BlendState>>::zeroed();
                    device.CreateBlendState(&blend_desc, Some(blend_state.as_mut_ptr())).expect("CreateBlendState failed");
                    blend_state.assume_init().expect("CreateBlendState returned null")
                };

                // create cull none rasterizer state
                let rasterizer_state = {
                    let rasterizer_desc = D3D11_RASTERIZER_DESC {
                        FillMode: D3D11_FILL_SOLID,
                        CullMode: D3D11_CULL_NONE,
                        DepthClipEnable: false.into(),
                        ScissorEnable: true.into(),
                        ..Default::default()
                    };

                    let mut rasterizer_state = MaybeUninit::<Option<ID3D11RasterizerState>>::zeroed();
                    device.CreateRasterizerState(&rasterizer_desc, Some(rasterizer_state.as_mut_ptr())).expect("CreateRasterizerState failed");
                    rasterizer_state.assume_init().expect("CreateRasterizerState returned null")
                };

                // create depth stencil state with no depth
                let depth_stencil_state = {
                    let depth_stencil_desc = D3D11_DEPTH_STENCIL_DESC {
                        DepthEnable: false.into(),
                        DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                        DepthFunc: D3D11_COMPARISON_ALWAYS,
                        StencilEnable: false.into(),
                        FrontFace: D3D11_DEPTH_STENCILOP_DESC {
                            StencilFailOp: D3D11_STENCIL_OP_KEEP,
                            StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
                            StencilPassOp: D3D11_STENCIL_OP_KEEP,
                            StencilFunc: D3D11_COMPARISON_ALWAYS,
                        },
                        BackFace: D3D11_DEPTH_STENCILOP_DESC {
                            StencilFailOp: D3D11_STENCIL_OP_KEEP,
                            StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
                            StencilPassOp: D3D11_STENCIL_OP_KEEP,
                            StencilFunc: D3D11_COMPARISON_ALWAYS,
                        },
                        ..Default::default()
                    };

                    let mut depth_stencil_state = MaybeUninit::<Option<ID3D11DepthStencilState>>::zeroed();
                    device.CreateDepthStencilState(&depth_stencil_desc, Some(depth_stencil_state.as_mut_ptr())).expect("CreateDepthStencilState failed");
                    depth_stencil_state.assume_init().expect("CreateDepthStencilState returned null")
                };

                // viewport
                let viewport = D3D11_VIEWPORT {
                    TopLeftX: 0.0,
                    TopLeftY: 0.0,
                    Width: sc_desc.BufferDesc.Width as f32,
                    Height: sc_desc.BufferDesc.Height as f32,
                    MinDepth: 0.0,
                    MaxDepth: 1.0,
                };

                // scissor rect
                let scissor_rect = RECT {
                    left: 0,
                    top: 0,
                    right: sc_desc.BufferDesc.Width as i32,
                    bottom: sc_desc.BufferDesc.Height as i32,
                };


                // let vertex_binding = D3D11_VERTEX_BUFFER_VIEW_DESC {
                //     BufferLocation: vertex_buffer.as_ptr() as _,
                //     StrideInBytes: std::mem::size_of::<DrawVertex>() as u32,
                //     SizeInBytes: std::mem::size_of::<DrawVertex>() as u32 * 6,
                // };

                // create the input layout
                // let input_layout = {
                //     let input_element_descs = [
                //         D3D11_INPUT_ELEMENT_DESC {
                //             SemanticName: s!("POSITION"),
                //             SemanticIndex: 0,
                //             Format: DXGI_FORMAT_R32G32_FLOAT,
                //             InputSlot: 0,
                //             AlignedByteOffset: 0,
                //             InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                //             InstanceDataStepRate: 0,
                //         },
                //         D3D11_INPUT_ELEMENT_DESC {
                //             SemanticName: s!("TEXCOORD"),
                //             SemanticIndex: 0,
                //             Format: DXGI_FORMAT_R32G32_FLOAT,
                //             InputSlot: 0,
                //             AlignedByteOffset: 8,
                //             InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                //             InstanceDataStepRate: 0,
                //         },
                //         D3D11_INPUT_ELEMENT_DESC {
                //             SemanticName: s!("COLOR"),
                //             SemanticIndex: 0,
                //             Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                //             InputSlot: 0,
                //             AlignedByteOffset: 8,
                //             InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                //             InstanceDataStepRate: 0,
                //         },
                //     ];

                //     let mut input_layout = MaybeUninit::<Option<ID3D11InputLayout>>::zeroed();
                //     device.CreateInputLayout(
                //         &input_element_descs,
                //         include_bytes!("shaders/vs.cso"),
                //         Some(input_layout.as_mut_ptr()),
                //     ).expect("CreateInputLayout failed");
                //     input_layout.assume_init().expect("CreateInputLayout returned null")
                // };
                // init render target view
                let rtv = {
                    let back_buffer: ID3D11Texture2D = this.GetBuffer(0).expect("failed to get back buffer");
                    let mut rtv = None;

                    device
                        .CreateRenderTargetView(&back_buffer, None, Some(&mut rtv))
                        .expect("failed to create render target view (CreateRenderTargetView not ok)");

                    rtv.expect("failed to create render target view (was null)")
                };
                
                // // create vertex buffer
                // let vertex_buffer = {
                //     let buffer_desc = D3D11_BUFFER_DESC {
                //         ByteWidth: 4 * std::mem::size_of::<f32>() as u32,
                //         Usage: D3D11_USAGE_DEFAULT,
                //         BindFlags: D3D11_BIND_INDEX_BUFFER,
                //         ..Default::default()
                //     };

                //     let subresource_desc = D3D11_SUBRESOURCE_DATA {
                //         pSysMem: verts.as_ptr() as *const _,
                //         ..Default::default()
                //     };

                //     let mut buffer = MaybeUninit::<Option<ID3D11Buffer>>::zeroed();
                //     device
                //         .CreateBuffer(
                //             &buffer_desc,
                //             Some(&subresource_desc),
                //             Some(buffer.as_mut_ptr()),
                //         )
                //         .expect("CreateBuffer failed");
                //     let buffer = buffer.assume_init();

                //     buffer
                // };

                // set the cell with the initialized data
                cell.set(RenderData {
                    sc_addr: &this as *const _,
                    rtv,
                    texture,
                    srv,
                    pixel_shader,
                    vertex_shader,
                    blend_state,
                    rasterizer_state,
                    depth_stencil_state,
                    sampler,
                    viewport,
                    scissor_rect,
                    last_frame_buffer: vec![0; (sc_desc.BufferDesc.Width * sc_desc.BufferDesc.Height * 4) as usize].into_boxed_slice(),
                    buffer_width: sc_desc.BufferDesc.Width,
                    buffer_height: sc_desc.BufferDesc.Height,
                    window_handle: sc_desc.OutputWindow,
                })
                .unwrap_unchecked();

                cell.get_mut().unwrap_unchecked() // SAFETY: we just set the cell
            }
        };

        let context = device.GetImmediateContext().unwrap();

        // use a new scope here to ensure the state backup is dropped at the end,
        // thus restoring the original render state before we call the original function
        {
            let _ = RenderStateBackup::new(device.GetImmediateContext().unwrap());

            // context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            //     TopLeftX: 0.0,
            //     TopLeftY: 0.0,
            //     Width: data.buffer_width as f32,
            //     Height: data.buffer_height as f32,
            //     ..Default::default()
            // }]));
            // context.IASetInputLayout(&data.input_layout);
            // context.IASetVertexBuffers(0, 1, Some(&Some(data.vertex_buffer)), Some(&data.vertex_buffer_stride), Some(&data.vertex_buffer_offset));
            // context.IASetIndexBuffer(&data.index_buffer, DXGI_FORMAT_R16_UINT, 0);
            // context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            // context.VSSetShader(&data.vertex_shader, None);
            // context.VSSetConstantBuffers(0, Some(&[Some(data.constant_buffer)]));
            // context.PSSetShader(&data.pixel_shader, None);
            // // context.PSSetSamplers(0, Some(&[Some(data.font_sampler)]));
            // context.GSSetShader(None, None);
            // context.HSSetShader(None, None);
            // context.DSSetShader(None, None);
            // context.CSSetShader(None, None);

            // // Setup blend state
            // context.OMSetBlendState(&data.blend_state, None, 0xFFFFFFFF); // second param?
            // context.OMSetDepthStencilState(&data.depth_stencil_state, 0); // last param?

            // context.RSSetState(&data.rasterizer_state);

            /*
                        _vertexBinding.Buffer = _vertexBuffer;
            _deviceContext.InputAssembler.SetVertexBuffers(0, _vertexBinding);
            _deviceContext.InputAssembler.SetIndexBuffer(_indexBuffer, Format.R16_UInt, 0);
            _deviceContext.InputAssembler.PrimitiveTopology = PrimitiveTopology.TriangleList;
            _deviceContext.VertexShader.Set(_vertexShader);
            _deviceContext.VertexShader.SetConstantBuffer(0, _vertexConstantBuffer);
            _deviceContext.PixelShader.Set(_pixelShader);
            _deviceContext.PixelShader.SetSampler(0, _fontSampler);
            _deviceContext.GeometryShader.Set(null);
            _deviceContext.HullShader.Set(null);
            _deviceContext.DomainShader.Set(null);
            _deviceContext.ComputeShader.Set(null);

            // Setup blend state
            _deviceContext.OutputMerger.BlendState = _blendState;
            _deviceContext.OutputMerger.BlendFactor = _blendColor;
            _deviceContext.OutputMerger.DepthStencilState = _depthStencilState;
            _deviceContext.Rasterizer.State = _rasterizerState; */


            /*
            // Setup orthographic projection matrix into our constant buffer
            // Our visible imgui space lies from drawData.DisplayPos (top left) to drawData.DisplayPos+drawData.DisplaySize (bottom right). DisplayPos is (0,0) for single viewport apps.
            var L = drawData.DisplayPos.X;
            var R = drawData.DisplayPos.X + drawData.DisplaySize.X;
            var T = drawData.DisplayPos.Y;
            var B = drawData.DisplayPos.Y + drawData.DisplaySize.Y;
            var mvp = new float[]
            {
                2f/(R-L),     0,              0,      0,
                0,            2f/(T-B),       0,      0,
                0,            0,              0.5f,   0,
                (R+L)/(L-R),  (T+B)/(B-T),    0.5f,   1f
            };

            var constantBuffer = _deviceContext.MapSubresource(_vertexConstantBuffer, 0, MapMode.WriteDiscard, MapFlags.None).DataPointer;
            unsafe
            {
                fixed (void* mvpPtr = mvp)
                {
                    System.Buffer.MemoryCopy(mvpPtr, constantBuffer.ToPointer(), 16 * sizeof(float), 16 * sizeof(float));
                }
            }
            _deviceContext.UnmapSubresource(_vertexConstantBuffer, 0); */

            // // setup vertex buffer for a single full-screen quad
            // {
            //     let mut mapped = MaybeUninit::<D3D11_MAPPED_SUBRESOURCE>::zeroed();
            //     context.Map(&data.vertex_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(mapped.as_mut_ptr())).expect("Map failed");
            //     let mapped = mapped.assume_init();

            //     let src = [
            //         [0f32, 0f32, 0f32, 1f32],
            //         [1f32, 0f32, 0f32, 1f32],
            //         [1f32, 1f32, 0f32, 1f32],
            //         [0f32, 1f32, 0f32, 1f32]
            //     ];
            //     let dst = mapped.pData as *mut f32;

            //     std::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());

            //     context.Unmap(&data.vertex_buffer, 0);
            // }
            // // Setup orthographic projection matrix into our constant buffer
            // {
            //     let mvp = [
            //         2f32 / data.buffer_width as f32, 0f32, 0f32, 0f32,
            //         0f32, 2f32 / -(data.buffer_height as i32) as f32, 0f32, 0f32,
            //         0f32, 0f32, 0.5f32, 0f32,
            //         -1f32, 1f32, 0.5f32, 1f32
            //     ];

            //     let mut mapped = MaybeUninit::<D3D11_MAPPED_SUBRESOURCE>::zeroed();
            //     context.Map(&data.constant_buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(mapped.as_mut_ptr())).expect("Map failed");
            //     let mapped = mapped.assume_init();

            //     let src = mvp.as_ptr();
            //     let dst = mapped.pData as *mut f32;

            //     std::ptr::copy_nonoverlapping(src, dst, mvp.len());

            //     context.Unmap(&data.constant_buffer, 0);
            // }

            // let colour = [0.2, 0.4, 0.6, 1.0];
            // context.ClearRenderTargetView(&data.rtv, colour.as_ptr());

            match WebView::instance().map(|wv| wv.capture.get_last_frame()).flatten() {
                Some(sub_data) => {
                    let mut mapped = MaybeUninit::<D3D11_MAPPED_SUBRESOURCE>::zeroed();
                    context.Map(&data.texture, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(mapped.as_mut_ptr())).expect("Map failed");
                    let mut mapped = mapped.assume_init();

                    let src = sub_data.pixels.as_ptr();
                    let dst = mapped.pData as *mut u8;

                    mapped.RowPitch = sub_data.width * 4;

                    let size = (data.buffer_width as usize * data.buffer_height as usize * 4).min(sub_data.pixels.len());
                    std::ptr::copy_nonoverlapping(src, dst, size);

                    context.Unmap(&data.texture, 0);
                    // let len = sub_data.width * sub_data.height * 4;
                    // data.last_frame_buffer = sub_data.pixels.clone();
                    
                    // context.UpdateSubresource(&data.texture, 0, None, data.last_frame_buffer.as_ptr() as *const _, sub_data.width * 4, len);
                },
                None => {
                    trace!("no frame to update with");
                }
            }
            
            context.RSSetViewports(Some(&[data.viewport]));
            context.RSSetScissorRects(Some(&[data.scissor_rect]));
            context.RSSetState(&data.rasterizer_state);

            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            context.IASetVertexBuffers(0, 0, None, None, None);

            context.VSSetShader(&data.vertex_shader, None);
            context.PSSetShader(&data.pixel_shader, None);

            context.PSSetShaderResources(0, Some(&[Some(data.srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(data.sampler.clone())]));

            context.OMSetBlendState(&data.blend_state, None, 0xffffffff);
            context.OMSetDepthStencilState(&data.depth_stencil_state, 0);
            
            context.OMSetRenderTargets(Some(&[Some(data.rtv.clone())]), None);

            context.Draw(3, 0);
        }

        // call the original function
        original.call(this, sync_interval, present_flags)
    })
}

// let (mut out, out_ptr, mut out_count) = temp_array!(D3D11_VIEWPORT, D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE);
macro_rules! temp_array {
    ($type: ident, $capacity: ident) => {{
        let mut out: MaybeUninit<[$type; $capacity as usize]> = MaybeUninit::zeroed();
        let out_ptr = out.as_mut_ptr();
        let out_count = $capacity as u32;

        (out, out_ptr, out_count)
    }};

    (Option<$opt_type: ident>, $capacity: ident) => {{
        let mut out: MaybeUninit<[Option<$opt_type>; $capacity as usize]> = MaybeUninit::zeroed();
        let out_ptr = out.as_mut_ptr();
        let out_count = $capacity as u32;

        (out, out_ptr, out_count)
    }};
}

// reconcile_array!(out, out_count);
macro_rules! reconcile_array {
    ($out: ident, $out_count: ident) => {{
        let mut vec = Vec::with_capacity($out_count as usize);
        vec.extend_from_slice(&$out.assume_init()[0..$out_count as usize]);
        vec
    }};
}

macro_rules! backup_shaders {
    (
        $context: ident, $obj_ptr: ident,
        ($shader_field: ident, $class_inst_field: ident) => $get_shader: ident,
        $constant_buf_field: ident => $get_constant_buf: ident,
        $resource_field: ident => $get_shader_resources: ident,
        $samplers_field: ident => $get_samplers: ident$(,)?
    ) => {{
        // save shader
        {
            let (out, out_ptr, mut out_count) =
                temp_array!(Option<ID3D11ClassInstance>, D3D11_SHADER_MAX_INTERFACES);

            $context.$get_shader(
                addr_of_mut!((*$obj_ptr).$shader_field),
                Some(out_ptr as *mut _),
                Some(&mut out_count),
            );

            addr_of_mut!((*$obj_ptr).$class_inst_field).write(reconcile_array!(out, out_count));
        }

        // save constant buffers
        {
            let (out, out_ptr, out_count) = temp_array!(
                Option<ID3D11Buffer>,
                D3D11_COMMONSHADER_CONSTANT_BUFFER_API_SLOT_COUNT
            );

            $context.$get_constant_buf(0, Some(&mut *out_ptr));

            addr_of_mut!((*$obj_ptr).$constant_buf_field).write(reconcile_array!(out, out_count));
        }

        // save resources
        {
            let (out, out_ptr, out_count) = temp_array!(
                Option<ID3D11ShaderResourceView>,
                D3D11_COMMONSHADER_INPUT_RESOURCE_SLOT_COUNT
            );

            $context.$get_shader_resources(0, Some(&mut *out_ptr));

            addr_of_mut!((*$obj_ptr).$resource_field).write(reconcile_array!(out, out_count));
        }

        // save samplers
        {
            let (out, out_ptr, out_count) = temp_array!(
                Option<ID3D11SamplerState>,
                D3D11_COMMONSHADER_SAMPLER_SLOT_COUNT
            );

            $context.$get_samplers(0, Some(&mut *out_ptr));

            addr_of_mut!((*$obj_ptr).$samplers_field).write(reconcile_array!(out, out_count));
        }
    }};
}

macro_rules! restore_shaders {
    (
        $context: expr,
        ($shader_field: expr, $class_inst_field: expr) => $set_shader: ident,
        $constant_buf_field: expr => $set_constant_buf: ident,
        $resource_field: expr => $set_shader_resources: ident,
        $samplers_field: expr => $set_samplers: ident$(,)?
    ) => {{
        $context.$set_shader($shader_field.as_ref(), Some($class_inst_field.as_slice()));
        $context.$set_constant_buf(0, Some($constant_buf_field.as_slice()));
        $context.$set_shader_resources(0, Some($resource_field.as_slice()));
        $context.$set_samplers(0, Some($samplers_field.as_slice()));
    }};
}

struct RenderStateBackup {
    context: ID3D11DeviceContext,

    // ### IA ###
    ia_input_layout: Option<ID3D11InputLayout>,
    ia_vertex_buffers: Vec<Option<ID3D11Buffer>>,
    ia_vertex_buffer_strides: Vec<u32>,
    ia_vertex_buffer_offsets: Vec<u32>,
    ia_index_buffer: Option<ID3D11Buffer>,
    ia_index_buffer_format: DXGI_FORMAT,
    ia_index_buffer_offset: u32,
    ia_primitive_topology: D3D_PRIMITIVE_TOPOLOGY,

    // ### RS ###
    rs_state: Option<ID3D11RasterizerState>,
    rs_viewport: Vec<D3D11_VIEWPORT>,
    rs_scissor_rect: Vec<RECT>,

    // ### OM ###
    om_blend_state: Option<ID3D11BlendState>,
    om_blend_factor: f32,
    om_sample_mask: u32,
    om_depth_stencil_state: Option<ID3D11DepthStencilState>,
    om_depth_stencil_ref: u32,
    om_render_targets: Vec<Option<ID3D11RenderTargetView>>,
    om_depth_stencil_view: Option<ID3D11DepthStencilView>,

    // ### VS ###
    vs_shader: Option<ID3D11VertexShader>,
    vs_class_instances: Vec<Option<ID3D11ClassInstance>>,
    vs_constant_buffers: Vec<Option<ID3D11Buffer>>,
    vs_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    vs_samplers: Vec<Option<ID3D11SamplerState>>,

    // ### HS ###
    hs_shader: Option<ID3D11HullShader>,
    hs_class_instances: Vec<Option<ID3D11ClassInstance>>,
    hs_constant_buffers: Vec<Option<ID3D11Buffer>>,
    hs_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    hs_samplers: Vec<Option<ID3D11SamplerState>>,

    // ### DS ###
    ds_shader: Option<ID3D11DomainShader>,
    ds_class_instances: Vec<Option<ID3D11ClassInstance>>,
    ds_constant_buffers: Vec<Option<ID3D11Buffer>>,
    ds_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    ds_samplers: Vec<Option<ID3D11SamplerState>>,

    // ### GS ###
    gs_shader: Option<ID3D11GeometryShader>,
    gs_class_instances: Vec<Option<ID3D11ClassInstance>>,
    gs_constant_buffers: Vec<Option<ID3D11Buffer>>,
    gs_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    gs_samplers: Vec<Option<ID3D11SamplerState>>,

    // ### PS ###
    ps_shader: Option<ID3D11PixelShader>,
    ps_class_instances: Vec<Option<ID3D11ClassInstance>>,
    ps_constant_buffers: Vec<Option<ID3D11Buffer>>,
    ps_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    ps_samplers: Vec<Option<ID3D11SamplerState>>,

    // ### CS ###
    cs_shader: Option<ID3D11ComputeShader>,
    cs_class_instances: Vec<Option<ID3D11ClassInstance>>,
    cs_constant_buffers: Vec<Option<ID3D11Buffer>>,
    cs_shader_resources: Vec<Option<ID3D11ShaderResourceView>>,
    cs_samplers: Vec<Option<ID3D11SamplerState>>,
}

impl RenderStateBackup {
    #[allow(const_item_mutation)]
    pub unsafe fn new(context: ID3D11DeviceContext) -> Self {
        // why are some of these using `as *mut _`?
        // this is why: https://github.com/microsoft/windows-rs/issues/1567
        let mut obj = MaybeUninit::<Self>::zeroed();
        let obj_ptr = obj.as_mut_ptr();

        Self::backup_ia(&context, obj_ptr);
        Self::backup_rs(&context, obj_ptr);
        Self::backup_om(&context, obj_ptr);
        Self::backup_vs(&context, obj_ptr);
        Self::backup_hs(&context, obj_ptr);
        Self::backup_ds(&context, obj_ptr);
        Self::backup_gs(&context, obj_ptr);
        Self::backup_ps(&context, obj_ptr);
        Self::backup_cs(&context, obj_ptr);

        // save the context
        addr_of_mut!((*obj_ptr).context).write(context);

        obj.assume_init()
    }

    unsafe fn backup_ia(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        // save input layout
        {
            addr_of_mut!((*obj_ptr).ia_input_layout).write(context.IAGetInputLayout().ok());
        }

        // save index buffer
        {
            context.IAGetIndexBuffer(
                Some(addr_of_mut!((*obj_ptr).ia_index_buffer)),
                Some(addr_of_mut!((*obj_ptr).ia_index_buffer_format)),
                Some(addr_of_mut!((*obj_ptr).ia_index_buffer_offset)),
            );
        }

        // save primitive topology
        {
            addr_of_mut!((*obj_ptr).ia_primitive_topology).write(context.IAGetPrimitiveTopology());
        }

        // save vertex buffers
        {
            let (buf_out, mut buf_out_ptr, buf_out_count) = temp_array!(
                Option<ID3D11Buffer>,
                D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT
            );
            let (stride_out, mut stride_out_ptr, stride_out_count) =
                temp_array!(u32, D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT);
            let (offset_out, mut offset_out_ptr, offset_out_count) =
                temp_array!(u32, D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT);

            context.IAGetVertexBuffers(
                0,
                D3D11_IA_VERTEX_INPUT_RESOURCE_SLOT_COUNT,
                Some(addr_of_mut!(buf_out_ptr) as *mut _),
                Some(addr_of_mut!(stride_out_ptr) as *mut _),
                Some(addr_of_mut!(offset_out_ptr) as *mut _),
            );

            addr_of_mut!((*obj_ptr).ia_vertex_buffers)
                .write(reconcile_array!(buf_out, buf_out_count));
            addr_of_mut!((*obj_ptr).ia_vertex_buffer_strides)
                .write(reconcile_array!(stride_out, stride_out_count));
            addr_of_mut!((*obj_ptr).ia_vertex_buffer_offsets)
                .write(reconcile_array!(offset_out, offset_out_count));
        }
    }

    unsafe fn backup_rs(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        // save rasterizer state
        {
            addr_of_mut!((*obj_ptr).rs_state).write(context.RSGetState().ok());
        }

        // save viewport
        {
            let (out, mut out_ptr, mut out_count) = temp_array!(
                D3D11_VIEWPORT,
                D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE
            );

            context.RSGetViewports(&mut out_count, Some(addr_of_mut!(out_ptr) as *mut _));

            addr_of_mut!((*obj_ptr).rs_viewport).write(reconcile_array!(out, out_count));
        }

        // save scissor rects
        {
            let (out, mut out_ptr, mut out_count) = temp_array!(
                RECT,
                D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE
            );

            context.RSGetScissorRects(&mut out_count, Some(addr_of_mut!(out_ptr) as *mut _));

            addr_of_mut!((*obj_ptr).rs_scissor_rect).write(reconcile_array!(out, out_count));
        }
    }

    unsafe fn backup_om(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        // save blend state
        {
            context.OMGetBlendState(
                Some(addr_of_mut!((*obj_ptr).om_blend_state)),
                Some(addr_of_mut!((*obj_ptr).om_blend_factor)),
                Some(addr_of_mut!((*obj_ptr).om_sample_mask)),
            );
        }

        // save depth stencil state
        {
            context.OMGetDepthStencilState(
                Some(addr_of_mut!((*obj_ptr).om_depth_stencil_state)),
                Some(addr_of_mut!((*obj_ptr).om_depth_stencil_ref)),
            );
        }

        // save render targets
        {
            context.OMGetRenderTargets(
                Some(&mut *addr_of_mut!((*obj_ptr).om_render_targets)),
                Some(addr_of_mut!((*obj_ptr).om_depth_stencil_view)),
            );
        }
    }

    unsafe fn backup_vs(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (vs_shader, vs_class_instances) => VSGetShader,
            vs_constant_buffers => VSGetConstantBuffers,
            vs_shader_resources => VSGetShaderResources,
            vs_samplers => VSGetSamplers,
        );
    }

    unsafe fn backup_hs(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (hs_shader, hs_class_instances) => HSGetShader,
            hs_constant_buffers => HSGetConstantBuffers,
            hs_shader_resources => HSGetShaderResources,
            hs_samplers => HSGetSamplers,
        );
    }

    unsafe fn backup_ds(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (ds_shader, ds_class_instances) => DSGetShader,
            ds_constant_buffers => DSGetConstantBuffers,
            ds_shader_resources => DSGetShaderResources,
            ds_samplers => DSGetSamplers,
        );
    }

    unsafe fn backup_gs(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (gs_shader, gs_class_instances) => GSGetShader,
            gs_constant_buffers => GSGetConstantBuffers,
            gs_shader_resources => GSGetShaderResources,
            gs_samplers => GSGetSamplers,
        );
    }

    unsafe fn backup_ps(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (ps_shader, ps_class_instances) => PSGetShader,
            ps_constant_buffers => PSGetConstantBuffers,
            ps_shader_resources => PSGetShaderResources,
            ps_samplers => PSGetSamplers,
        );
    }

    unsafe fn backup_cs(context: &ID3D11DeviceContext, obj_ptr: *mut Self) {
        backup_shaders!(
            context, obj_ptr,
            (cs_shader, cs_class_instances) => CSGetShader,
            cs_constant_buffers => CSGetConstantBuffers,
            cs_shader_resources => CSGetShaderResources,
            cs_samplers => CSGetSamplers,
        );
    }

    unsafe fn restore_ia(&self) {
        self.context.IASetInputLayout(self.ia_input_layout.as_ref());
        self.context
            .IASetPrimitiveTopology(self.ia_primitive_topology);
        self.context.IASetVertexBuffers(
            0,
            self.ia_vertex_buffers.len() as u32,
            Some(self.ia_vertex_buffers.as_ptr()),
            Some(self.ia_vertex_buffer_strides.as_ptr()),
            Some(self.ia_vertex_buffer_offsets.as_ptr()),
        );
    }

    unsafe fn restore_rs(&self) {
        self.context.RSSetState(self.rs_state.as_ref());
        self.context
            .RSSetViewports(Some(self.rs_viewport.as_slice()));
        self.context
            .RSSetScissorRects(Some(self.rs_scissor_rect.as_slice()));
    }

    unsafe fn restore_om(&self) {
        self.context.OMSetBlendState(
            self.om_blend_state.as_ref(),
            Some(&self.om_blend_factor),
            self.om_sample_mask,
        );
        self.context.OMSetDepthStencilState(
            self.om_depth_stencil_state.as_ref(),
            self.om_depth_stencil_ref,
        );
        self.context.OMSetRenderTargets(
            Some(&self.om_render_targets),
            self.om_depth_stencil_view.as_ref(),
        );
    }

    unsafe fn restore_vs(&self) {
        restore_shaders!(
            self.context,
            (self.vs_shader, self.vs_class_instances) => VSSetShader,
            self.vs_constant_buffers => VSSetConstantBuffers,
            self.vs_shader_resources => VSSetShaderResources,
            self.vs_samplers => VSSetSamplers,
        );
    }

    unsafe fn restore_hs(&self) {
        restore_shaders!(
            self.context,
            (self.hs_shader, self.hs_class_instances) => HSSetShader,
            self.hs_constant_buffers => HSSetConstantBuffers,
            self.hs_shader_resources => HSSetShaderResources,
            self.hs_samplers => HSSetSamplers,
        );
    }

    unsafe fn restore_ds(&self) {
        restore_shaders!(
            self.context,
            (self.ds_shader, self.ds_class_instances) => DSSetShader,
            self.ds_constant_buffers => DSSetConstantBuffers,
            self.ds_shader_resources => DSSetShaderResources,
            self.ds_samplers => DSSetSamplers,
        );
    }

    unsafe fn restore_gs(&self) {
        restore_shaders!(
            self.context,
            (self.gs_shader, self.gs_class_instances) => GSSetShader,
            self.gs_constant_buffers => GSSetConstantBuffers,
            self.gs_shader_resources => GSSetShaderResources,
            self.gs_samplers => GSSetSamplers,
        );
    }

    unsafe fn restore_ps(&self) {
        restore_shaders!(
            self.context,
            (self.ps_shader, self.ps_class_instances) => PSSetShader,
            self.ps_constant_buffers => PSSetConstantBuffers,
            self.ps_shader_resources => PSSetShaderResources,
            self.ps_samplers => PSSetSamplers,
        );
    }

    unsafe fn restore_cs(&self) {
        restore_shaders!(
            self.context,
            (self.cs_shader, self.cs_class_instances) => CSSetShader,
            self.cs_constant_buffers => CSSetConstantBuffers,
            self.cs_shader_resources => CSSetShaderResources,
            self.cs_samplers => CSSetSamplers,
        );
    }
}

/// Restores the render state that was backed up in the constructor.
impl Drop for RenderStateBackup {
    fn drop(&mut self) {
        unsafe {
            self.restore_ia();
            self.restore_rs();
            self.restore_om();
            self.restore_vs();
            self.restore_hs();
            self.restore_ds();
            self.restore_gs();
            self.restore_ps();
            self.restore_cs();
        }
    }
}
