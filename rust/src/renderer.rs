use bytemuck::{Pod, Zeroable};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub type Color = [f32; 4];

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

pub struct Painter {
    width: f32,
    height: f32,
    vertices: Vec<Vertex>,
}

impl Painter {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            vertices: Vec::new(),
        }
    }

    fn vertex(&self, x: f32, y: f32, color: Color) -> Vertex {
        Vertex {
            position: [x / self.width * 2.0 - 1.0, 1.0 - y / self.height * 2.0],
            color,
        }
    }

    pub fn rect(&mut self, rect: Rect, color: Color) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let a = self.vertex(rect.x, rect.y, color);
        let b = self.vertex(rect.x + rect.w, rect.y, color);
        let c = self.vertex(rect.x + rect.w, rect.y + rect.h, color);
        let d = self.vertex(rect.x, rect.y + rect.h, color);
        self.vertices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    pub fn triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: Color) {
        self.vertices.extend_from_slice(&[
            self.vertex(a[0], a[1], color),
            self.vertex(b[0], b[1], color),
            self.vertex(c[0], c[1], color),
        ]);
    }

    pub fn line(&mut self, ax: f32, ay: f32, bx: f32, by: f32, thickness: f32, color: Color) {
        let dx = bx - ax;
        let dy = by - ay;
        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.001 {
            return;
        }
        let nx = -dy / length * thickness * 0.5;
        let ny = dx / length * thickness * 0.5;
        let a = self.vertex(ax + nx, ay + ny, color);
        let b = self.vertex(bx + nx, by + ny, color);
        let c = self.vertex(bx - nx, by - ny, color);
        let d = self.vertex(ax - nx, ay - ny, color);
        self.vertices.extend_from_slice(&[a, b, c, a, c, d]);
    }

    pub fn outline(&mut self, rect: Rect, thickness: f32, color: Color) {
        self.line(rect.x, rect.y, rect.x + rect.w, rect.y, thickness, color);
        self.line(
            rect.x + rect.w,
            rect.y,
            rect.x + rect.w,
            rect.y + rect.h,
            thickness,
            color,
        );
        self.line(
            rect.x + rect.w,
            rect.y + rect.h,
            rect.x,
            rect.y + rect.h,
            thickness,
            color,
        );
        self.line(rect.x, rect.y + rect.h, rect.x, rect.y, thickness, color);
    }

    pub fn circle(&mut self, x: f32, y: f32, radius: f32, thickness: f32, color: Color) {
        let segments = 96;
        for i in 0..segments {
            let a = i as f32 * std::f32::consts::TAU / segments as f32;
            let b = (i + 1) as f32 * std::f32::consts::TAU / segments as f32;
            self.line(
                x + radius * a.cos(),
                y + radius * a.sin(),
                x + radius * b.cos(),
                y + radius * b.sin(),
                thickness,
                color,
            );
        }
    }

    pub fn text(&mut self, x: f32, y: f32, label: &str, scale: f32, color: Color) {
        let mut cursor = x;
        for ch in label.chars() {
            if let Some(rows) = BASIC_FONTS.get(ch) {
                for (row, bits) in rows.iter().enumerate() {
                    for col in 0..8 {
                        if (bits >> col) & 1 == 1 {
                            self.rect(
                                Rect {
                                    x: cursor + col as f32 * scale,
                                    y: y + row as f32 * scale,
                                    w: scale,
                                    h: scale,
                                },
                                color,
                            );
                        }
                    }
                }
            }
            cursor += 8.0 * scale;
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|error| format!("surface: {error}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("adapter: {error}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| format!("device: {error}"))?;
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("no supported surface configuration")?;
        if surface
            .get_capabilities(&adapter)
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        }
        surface.configure(&device, &config);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PuppyCAD UI shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PuppyCAD UI pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn draw(&mut self, painter: Painter) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&Default::default());
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("PuppyCAD UI vertices"),
                contents: bytemuck::cast_slice(&painter.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("PuppyCAD frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PuppyCAD UI"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..painter.vertices.len() as u32, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
