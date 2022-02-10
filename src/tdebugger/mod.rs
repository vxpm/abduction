mod debugger;
mod tui_helper;

use atomic::Atomic;
use crossterm::execute;
use debugger::*;
use parking_lot::Mutex;
use std::{
    io,
    sync::{atomic::AtomicBool, Arc},
};
use tui::backend::CrosstermBackend;
use tui_helper::*;

use crate::gameboy::Gameboy;

pub fn run_with_debugger(args: crate::AbductionArgs) -> anyhow::Result<()> {
    // create shared state
    let boot = crate::util::read_bytes(args.boot)?;
    let rom = crate::util::read_bytes(args.rom)?;
    let gameboy = Mutex::new(Gameboy::new(rom, boot)?);

    let shared = Arc::new(DebuggerShared {
        gameboy,
        state: Atomic::new(DebuggerEmulationState::Stepping),
        exit: AtomicBool::new(false),
    });

    // spawn thread for gameboy
    let shared_clone = shared.clone();
    let _ = std::thread::spawn(|| {
        let shared = shared_clone;
        let mut m_cycles;
        loop {
            m_cycles = 0;
            let before = std::time::Instant::now();

            if shared.exit.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            if let DebuggerEmulationState::Stepping =
                shared.state.load(std::sync::atomic::Ordering::Relaxed)
            {
                let mut lock = shared.gameboy.lock();
                for _ in 0..4 {
                    m_cycles += lock.step();
                }
            }

            let frame_time: std::time::Duration =
                std::time::Duration::from_nanos(m_cycles as u64 * 953);

            // TODO: make this better? idk sleeping would be nice
            while !frame_time.saturating_sub(before.elapsed()).is_zero() {
                std::hint::spin_loop();
            }
        }
    });

    // spawn thread for app
    let shared_clone = shared.clone();
    std::thread::spawn(move || {
        let shared = shared_clone;

        // setup terminal
        crossterm::terminal::enable_raw_mode().unwrap();
        let mut stdout = io::stdout();
        execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )
        .unwrap();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = tui::Terminal::new(backend).unwrap();

        // create app and run it
        let app = App::new(vec![Box::new(SummaryTab::new(shared.clone()))], 30);
        let res = run_app(&mut terminal, app);

        // restore terminal
        crossterm::terminal::disable_raw_mode().unwrap();
        execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )
        .unwrap();
        terminal.show_cursor().unwrap();
        shared.exit.store(true, std::sync::atomic::Ordering::SeqCst);

        // unwrap result
        res.unwrap();
    });

    // open window
    let event_loop = winit::event_loop::EventLoop::new();
    let mut input = winit_input_helper::WinitInputHelper::new();
    let window = {
        let size = winit::dpi::LogicalSize::new(
            160u16 * args.size_multiplier as u16,
            144u16 * args.size_multiplier as u16,
        );
        winit::window::WindowBuilder::new()
            .with_title("abduction")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_resizable(false)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            pixels::SurfaceTexture::new(window_size.width, window_size.height, &window);
        pixels::Pixels::new(160, 144, surface_texture).unwrap()
    };

    // run window
    let mut last_redraw = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        if shared.exit.load(std::sync::atomic::Ordering::SeqCst) {
            *control_flow = winit::event_loop::ControlFlow::Exit;
        }

        match event {
            winit::event::Event::RedrawRequested(_) => {
                {
                    let lock = shared.gameboy.lock();
                    let buffer = lock.ppu().screen();
                    let pixels_frame = pixels.get_frame();

                    for (i, pixel) in pixels_frame.chunks_exact_mut(4).enumerate() {
                        let (y, x) = crate::util::div_rem(i, 160);
                        let v = buffer.get_pixel(x as usize, y as usize).unwrap();
                        // let c = match v {
                        //     3 => [0x92, 0x5E, 0xC2, 0xFF],
                        //     2 => [0xCF, 0x5B, 0xA6, 0xFF],
                        //     1 => [0xFF, 0x71, 0x8F, 0xFF],
                        //     0 => [0xFF, 0x99, 0x6F, 0xFF],
                        //     _ => unreachable!(),
                        // };

                        let c = match v {
                            3 => [0x1B, 0x03, 0x26, 0xFF],
                            2 => [0x7A, 0x1C, 0x4B, 0xFF],
                            1 => [0xBA, 0x50, 0x44, 0xFF],
                            0 => [0xDC, 0xBC, 0xA1, 0xFF],
                            _ => unreachable!(),
                        };

                        pixel.copy_from_slice(&c);
                    }

                    last_redraw = std::time::Instant::now();
                }

                if pixels.render().is_err() {
                    shared.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
            }
            _ => {
                if input.update(&event) {
                    // Close events
                    if input.key_pressed(winit::event::VirtualKeyCode::Escape) || input.quit() {
                        shared.exit.store(true, std::sync::atomic::Ordering::SeqCst);
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                        return;
                    }

                    // Resize the window
                    if let Some(size) = input.window_resized() {
                        pixels.resize_surface(size.width, size.height);
                    }

                    // Update input
                    const INPUT_CHECK: [(
                        crate::gameboy::JoypadButton,
                        winit::event::VirtualKeyCode,
                    ); 8] = [
                        (
                            crate::gameboy::JoypadButton::Right,
                            winit::event::VirtualKeyCode::Right,
                        ),
                        (
                            crate::gameboy::JoypadButton::A,
                            winit::event::VirtualKeyCode::Z,
                        ),
                        (
                            crate::gameboy::JoypadButton::Left,
                            winit::event::VirtualKeyCode::Left,
                        ),
                        (
                            crate::gameboy::JoypadButton::B,
                            winit::event::VirtualKeyCode::X,
                        ),
                        (
                            crate::gameboy::JoypadButton::Up,
                            winit::event::VirtualKeyCode::Up,
                        ),
                        (
                            crate::gameboy::JoypadButton::Select,
                            winit::event::VirtualKeyCode::C,
                        ),
                        (
                            crate::gameboy::JoypadButton::Down,
                            winit::event::VirtualKeyCode::Down,
                        ),
                        (
                            crate::gameboy::JoypadButton::Start,
                            winit::event::VirtualKeyCode::Space,
                        ),
                    ];

                    {
                        let mut lock = shared.gameboy.lock();
                        for (button, key) in INPUT_CHECK {
                            if input.key_pressed(key) || input.key_held(key) {
                                lock.joypad_mut().set_button(button, true);
                            } else {
                                lock.joypad_mut().set_button(button, false);
                            }
                        }
                    }
                } else {
                    *control_flow = winit::event_loop::ControlFlow::WaitUntil(
                        std::time::Instant::now()
                            + std::time::Duration::from_millis(15)
                                .saturating_sub(last_redraw.elapsed()),
                    );
                }
            }
        }

        if last_redraw.elapsed() > std::time::Duration::from_millis(15) {
            window.request_redraw();
        }
    });
}
