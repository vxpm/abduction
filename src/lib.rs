#![feature(mixed_integer_ops)]
#![feature(trait_alias)]
#![feature(thread_is_running)]
#[allow(clippy::new_without_default)]
#[deny(clippy::perf)]
pub mod gameboy;
pub mod util;

#[cfg(feature = "tdebugger")]
pub mod tdebugger;

use clap::{ArgEnum, Parser};
use gameboy::Gameboy;
use parking_lot::Mutex;
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ArgEnum)]
pub enum ScreenPalette {
    Classic,     // from: https://lospec.com/palette-list/nintendo-gameboy-bgb
    Moonlight,   // from: https://lospec.com/palette-list/moonlight-gb
    Lava,        // from: https://lospec.com/palette-list/lava-gb
    Mist,        // from: https://lospec.com/palette-list/mist-gb
    Florescence, // from: https://lospec.com/palette-list/florescence
    Lollipop,    // from: https://lospec.com/palette-list/t-lollipop
    Crystal,     // from: https://lospec.com/palette-list/moon-crystal
    Autumn,      // from: https://lospec.com/palette-list/autumn-chill
    Metallic,    // from: https://lospec.com/palette-list/2bit-demichrome
    BlackAndWhite,
}

impl ScreenPalette {
    pub fn to_color_array(self) -> [hex_color::HexColor; 4] {
        match self {
            ScreenPalette::Classic => [
                "#081820".parse().unwrap(),
                "#346856".parse().unwrap(),
                "#88c070".parse().unwrap(),
                "#e0f8d0".parse().unwrap(),
            ],
            ScreenPalette::Moonlight => [
                "#0f052d".parse().unwrap(),
                "#203671".parse().unwrap(),
                "#36868f".parse().unwrap(),
                "#5fc75d".parse().unwrap(),
            ],
            ScreenPalette::Lava => [
                "#051f39".parse().unwrap(),
                "#4a2480".parse().unwrap(),
                "#c53a9d".parse().unwrap(),
                "#ff8e80".parse().unwrap(),
            ],
            ScreenPalette::Mist => [
                "#2d1b00".parse().unwrap(),
                "#1e606e".parse().unwrap(),
                "#5ab9a8".parse().unwrap(),
                "#c4f0c2".parse().unwrap(),
            ],
            ScreenPalette::Florescence => [
                "#311f5f".parse().unwrap(),
                "#1687a7".parse().unwrap(),
                "#1fd5bc".parse().unwrap(),
                "#edffb1".parse().unwrap(),
            ],
            ScreenPalette::Lollipop => [
                "#151640".parse().unwrap(),
                "#3f6d9e".parse().unwrap(),
                "#f783b0".parse().unwrap(),
                "#e6f2ef".parse().unwrap(),
            ],
            ScreenPalette::Crystal => [
                "#755f9c".parse().unwrap(),
                "#8d89c7".parse().unwrap(),
                "#d9a7c6".parse().unwrap(),
                "#ffe2db".parse().unwrap(),
            ],
            ScreenPalette::Autumn => [
                "#2c1e74".parse().unwrap(),
                "#c23a73".parse().unwrap(),
                "#d58863".parse().unwrap(),
                "#dad3af".parse().unwrap(),
            ],
            ScreenPalette::Metallic => [
                "#211e20".parse().unwrap(),
                "#555568".parse().unwrap(),
                "#a0a08b".parse().unwrap(),
                "#e9efec".parse().unwrap(),
            ],
            ScreenPalette::BlackAndWhite => [
                "#000000".parse().unwrap(),
                "#555555".parse().unwrap(),
                "#AAAAAA".parse().unwrap(),
                "#FFFFFF".parse().unwrap(),
            ],
        }
    }
}

