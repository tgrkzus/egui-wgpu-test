use std::iter;
use std::time::Instant;
use eframe::UserEvent::RequestRepaint;

use egui_wgpu::renderer::ScreenDescriptor;
use winit::event::Event::*;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopBuilder};
use winit::window::WindowBuilder;


fn main() {
    env_logger::init();
    let event_loop = EventLoopBuilder::new().build();
    let window = WindowBuilder::new()
        .with_title("Grims")
        .with_inner_size(winit::dpi::PhysicalSize::new(1300, 1300))
        .with_resizable(true)
        .with_transparent(false)
        .build(&event_loop)
        .unwrap();
    let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
    let surface = unsafe { instance.create_surface(&window) };

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
        .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::default(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ))
        .unwrap();

    let size = window.inner_size();
    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_supported_formats(&adapter)[0],
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface.get_supported_alpha_modes(&adapter)[0],
    };
    surface.configure(&device, &surface_config);

    let mut egui_renderer = egui_wgpu::Renderer::new(
        &device,
        surface.get_supported_formats(&adapter)[0],
        None,
        1
    );
    let mut egui_state = egui_winit::State::new(&event_loop);

    // Display the demo application that ships with egui.
    let mut demo_app = egui_demo_lib::DemoWindows::default();

    let context = egui::Context::default();
    event_loop.run(move |event, _, control_flow| {
        match event {
            RedrawRequested(..) => {
                let output_frame = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Outdated) => {
                        // This error occurs when the app is minimized on Windows.
                        // Silently return here to prevent spamming the console with:
                        // "The underlying surface has changed, and therefore the swap chain must be updated"
                        return;
                    }
                    Err(e) => {
                        eprintln!("Dropped frame with error: {}", e);
                        return;
                    }
                };
                let view = output_frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let input = egui_state.take_egui_input(&window);
                context.begin_frame(input);
                demo_app.ui(&context);
                egui::Window::new("Window").show(&context, |ui| {
                    ui.label("Hello world!");
                    ui.label("See https://github.com/emilk/egui for how to make other UI elements");
                });
                let output = context.end_frame();
                let paint_jobs = context.tessellate(output.shapes);

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });
                
                // My rendering
                {
                    // Blah blah render pipeline stuff here
                }
                
                // Egui rendering now
                let screen_descriptor = ScreenDescriptor {
                    size_in_pixels: [surface_config.width, surface_config.height],
                    // Forcing pixels per point 1.0 - the egui input handling seems to not scale the cursor coordinates automatically
                    pixels_per_point: 1.0, 
                };
                
                let user_cmd_bufs = {
                    for (id, image_delta) in &output.textures_delta.set {
                        egui_renderer.update_texture(
                            &device,
                            &queue,
                            *id,
                            image_delta,
                        );
                    }

                    egui_renderer.update_buffers(
                        &device,
                        &queue,
                        &mut encoder,
                        &paint_jobs.as_ref(),
                        &screen_descriptor,
                    )
                };
                
                egui_renderer.update_buffers(&device, &queue, &mut encoder, &paint_jobs.as_ref(), &screen_descriptor);
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("UI Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });

                    egui_renderer
                        .render(
                            &mut render_pass,
                            &paint_jobs.as_ref(),
                            &screen_descriptor,
                        );
                }
                

                for id in &output.textures_delta.free {
                    egui_renderer.free_texture(id);
                }
                    
                queue.submit(
                    user_cmd_bufs
                        .into_iter()
                        .chain(std::iter::once(encoder.finish())),
                );

                // Redraw egui
                output_frame.present();
            }
            MainEventsCleared  => {
                window.request_redraw();
            }
            WindowEvent { event, .. } => {
                // TODO use for seeing if egui wanted this event or not
                let _ = egui_state.on_event(&context, &event);
                match event {
                    winit::event::WindowEvent::Resized(size) => {
                        // Resize with 0 width and height is used by winit to signal a minimize event on Windows.
                        // See: https://github.com/rust-windowing/winit/issues/208
                        // This solves an issue where the app would panic when minimizing on Windows.
                        if size.width > 0 && size.height > 0 {
                            surface_config.width = size.width;
                            surface_config.height = size.height;
                            surface.configure(&device, &surface_config);
                        }
                    }
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                }
            },
            _ => (),
        }
    });
}