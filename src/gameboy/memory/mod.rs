pub mod registers;
use std::ops::Deref;

use super::rom::*;

/// Trait for memory components of the gameboy.
pub trait GameboyMemory {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8);
}

pub trait Vram: GameboyMemory {
    fn as_slice(&self) -> &[u8];
}

pub struct DMGVram {
    data: Box<[u8; 8 * bytesize::KIB as usize]>,
}

impl Default for DMGVram {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 8 * bytesize::KIB as usize]),
        }
    }
}

impl GameboyMemory for DMGVram {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

impl Vram for DMGVram {
    fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }
}

// TODO: fix implementation with banking
pub struct CGBVram {
    data: Box<[u8; 16 * bytesize::KIB as usize]>,
}

impl Default for CGBVram {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 16 * bytesize::KIB as usize]),
        }
    }
}

impl GameboyMemory for CGBVram {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

impl Vram for CGBVram {
    fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }
}

pub struct DMGWram {
    data: Box<[u8; 8 * bytesize::KIB as usize]>,
}

impl Default for DMGWram {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 8 * bytesize::KIB as usize]),
        }
    }
}

impl GameboyMemory for DMGWram {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

pub struct CGBWram {
    data: Box<[u8; 32 * bytesize::KIB as usize]>,
}

impl Default for CGBWram {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 32 * bytesize::KIB as usize]),
        }
    }
}

// TODO: fix implementation with banking
impl GameboyMemory for CGBWram {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

pub struct Oam {
    data: Box<[u8; 160]>,
}

impl Default for Oam {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 160]),
        }
    }
}

impl GameboyMemory for Oam {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

impl Deref for Oam {
    type Target = Box<[u8; 160]>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

pub struct IORegisters {
    data: Box<[u8; 128]>,
}

impl Default for IORegisters {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 128]),
        }
    }
}

impl GameboyMemory for IORegisters {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

pub struct Hram {
    data: Box<[u8; 128]>,
}

impl Default for Hram {
    fn default() -> Self {
        Self {
            data: Box::new([0xFFu8; 128]),
        }
    }
}

impl GameboyMemory for Hram {
    fn read(&self, address: u16) -> u8 {
        self.data[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.data[address as usize] = data;
    }
}

/// A Gameboy memory component.
pub struct Memory {
    boot_mode: bool,
    boot: Box<[u8]>,
    rom: Rom,
    vram: Box<dyn Vram + Sync + Send>,
    wram: Box<dyn GameboyMemory + Sync + Send>,
    oam: Oam,
    io_registers: IORegisters,
    hram: Hram,
}

impl Memory {
    pub fn new(rom: Rom, boot: Box<[u8]>) -> Self {
        // here we already have the rom, so we can already decide if we should use CGB mode or not (and etc)!
        match rom.header().cgb {
            RomCgbStatus::CGBOnly | RomCgbStatus::CGBSupport => Self {
                boot_mode: true,
                boot,
                rom,
                vram: Box::new(CGBVram::default()),
                wram: Box::new(CGBWram::default()),
                oam: Oam::default(),
                io_registers: IORegisters::default(),
                hram: Hram::default(),
            },
            RomCgbStatus::NoCGB => Self {
                boot_mode: true,
                boot,
                rom,
                vram: Box::new(DMGVram::default()),
                wram: Box::new(DMGWram::default()),
                oam: Oam::default(),
                io_registers: IORegisters::default(),
                hram: Hram::default(),
            },
        }
    }

    /// Reads a value from memory.
    ///
    /// Adresses 0xFEA0..=0xFEFF always return 0xFF.
    #[inline]
    pub fn read(&self, address: u16) -> u8 {
        // TODO: this boot mode behaviour doesn't take CGB into consideration
        if self.boot_mode && address <= 0xFF {
            return self.boot[address as usize];
        }

        match address {
            0x0000..=0x3FFF => self.rom.read(address), // rom bank 00 (fixed)
            0x4000..=0x7FFF => self.rom.read(address), // rom bank 01 / NN (switchable)
            0x8000..=0x9FFF => self.vram.read(address - 0x8000), // vram | in cgb, switchable bank 0/1
            0xA000..=0xBFFF => self.rom.external_read(address), // external ram (switchable bank if any)
            0xC000..=0xCFFF => self.wram.read(address - 0xC000), // wram | in cgb, bank 0
            0xD000..=0xDFFF => self.wram.read(address - 0xC000), // wram | in cgb, switchable bank 1-7
            0xE000..=0xFDFF => self.wram.read(address - 0xE000), // echo ram, mirror of C000~DDFF
            0xFE00..=0xFE9F => self.oam.read(address - 0xFE00),  // sprite attribute table (oam)
            0xFEA0..=0xFEFF => 0xFF,                             // unused
            0xFF00..=0xFF7F => self.io_registers.read(address - 0xFF00), // I/O registers
            0xFF80..=0xFFFF => self.hram.read(address - 0xFF80), // high ram (hram)
        }
    }

    /// Writes a value to memory.
    ///
    /// Writes to adresses 0xFEA0..=0xFEFF have no effect and writing any value to 0xFF50 while boot mode is on turns it off.
    #[inline]
    pub fn write(&mut self, address: u16, data: u8) {
        if self.boot_mode && address == 0xFF50 {
            // disable boot mode
            self.boot_mode = false;
        }

        if address == registers::addresses::DMA {
            let source = ((data as u16) << 8)..=(((data as u16) << 8) | 0x9F);
            for (oam_index, source_index) in source.enumerate() {
                self.oam.write(oam_index as u16, self.read(source_index));
            }
        }

        match address {
            0x0000..=0x3FFF => self.rom.write(address, data), // rom bank 00 (fixed)
            0x4000..=0x7FFF => self.rom.write(address, data), // rom bank 01 / NN (switchable)
            0x8000..=0x9FFF => self.vram.write(address - 0x8000, data), // vram | in cgb, switchable bank 0/1
            0xA000..=0xBFFF => self.rom.external_write(address, data), // external ram (switchable bank if any)
            0xC000..=0xCFFF => self.wram.write(address - 0xC000, data), // wram | in cgb, bank 0
            0xD000..=0xDFFF => self.wram.write(address - 0xC000, data), // wram | in cgb, switchable bank 1-7
            0xE000..=0xFDFF => self.wram.write(address - 0xE000, data), // echo ram, mirror of C000~DDFF
            0xFE00..=0xFE9F => self.oam.write(address - 0xFE00, data), // sprite attribute table (oam)
            0xFEA0..=0xFEFF => (),                                     // unused
            0xFF00..=0xFF7F => self.io_registers.write(address - 0xFF00, data), // I/O registers
            0xFF80..=0xFFFF => self.hram.write(address - 0xFF80, data), // high ram (hram)
        }
    }

    /// Requests an interrupt by turning the corresponding bit in the interrupt request register on.
    #[inline]
    pub fn request_interrupt(&mut self, interrupt: registers::Interrupt) {
        let current = self.read(registers::addresses::INTERRUPT_REQUEST);
        self.write(
            registers::addresses::INTERRUPT_REQUEST,
            current | (!!interrupt).bits(), // TODO: fix this hacky method
        );
    }

    /// Whether boot mode is active or not.
    pub fn boot_mode(&self) -> bool {
        self.boot_mode
    }

    pub fn oam(&self) -> &Oam {
        &self.oam
    }

    pub fn vram(&self) -> &(dyn Vram + Sync + Send) {
        &*self.vram
    }

    pub fn rom_header(&self) -> &RomHeader {
        self.rom.header()
    }
}
