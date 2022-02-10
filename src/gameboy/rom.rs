use binread::{io::Cursor, BinReaderExt};
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

const HEADER_LEN: usize = 0x014F - 0x0133 + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomCgbStatus {
    CGBOnly,
    CGBSupport,
    NoCGB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomSgbStatus {
    SGBSupport,
    NoSGB,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomMBCType {
    NoMBC,
    MBC1,
    MBC1Ram,
    MBC1RamBattery,
    Unknown,
}

/// Represents information regarding a [Rom].
#[derive(Debug, Clone)]
pub struct RomHeader {
    /// The title of the game.
    pub title: String,
    /// The manufacturer code.
    pub manufacturer: u32, // todo: turn into an enum
    /// Whether this rom supports CGB, and if it does, whether it is CGB only or not.
    pub cgb: RomCgbStatus,
    /// The license code.
    pub license: u16, // todo: turn into an enum
    /// Wether the game supports SGB functions.
    pub sgb: RomSgbStatus,
    /// Specifies which MBC is used in this rom, if any.
    pub rom_type: RomMBCType,
    /// The size of the rom, in bytes.
    pub rom_size: usize,
    /// The size of the external ram, if any, in bytes.
    pub ram_size: usize,
    /// Wether this version of the game was sold in Japan or not.
    pub japanese: bool,
    /// The old license code.
    pub old_license: u8, // todo: turn into an enum
    /// The version number of the rom.
    pub rom_version: u8,
    /// The header checksum.
    pub checksum: u8,
    /// The rom checksum.
    pub rom_checksum: u16,
}

impl RomHeader {
    /// Tries to decode a [RomHeader] instance from bytes.
    ///
    /// Exactly [HEADER_LEN] bytes are expected and an error is returned if the input length is wrong.
    pub fn try_from_bytes<B>(bytes: B) -> anyhow::Result<Self>
    where
        B: AsRef<[u8]>,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != HEADER_LEN {
            anyhow::bail!(
                "Wrong input length. Expected {}, got {}",
                HEADER_LEN,
                bytes.len()
            );
        }

        let title: String = bytes[0..16]
            .iter()
            .map_while(|x| {
                if x.is_ascii() && !x.is_ascii_control() {
                    Some(*x as char)
                } else {
                    None
                }
            })
            .collect();

        let mut reader = Cursor::new(&bytes[12..]);

        let manufacturer: u32 = reader.read_le()?;
        let cgb = match reader.read_le::<u8>()? {
            0x80 => RomCgbStatus::CGBSupport,
            0xC0 => RomCgbStatus::CGBOnly,
            _ => RomCgbStatus::NoCGB,
        };
        let license: u16 = reader.read_le()?;
        let sgb = match reader.read_le::<u8>()? {
            0x03 => RomSgbStatus::SGBSupport,
            _ => RomSgbStatus::NoSGB,
        };
        let rom_type = match reader.read_le::<u8>()? {
            0x00 => RomMBCType::NoMBC,
            0x01 => RomMBCType::MBC1,
            0x02 => RomMBCType::MBC1Ram,
            0x03 => RomMBCType::MBC1RamBattery,
            _ => RomMBCType::Unknown,
        };
        let rom_size = 32 * 2usize.pow(reader.read_le::<u8>()? as u32) * bytesize::KIB as usize;
        let ram_size = match reader.read_le::<u8>()? {
            0x00 => 0,
            0x02 => 8,   // 1 bank
            0x03 => 32,  // 4 banks of 8kb
            0x04 => 128, // 16 banks of 8kb
            0x05 => 64,  // 8 banks of 8kb
            _ => 128,    // unknown
        } * bytesize::KIB as usize;
        let japanese = reader.read_le::<u8>()? == 0;
        let old_license = reader.read_le::<u8>()?;
        let rom_version = reader.read_le::<u8>()?;
        let checksum = reader.read_le::<u8>()?;
        let rom_checksum: u16 = reader.read_le()?;

        Ok(Self {
            title,
            manufacturer,
            cgb,
            license,
            sgb,
            rom_type,
            rom_size,
            ram_size,
            japanese,
            old_license,
            rom_version,
            checksum,
            rom_checksum,
        })
    }
}

pub trait MemoryBankController {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8);
    fn external_read(&self, address: u16) -> u8;
    fn external_write(&mut self, address: u16, data: u8);
}

struct NoMBC {
    rom: Box<[u8]>, // 32KiB
    external: Box<[u8]>,
}

impl NoMBC {
    pub fn new(rom: Box<[u8]>, external: Box<[u8]>) -> Self {
        Self { rom, external }
    }
}

impl MemoryBankController for NoMBC {
    fn read(&self, address: u16) -> u8 {
        self.rom[address as usize]
    }