/// a gameboy emulator, because there sure aren't enough of these out there already!
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct AbductionArgs {
    /// Path to the game ROM
    #[clap(short, long)]
    pub rom: String,

    /// Path to the boot ROM to utilize
    #[clap(short, long, default_value = "boot.gb")]
    pub boot: String,

    /// When passed, abduction will print the rom header instead of running
    #[clap(short, long)]
    pub header: bool,

    /// Screen pallete to use
    #[clap(arg_enum, default_value = "classic")]
    pub palette: ScreenPalette,

    /// Window size multiplier
    #[clap(short, long, default_value = "4")]
    pub size_multiplier: u8,

    /// How long a machine cycle should take to execute, in nanoseconds
    #[clap(short, long, default_value = "953")]
    pub cycle_duration_ns: u64,
}

pub fn lib_main(args: AbductionArgs) -> anyhow::Result<()> {
    if args.header {
        let rom = crate::util::read_bytes(args.rom)?;
        let header = gameboy::rom::RomHeader::try_from_bytes(&rom[0x0133..=0x014F])?;
        println!("{:#?}", header);
        Ok(())
    } else {
        run(args)
    }
}

pub fn run(args: AbductionArgs) -> anyhow::Result<()> {
    // create shared state
    let rom = crate::util::read_bytes(args.rom)?;
    let boot = crate::util::read_bytes(args.boot)?;

    let gameboy = Mutex::new(Gameboy::new(rom, boot)?);
    let shared = Arc::new((gameboy, AtomicBool::new(false)));

    // spawn thread for gameboy
    let shared_clone = shared.clone();
    let res = std::thread::spawn(move || {
        let shared = shared_clone;
        let mut m_cycles;

        loop {
            m_cycles = 0;
            let before = std::time::Instant::now();

            if shared.1.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let mut lock = shared.0.lock();
            for _ in 0..4 {
                m_cycles += lock.step();
            }

            let frame_time: std::time::Duration =
                std::time::Duration::from_nanos(m_cycles as u64 * args.cycle_duration_ns);

            // TODO: make this better? idk sleeping would be nice
            while !frame_time.saturating_sub(before.elapsed()).is_zero() {
                std::hint::spin_loop();
            }
        }
    });

    // open window
    let event_loop = winit::event_loop::EventLoop::new();
    let mut input = winit_input_helper::WinitInputHelper::new();
    let window = {
        let size = winit::dpi::LogicalSize::new(
            160u16 * args.size_multiplier.max(1) as u16,
            144u16 * args.size_multiplier.max(1) as u16,
        );
        winit::window::WindowBuilder::new()
            .with_title(format!(
                "abduction - {}",
                shared.0.lock().memory().rom_header().title
            ))
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
    let color_array = args.palette.to_color_array();
    let mut last_redraw = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        if !res.is_running() {
            *control_flow = winit::event_loop::ControlFlow::Exit;
        }

        match event {
            winit::event::Event::RedrawRequested(_) => {
                {
                    let lock = shared.0.lock();
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

                        // let c = match v {
                        //     3 => [0x1B, 0x03, 0x26, 0xFF],
                        //     2 => [0x7A, 0x1C, 0x4B, 0xFF],
                        //     1 => [0xBA, 0x50, 0x44, 0xFF],
                        //     0 => [0xDC, 0xBC, 0xA1, 0xFF],
                        //     _ => unreachable!(),
                        // };

                        let color = color_array[3 - v as usize];
                        let rgba = [color.r, color.g, color.b, 0xFF];

                        pixel.copy_from_slice(&rgba);
                    }

                    last_redraw = std::time::Instant::now();
                }

                if pixels.render().is_err() {
                    shared.1.store(true, std::sync::atomic::Ordering::SeqCst);
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
            }
            _ => {
                if input.update(&event) {
                    // Close events
                    if input.key_pressed(winit::event::VirtualKeyCode::Escape) || input.quit() {
                        shared.1.store(true, std::sync::atomic::Ordering::SeqCst);
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
                        let mut lock = shared.0.lock();
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
