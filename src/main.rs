use std::sync::Arc;
mod camera;

mod aabb;
mod app;
mod cell;
mod disk_util;
mod map;
mod quadtree;
mod texture_quadtree;
mod window_state;

use app::{App, SwapchainState};
use vulkano::{
    instance::debug::{DebugUtilsMessenger, DebugUtilsMessengerCreateInfo},
    sync::GpuFuture,
};
use window_state::WindowState;

use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::Window,
};

mod util {
    use crate::map::Map;

    pub fn get_map() -> Map {
        let mut args = std::env::args();
        let _ = args.next();
        let map_path = args.next().unwrap();

        Map::new(map_path).unwrap()
    }
}

fn main() {
    let map = util::get_map();

    let (window_state, event_loop) = WindowState::create(map.info.name.clone());

    let mut app = App::new(window_state, map);

    let _callback = unsafe {
        DebugUtilsMessenger::new(
            app.window_state.instance.clone(),
            DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
                println!("Debug callback: {:?}", msg.description);
            })),
        )
        .ok()
    };

    let mut swapachain_state = SwapchainState::Good;

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,

        Event::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } => {
            swapachain_state = SwapchainState::Dirty;
        }

        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                },
            ..
        } => {
            let keycode = virtual_keycode.unwrap();
            match keycode {
                VirtualKeyCode::W => app.camera.move_up(),
                VirtualKeyCode::A => app.camera.move_left(),
                VirtualKeyCode::S => app.camera.move_down(),
                VirtualKeyCode::D => app.camera.move_right(),
                VirtualKeyCode::L => app.camera.rotate_ccw_horizontally(),
                VirtualKeyCode::H => app.camera.rotate_cw_horizontally(),
                VirtualKeyCode::J => app.camera.rotate_cw_vertically(),
                VirtualKeyCode::K => app.camera.rotate_ccw_vertically(),
                VirtualKeyCode::U => app.camera.rotate_ccw_sideways(),
                VirtualKeyCode::I => app.camera.rotate_cw_sideways(),
                VirtualKeyCode::Equals => app.camera.move_forward(),
                VirtualKeyCode::Minus => app.camera.move_backward(),
                VirtualKeyCode::O => app.camera.reset(),
                VirtualKeyCode::Q => *control_flow = ControlFlow::Exit,
                _k => {}
            }

            app.previous_frame_end.as_mut().unwrap().cleanup_finished();
            app.camera_updated();
        }

        Event::RedrawEventsCleared => {
            let window = app
                .window_state
                .surface
                .object()
                .unwrap()
                .downcast_ref::<Window>()
                .unwrap();
            let dimensions = window.inner_size();
            if dimensions.width == 0 || dimensions.height == 0 {
                return;
            }
            app.previous_frame_end.as_mut().unwrap().cleanup_finished();

            match swapachain_state {
                SwapchainState::Dirty | SwapchainState::SubOptimal => {
                    app.recreate_swapchain();
                }
                _ => {}
            }

            swapachain_state = app.draw();
        }

        _ => {}
    });
}