    fn write(&mut self, _address: u16, _data: u8) {
        // nothing!
    }

    fn external_read(&self, address: u16) -> u8 {
        self.external[address as usize]
    }

    fn external_write(&mut self, address: u16, data: u8) {
        self.external[address as usize] = data;
    }
}

struct MBC1 {
    rom: Box<[u8]>,      // Maximum 2MiB
    external: Box<[u8]>, // 32KiB
    rom_bank: u8,
    ram_bank: u8,
    ram_enabled: bool,
}

impl MBC1 {
    pub fn new(rom: Box<[u8]>, external: Box<[u8]>) -> Self {
        Self {
            rom,
            external,
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
        }
    }
}

impl MemoryBankController for MBC1 {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.rom[address as usize], // bank 00
            0x4000..=0x7FFF => {
                // bank 0N
                let bank_address = self.rom_bank as usize * 0x4000;
                let relative_address = address as usize - 0x4000;
                self.rom[bank_address + relative_address]
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            0x0000..=0x1FFF => {
                // ram enable/disable
                self.ram_enabled = data != 0x00;
            }
            0x2000..=0x3FFF => {
                // rom bank switching (first 5 bits)
                // let n_of_banks = self.rom.len() / 16 * bytesize::KIB as usize;
                let bank = data & 0b0001_1111;

                self.rom_bank &= !0b0001_1111;
                self.rom_bank |= match bank {
                    0x20 | 0x40 | 0x60 => bank | 1,
                    _ => bank,
                }
            }
            0x4000..=0x5FFF => {
                // ram bank switching
                if self.external.len() >= 32 * bytesize::KIB as usize {
                    self.ram_bank = data & 0b0000_0011;
                }

                // rom bank switch (2 bits after first 5)
                if self.rom.len() >= bytesize::MIB as usize {
                    self.rom_bank &= !(0b0000_0011 << 5);
                    self.rom_bank |= (data & 0b0000_0011) << 5;
                }
            }
            _ => (),
        }
    }

    fn external_read(&self, address: u16) -> u8 {
        if self.ram_enabled {
            let bank_address = self.ram_bank as usize * 0x2000;
            self.external[bank_address + address as usize]
        } else {
            0xFF
        }
    }

    fn external_write(&mut self, address: u16, data: u8) {
        if self.ram_enabled {
            let bank_address = self.ram_bank as usize * 0x2000;
            self.external[bank_address + address as usize] = data;
        }
    }

    // fn external_read(&self, address: u16) -> u8 {
    //     if self.ram_enabled {
    //         let bank_address = self.ram_bank as usize * 0x2000;
    //         self.external
    //             .get(bank_address + address as usize)
    //             .copied()
    //             .unwrap_or(0xFF)
    //     } else {
    //         0xFF
    //     }
    // }
    //
    // fn external_write(&mut self, address: u16, data: u8) {
    //     if self.ram_enabled {
    //         let bank_address = self.ram_bank as usize * 0x2000;
    //         if let Some(r) = self.external.get_mut(bank_address + address as usize) {
    //             *r = data;
    //         }
    //     }
    // }
}

/// Represents a gameboy game rom.
pub struct Rom {
    header: RomHeader,
    mbc: Box<dyn MemoryBankController + Sync + Send>,
}

impl Rom {
    pub fn try_from_bytes<'a, B>(bytes: B) -> anyhow::Result<Self>
    where
        B: Into<Cow<'a, [u8]>>,
    {
        let bytes: Cow<'a, [u8]> = bytes.into();
        if bytes.len() < 0x014F {
            anyhow::bail!("Rom too small to even contain a rom header");
        }

        let bytes: Box<[u8]> = bytes.into_owned().into();
        let header = RomHeader::try_from_bytes(&bytes[0x0133..=0x014F])?;

        if bytes.len() != header.rom_size {
            anyhow::bail!("Rom size doesn't match with size specified in it's header");
        }

        let external = vec![0xFFu8; header.ram_size * bytesize::KIB as usize].into();

        let mbc: Box<dyn MemoryBankController + Sync + Send> = match header.rom_type {
            RomMBCType::NoMBC => Box::new(NoMBC::new(bytes, external)),
            RomMBCType::MBC1 | RomMBCType::MBC1RamBattery => Box::new(MBC1::new(bytes, external)),
            _ => {
                anyhow::bail!("MBC not supported");
            }
        };

        Ok(Self { header, mbc })
    }

    pub fn header(&self) -> &RomHeader {
        &self.header
    }
}

impl Deref for Rom {
    type Target = Box<dyn MemoryBankController + Sync + Send>;

    fn deref(&self) -> &Self::Target {
        &self.mbc
    }
}

impl DerefMut for Rom {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mbc
    }
}
