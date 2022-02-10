pub mod apu;
pub mod cpu;
pub mod memory;
pub mod ppu;
pub mod rom;
pub mod timer;

use std::borrow::Cow;

use apu::*;
use cpu::*;
use memory::*;
use ppu::*;
use rom::*;
use timer::*;

pub enum JoypadButton {
    Right = 0b0000_0001,
    Left = 0b0000_0010,
    Up = 0b0000_0100,
    Down = 0b0000_1000,
    A = 0b0001_0000,
    B = 0b0010_0000,
    Select = 0b0100_0000,
    Start = 0b1000_0000,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Joypad {
    data: u8,
}

impl Joypad {
    pub fn new() -> Self {
        Self { data: 0b1110_0000 }
    }

    pub fn set_button(&mut self, button: JoypadButton, value: bool) {
        if value {
            self.data |= button as u8;
        } else {
            self.data &= !(button as u8);
        }
    }

    pub fn action_buttons(&self) -> u8 {
        (self.data & 0xF0) >> 4
    }

    pub fn directional_buttons(&self) -> u8 {
        self.data & 0x0F
    }
}

/// A Gameboy emulator.
pub struct Gameboy {
    memory: Memory,
    cpu: Cpu,
    ppu: Ppu,
    apu: Apu,
    timer: Timer,
    joypad: Joypad,
}

impl Gameboy {
    /// Returns a new gameboy emulator instance with the given rom and bootrom.
    pub fn new<'a, R, B>(rom: R, boot: B) -> anyhow::Result<Self>
    where
        R: Into<Cow<'a, [u8]>>,
        B: Into<Box<[u8]>>,
    {
        let rom = Rom::try_from_bytes(rom)?;

        let mut memory = Memory::new(rom, boot.into());
        let cpu = Cpu::new();
        let ppu = Ppu::new(&mut memory);
        let apu = Apu::new();
        let timer = Timer::new();
        let joypad = Joypad::new();

        Ok(Self {
            memory,
            cpu,
            ppu,
            apu,
            timer,
            joypad,
        })
    }

    /// Steps the emulation forward by 1 cpu step. Returns how many machine cycles have been executed.
    pub fn step(&mut self) -> u8 {
        let mut m_cycles: u8 = 0;
        self.cpu.step(&mut self.memory, &mut |memory: &mut Memory| {
            // one machine cycle is 4 clock cycles
            for _ in 0..4 {
                self.ppu.cycle(memory);
                self.apu.cycle(memory);
                self.timer.cycle(memory);
            }

            // update joypad register
            let joyp = !memory.read(registers::addresses::JOYP);
            let updated = if joyp & (1 << 4) == 0 {
                self.joypad.action_buttons()
            } else {
                self.joypad.directional_buttons()
            };

            memory.write(registers::addresses::JOYP, !((joyp & 0xF0) | updated));
            if joyp & 0x0F != updated {
                memory.request_interrupt(registers::Interrupt::Joypad);
            }

            m_cycles += 1;
        });

        m_cycles
    }

    /// Returns an reference to the [Cpu] instance of this emulator.
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    /// Returns an reference to the [Ppu] instance of this emulator.
    pub fn ppu(&self) -> &Ppu {
        &self.ppu
    }

    /// Returns an reference to the [Memory] instance of this emulator.
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Returns an reference to the [Joypad] instance of this emulator.
    pub fn joypad_mut(&mut self) -> &mut Joypad {
        &mut self.joypad
    }
}
