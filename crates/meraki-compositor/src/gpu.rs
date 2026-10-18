use crate::*;
use wgpu::util::DeviceExt;
struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}
struct Layer {
    size: [u32; 2],
    upload: Texture,
    a: Texture,
    b: Texture,
}
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipelines: Vec<wgpu::RenderPipeline>,
    layers: Vec<Layer>,
    output: Option<Texture>,
    pair: Option<Texture>,
    readback: Option<wgpu::Buffer>,
    size: [u32; 2],
    pub name: String,
    failed: Arc<std::sync::atomic::AtomicBool>,
}
impl Gpu {
    pub fn new() -> Result<Self, String> {
        pollster::block_on(Self::initialize())
    }
    async fn initialize() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: match std::env::var("MERAKI_GPU_BACKEND").as_deref() {
                Ok("dx12") => wgpu::Backends::DX12,
                Ok("vulkan") => wgpu::Backends::VULKAN,
                _ => wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            },
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            return Err("Software GPU: using Rayon".into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Meraki Meraki"),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let err = failed.clone();
        device.on_uncaptured_error(Box::new(move |e| {
            eprintln!("Meraki GPU: {e}");
            err.store(true, std::sync::atomic::Ordering::Release);
        }));
        let lost = failed.clone();
        device.set_device_lost_callback(move |_, _| {
            lost.store(true, std::sync::atomic::Ordering::Release);
        });
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Meraki effects"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let mut pipelines = Vec::new();
        for (i, entry) in ["premultiply", "color", "blur", "place", "place"]
            .iter()
            .enumerate()
        {
            pipelines.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pl),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vertex"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: if i == 3 {
                                wgpu::TextureFormat::Rgba8Unorm
                            } else {
                                wgpu::TextureFormat::Rgba16Float
                            },
                            blend: if i == 3 {
                                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
                            } else if i == 4 {
                                let add = wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                };
                                Some(wgpu::BlendState {
                                    color: add,
                                    alpha: add,
                                })
                            } else {
                                None
                            },
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                }),
            );
        }
        if let Some(e) = device.pop_error_scope().await {
            return Err(e.to_string());
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            layout,
            sampler,
            pipelines,
            layers: vec![],
            output: None,
            pair: None,
            readback: None,
            size: [0, 0],
            name: format!("{} / {:?}", info.name, info.backend),
            failed,
        })
    }
    fn texture(&self, size: [u32; 2], format: wgpu::TextureFormat) -> Texture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Texture { texture, view }
    }
    fn pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src: &Texture,
        dst: &Texture,
        index: usize,
        p: [f32; 16],
        clear: bool,
    ) {
        let bytes: Vec<u8> = p.iter().flat_map(|v| v.to_le_bytes()).collect();
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipelines[index]);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
    pub fn compose(
        &mut self,
        timeline: &Timeline,
        frames: &[(&Clip, Arc<VideoFrame>)],
        size: [u32; 2],
    ) -> Result<Vec<u8>, String> {
        self.compose_plan(timeline, frames, size, &vec![Blend::Over; frames.len()])
    }
    pub(crate) fn compose_plan(
        &mut self,
        timeline: &Timeline,
        frames: &[(&Clip, Arc<VideoFrame>)],
        size: [u32; 2],
        blends: &[Blend],
    ) -> Result<Vec<u8>, String> {
        use std::sync::atomic::Ordering;
        if self.failed.load(Ordering::Acquire) {
            return Err("GPU lost".into());
        }
        let stride = (size[0] * 4).div_ceil(256) * 256;
        if self.size != size {
            self.output = Some(self.texture(size, wgpu::TextureFormat::Rgba8Unorm));
            self.pair = Some(self.texture(size, wgpu::TextureFormat::Rgba16Float));
            self.readback = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(stride) * u64::from(size[1]),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.size = size;
        }
        self.layers.truncate(frames.len());
        for (i, (_, f)) in frames.iter().enumerate() {
            f.validate()?;
            let s = [f.width, f.height];
            if self.layers.get(i).is_none_or(|l| l.size != s) {
                let l = Layer {
                    size: s,
                    upload: self.texture(s, wgpu::TextureFormat::Rgba8Unorm),
                    a: self.texture(s, wgpu::TextureFormat::Rgba16Float),
                    b: self.texture(s, wgpu::TextureFormat::Rgba16Float),
                };
                if i == self.layers.len() {
                    self.layers.push(l);
                } else {
                    self.layers[i] = l;
                }
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.layers[i].upload.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &f.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(f.stride),
                    rows_per_image: Some(f.height),
                },
                wgpu::Extent3d {
                    width: f.width,
                    height: f.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let output = self.output.as_ref().unwrap();
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        for (i, (clip, f)) in frames.iter().enumerate() {
            let l = &self.layers[i];
            self.pass(&mut encoder, &l.upload, &l.a, 0, [0.; 16], true);
            let (mut src, mut dst) = (&l.a, &l.b);
            if let Some(id) = clip.effects {
                for op in compile(timeline.graphs.get(&id).ok_or("Missing graph")?)
                    .map_err(|e| e.to_string())?
                {
                    match op {
                        Operation::Color {
                            brightness,
                            contrast,
                            saturation,
                        } => {
                            let mut p = [0.; 16];
                            p[..3].copy_from_slice(&[brightness, contrast, saturation]);
                            self.pass(&mut encoder, src, dst, 1, p, true);
                            std::mem::swap(&mut src, &mut dst);
                        }
                        Operation::Blur { radius } => {
                            let radius = (radius
                                * (f.width as f32 / 1920.).max(f.height as f32 / 1080.))
                            .round();
                            if radius > 0. {
                                for dir in [[1. / f.width as f32, 0.], [0., 1. / f.height as f32]] {
                                    let mut p = [0.; 16];
                                    p[..3].copy_from_slice(&[dir[0], dir[1], radius]);
                                    self.pass(&mut encoder, src, dst, 2, p, true);
                                    std::mem::swap(&mut src, &mut dst);
                                }
                            }
                        }
                    }
                }
            }
            let [a, b, c, d, tx, ty] = clip.transform.affine([f.width, f.height], size);
            let det = a * d - b * c;
            let mut p = [0.; 16];
            p[..7].copy_from_slice(&[
                d / det,
                -c / det,
                -b / det,
                a / det,
                tx,
                ty,
                clip.transform.opacity,
            ]);
            if blends[i] == Blend::Over {
                self.pass(&mut encoder, src, output, 3, p, false);
            } else {
                let pair = self.pair.as_ref().unwrap();
                self.pass(&mut encoder, src, pair, 4, p, blends[i] == Blend::Begin);
                if blends[i] == Blend::End {
                    let mut identity = [0.; 16];
                    identity[..7].copy_from_slice(&[1., 0., 0., 1., 0., 0., 1.]);
                    self.pass(&mut encoder, pair, output, 3, identity, false);
                }
            }
        }
        let readback = self.readback.as_ref().unwrap();
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            self.device
                .poll(wgpu::PollType::Poll)
                .map_err(|e| e.to_string())?;
            if let Ok(r) = rx.try_recv() {
                r.map_err(|e| e.to_string())?;
                break;
            }
            if self.failed.load(Ordering::Acquire) || std::time::Instant::now() > deadline {
                return Err("GPU readback timeout/device lost".into());
            }
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        let mapped = readback.slice(..).get_mapped_range();
        let mut bytes = Vec::with_capacity((size[0] * size[1] * 4) as usize);
        for row in mapped.chunks(stride as usize) {
            bytes.extend_from_slice(&row[..size[0] as usize * 4]);
        }
        drop(mapped);
        readback.unmap();
        if self.failed.load(Ordering::Acquire) {
            return Err("GPU validation failure".into());
        }
        Ok(bytes)
    }
}
