use crate::renderer::Renderer;
use winit::event::*;

pub fn handle_input(event: &WindowEvent, renderer: &mut Renderer) -> bool {
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            let center_w = (renderer.size.width / 2) as f64;
            let center_h = (renderer.size.height / 2) as f64;
            let max_dist_to_center = (center_w.powi(2) + center_h.powi(2)).sqrt();
            let dist_to_center_normalized =
                ((center_w - position.x).powi(2) + (center_h - position.y).powi(2)).sqrt()
                    / max_dist_to_center;
            renderer.clear_color = wgpu::Color {
                r: dist_to_center_normalized,
                g: 1.0 - dist_to_center_normalized,
                b: 0.0,
                a: 1.0,
            };
            true
        }
        WindowEvent::KeyboardInput { input, .. } => {
            if let KeyboardInput {
                state: ElementState::Pressed,
                virtual_keycode,
                ..
            } = input
            {
                return match virtual_keycode {
                    Some(VirtualKeyCode::Space) => {
                        println!("space");
                        true
                    }
                    _ => false,
                };
            }
            false
        }
        _ => false,
    }
}
