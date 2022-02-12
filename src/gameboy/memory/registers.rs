use crate::gameboy::ppu::Tilemap;
use flagset::flags;

/// Adresses for memory mapped registers.
pub mod addresses {
    pub const INTERRUPT_ENABLE: u16 = 0xFFFF;
    pub const INTERRUPT_REQUEST: u16 = 0xFF0F;
    pub const LCDC: u16 = 0xFF40;
    pub const STAT: u16 = 0xFF41;
    pub const SCY: u16 = 0xFF42;
    pub const SCX: u16 = 0xFF43;
    pub const LY: u16 = 0xFF44;
    pub const LYC: u16 = 0xFF45;
    pub const WY: u16 = 0xFF4A;
    pub const WX: u16 = 0xFF4B;
    pub const BGP: u16 = 0xFF47;
    pub const OBP0: u16 = 0xFF48;
    pub const OBP1: u16 = 0xFF49;
    pub const DMA: u16 = 0xFF46;
    pub const DIV: u16 = 0xFF04;
    pub const TIMA: u16 = 0xFF05;
    pub const TMA: u16 = 0xFF06;
    pub const TAC: u16 = 0xFF07;
    pub const JOYP: u16 = 0xFF00;

    pub const NR10: u16 = 0xFF10;
    pub const NR11: u16 = 0xFF11;
    pub const NR12: u16 = 0xFF12;
    pub const NR13: u16 = 0xFF13;
    pub const NR14: u16 = 0xFF14;

    pub const NR21: u16 = 0xFF16;
    pub const NR22: u16 = 0xFF17;
    pub const NR23: u16 = 0xFF18;
    pub const NR24: u16 = 0xFF19;

    pub const NR30: u16 = 0xFF1A;
    pub const NR31: u16 = 0xFF1B;
    pub const NR32: u16 = 0xFF1C;
    pub const NR33: u16 = 0xFF1D;
    pub const NR34: u16 = 0xFF1E;

    pub const NR41: u16 = 0xFF20;
    pub const NR42: u16 = 0xFF21;
    pub const NR43: u16 = 0xFF22;
    pub const NR44: u16 = 0xFF23;

    pub const NR50: u16 = 0xFF24;
    pub const NR51: u16 = 0xFF25;
    pub const NR52: u16 = 0xFF26;
}

flags! {
    /// Can represent the flags in both InterruptEnable (0xFFFF) and InterruptFlag/Request (0xFF0F) registers.
    pub enum Interrupt: u8 {
        VBlank = 0b0000_0001,
        STAT = 0b0000_0010,
        Timer = 0b0000_0100,
        Serial = 0b0000_1000,
        Joypad = 0b0001_0000,
    }
}

/// Represents an instance of the state of the LCDC register.
pub struct LCDC {
    inner: u8,
}

impl From<u8> for LCDC {
    fn from(data: u8) -> Self {
        let inner = data;
        Self { inner }
    }
}

impl LCDC {
    pub fn background_window_priority(&self) -> bool {
        self.inner & (1 << 0) != 0
    }

    pub fn objects_enabled(&self) -> bool {
        self.inner & (1 << 1) != 0
    }

    pub fn double_height_objects(&self) -> bool {
        self.inner & (1 << 2) != 0
    }

    pub fn background_tilemap(&self) -> Tilemap {
        if self.inner & (1 << 3) != 0 {
            Tilemap::Tilemap1
        } else {
            Tilemap::Tilemap0
        }
    }

    pub fn alternative_addressing_mode(&self) -> bool {
        self.inner & (1 << 4) == 0
    }

    pub fn window_enabled(&self) -> bool {
        self.inner & (1 << 5) != 0
    }

    pub fn window_tilemap(&self) -> Tilemap {
        if self.inner & (1 << 6) != 0 {
            Tilemap::Tilemap1
        } else {
            Tilemap::Tilemap0
        }
    }

    pub fn screen_enabled(&self) -> bool {
        self.inner & (1 << 7) != 0
    }
}

flags! {
    /// Represents the flags in the STAT (0xFF41) register.
    pub enum StatFlag: u8 {
        LYCEqualsLY = 0b0000_0100,
        HBlankInterruptEnabled = 0b0000_1000,
        VBlankInterruptEnabled = 0b0001_0000,
        OAMInterruptEnabled = 0b0010_0000,
        LYCEqualsLYInterruptEnabled = 0b0100_0000,

        // not individual flags
        /// Alias for `RenderingMode`
        ModeBits = 0b0000_0011,
        HBlankMode = 0b0000_0000,
        VBlankMode = 0b0000_0001,
        OAMSearchMode = 0b0000_0010,
        RenderingMode = 0b0000_0011,
    }
}
