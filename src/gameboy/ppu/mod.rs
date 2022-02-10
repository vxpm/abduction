use super::memory::registers as memreg;
use super::memory::Memory;
use flagset::{flags, FlagSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PPUMode {
    HBlank,
    VBlank,
    OAMSearch,
    Rendering,
}

impl PPUMode {
    #[inline]
    pub fn into_stat_flag(self) -> memreg::StatFlag {
        match self {
            PPUMode::HBlank => memreg::StatFlag::HBlankMode,
            PPUMode::VBlank => memreg::StatFlag::VBlankMode,
            PPUMode::OAMSearch => memreg::StatFlag::OAMSearchMode,
            PPUMode::Rendering => memreg::StatFlag::RenderingMode,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tile {
    bytes: [u8; 16],
}

impl Tile {
    pub fn new(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    pub fn get_pixel_color_index(&self, x: u8, y: u8) -> anyhow::Result<u8> {
        if !((0..8).contains(&x) && (0..8).contains(&y)) {
            anyhow::bail!("Pixel position ({}, {}) out of range", x, y);
        }

        let index = y as usize * 2;
        let bytes = &self.bytes[index..=index + 1];

        let offset = 7 - x;
        let low_bit = (bytes[0] >> offset) & 1;
        let high_bit = (bytes[1] >> offset) & 1;
        let pixel = (high_bit << 1) | low_bit;

        Ok(pixel)
    }
}

pub enum Tilemap {
    /// Resides at 0x9800..=0x9BFF
    Tilemap0,
    /// Resides at 0x9C00..=0x9FFF
    Tilemap1,
}

pub enum Tileset {
    /// Resides at 0x8000..=0x87FF
    Tileset0,
    /// Resides at 0x8800..=0x8FFF
    Tileset1,
    /// Resides at 0x9000..=0x97FF
    Tileset2,
}

flags! {
    /// Represents the flags in the STAT (0xFF41) register.
    pub enum ObjectAttributesFlags: u8 {
        CgbPaletteBit0 = 0b0000_0001,
        CgbPaletteBit1 = 0b0000_0010,
        CgbPaletteBit2 = 0b0000_0100,
        VramBank = 0b0000_1000,
        DmgPalette = 0b0001_0000,
        FlipX = 0b0010_0000,
        FlipY = 0b0100_0000,
        UnderBgWindow = 0b1000_0000,

        // not individual flags
        CgbPaletteBits = 0b0000_0111,
    }
}

#[derive(Debug)]
struct ObjectAttributes {
    pub x: u8,
    pub y: u8,
    pub tile_index: u8,
    pub flags: FlagSet<ObjectAttributesFlags>,
}

impl ObjectAttributes {
    pub fn new(bytes: [u8; 4]) -> anyhow::Result<Self> {
        Ok(Self {
            x: bytes[1],
            y: bytes[0],
            tile_index: bytes[2],
            flags: FlagSet::new(bytes[3]).map_err(|_| anyhow::anyhow!("Invalid bits"))?,
        })
    }

    pub fn x_top_left(&self) -> i16 {
        self.x as i16 - 8
    }

    pub fn y_top_left(&self) -> i16 {
        self.y as i16 - 16
    }

    pub fn flip_x(&self) -> bool {
        self.flags.contains(ObjectAttributesFlags::FlipX)
    }

    pub fn flip_y(&self) -> bool {
        self.flags.contains(ObjectAttributesFlags::FlipY)
    }

    pub fn vram_bank(&self) -> u8 {
        if self.flags.contains(ObjectAttributesFlags::VramBank) {
            1
        } else {
            0
        }
    }

    pub fn dmg_palette(&self) -> u8 {
        if self.flags.contains(ObjectAttributesFlags::DmgPalette) {
            1
        } else {
            0
        }
    }

    pub fn cgb_palette(&self) -> u8 {
        (self.flags & ObjectAttributesFlags::CgbPaletteBits).bits()
    }

    pub fn under_bg_window(&self) -> bool {
        self.flags.contains(ObjectAttributesFlags::UnderBgWindow)
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    data: u8,
}

impl Palette {
    pub fn new(data: u8) -> Self {
        Self::from(data)
    }

    pub fn color_0(&self) -> u8 {
        self.data & 0b0000_0011
    }

    pub fn color_1(&self) -> u8 {
        (self.data & 0b0000_1100) >> 2
    }

    pub fn color_2(&self) -> u8 {
        (self.data & 0b0011_0000) >> 4
    }

    pub fn color_3(&self) -> u8 {
        (self.data & 0b1100_0000) >> 6
    }
}

impl From<u8> for Palette {
    fn from(data: u8) -> Self {
        Self { data }
    }
}

struct BackgroundPixel {
    pub color_index: u8,
}

struct ObjectPixel {
    pub color_index: u8,
    pub palette: u8,
    pub under_bg_window: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ScreenBuffer {
    pixels: [u8; 160 * 144],
}

impl ScreenBuffer {
    pub fn new() -> Self {
        Self {
            pixels: [0; 160 * 144],
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> anyhow::Result<u8> {
        if !((0..160).contains(&x) && (0..144).contains(&y)) {
            anyhow::bail!("Pixel position ({}, {}) out of range", x, y);
        }

        let index = y * 160 + x;
        Ok(self.pixels[index])
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, value: u8) -> anyhow::Result<()> {
        if !((0..160).contains(&x) && (0..144).contains(&y)) {
            anyhow::bail!("Pixel position ({}, {}) out of range", x, y);
        }

        let index = y * 160 + x;
        self.pixels[index] = value;

        Ok(())
    }

    pub fn clear(&mut self) {
        for pixel in self.pixels.iter_mut() {
            *pixel = 0;
        }
    }
}

pub struct ScreenDoubleBuffer {
    buffers: [Box<ScreenBuffer>; 2],
    back: usize,
}

impl ScreenDoubleBuffer {
    pub fn new() -> Self {
        ScreenDoubleBuffer {
            buffers: [Box::new(ScreenBuffer::new()), Box::new(ScreenBuffer::new())],
            back: 0,
        }
    }

    pub fn front(&self) -> &ScreenBuffer {
        &self.buffers[1 - self.back]
    }

    pub fn back(&self) -> &ScreenBuffer {
        &self.buffers[self.back]
    }

    pub fn back_mut(&mut self) -> &mut ScreenBuffer {
        &mut self.buffers[self.back]
    }

    pub fn switch(&mut self) {
        self.back = 1 - self.back;
    }
}

pub struct Ppu {
    cycles: u16,
    mode: PPUMode,
    interrupt_ongoing: bool,
    buffers: ScreenDoubleBuffer,
    master_tileset: Box<[Tile; 384]>,
    tilemap0: Box<[u8; 1024]>,
    tilemap1: Box<[u8; 1024]>,
    scanline_objects: Vec<ObjectAttributes>,
    window_line_counter: u8,
}

impl Ppu {
    pub fn new(memory: &mut Memory) -> Self {
        let stat_flags: FlagSet<_> = memreg::StatFlag::OAMSearchMode.into();

        memory.write(memreg::addresses::STAT, stat_flags.bits());
        memory.write(memreg::addresses::LY, 0x00);
        memory.write(memreg::addresses::SCX, 0x00);
        memory.write(memreg::addresses::SCY, 0x00);

        Self {
            cycles: 0,
            mode: PPUMode::OAMSearch,
            interrupt_ongoing: false,
            buffers: ScreenDoubleBuffer::new(),
            master_tileset: crate::util::boxed_array(Tile::default()),
            tilemap0: crate::util::boxed_array(0u8),
            tilemap1: crate::util::boxed_array(0u8),
            scanline_objects: Vec::with_capacity(10),
            window_line_counter: 0,
        }
    }

    #[inline]
    fn get_lcdc(memory: &Memory) -> memreg::LCDC {
        memreg::LCDC::from(memory.read(memreg::addresses::LCDC))
    }

    #[inline]
    fn get_stat(memory: &Memory) -> FlagSet<memreg::StatFlag> {
        let bits = memory.read(memreg::addresses::STAT);
        FlagSet::<memreg::StatFlag>::new(bits & 0b0111_1111).unwrap()
    }

    #[inline]
    fn set_stat_flag(memory: &mut Memory, flag: memreg::StatFlag, value: bool) {
        let bits = memory.read(memreg::addresses::STAT);
        memory.write(
            memreg::addresses::STAT,
            if value {
                bits | (!!flag).bits() // TODO: fix this hacky method
            } else {
                bits & (!flag).bits()
            },
        );
    }

    #[inline]
    fn set_mode(&mut self, memory: &mut Memory, mode: PPUMode) {
        self.mode = mode;
        let flag = mode.into_stat_flag();

        Self::set_stat_flag(memory, memreg::StatFlag::ModeBits, false);
        Self::set_stat_flag(memory, flag, true);
    }

    #[inline]
    fn update_stat_interrupt(&mut self, memory: &mut Memory) {
        let stat = Self::get_stat(memory);
        let ly = memory.read(memreg::addresses::LY);
        let lyc = memory.read(memreg::addresses::LYC);
        let check = [
            (
                memreg::StatFlag::HBlankInterruptEnabled,
                self.mode == PPUMode::HBlank,
            ),
            (
                memreg::StatFlag::VBlankInterruptEnabled,
                self.mode == PPUMode::VBlank,
            ),
            (
                memreg::StatFlag::OAMInterruptEnabled,
                self.mode == PPUMode::OAMSearch,
            ),
            (memreg::StatFlag::LYCEqualsLYInterruptEnabled, ly == lyc),
        ];

        // caso o interrupt_ongoing seja verdadeiro, esperar até o check for completamente falso para poder
        // requisitar o interrupt. caso o interrupt_ongoing seja falso e um interrupt for verdadeiro, tornar interrupt_ongoing
        // verdadeiro e requisitar o interrupt

        let cond = check
            .into_iter()
            .filter(|(flag, _)| stat.contains(*flag))
            .any(|(_, condition)| condition);
        if self.interrupt_ongoing {
            self.interrupt_ongoing = cond;
        } else if cond {
            self.interrupt_ongoing = true;
            memory.request_interrupt(memreg::Interrupt::STAT);
        }
    }

    pub fn screen(&self) -> &ScreenBuffer {
        self.buffers.front()
    }

    #[inline]
    fn increment_ly(memory: &mut Memory) {
        let new_ly = (memory.read(memreg::addresses::LY) + 1) % 154;
        memory.write(memreg::addresses::LY, new_ly);
    }

    #[inline]
    fn update_master_tileset(&mut self, memory: &mut Memory) {
        let vram_tileset = &memory.vram().as_slice()[..0x1800];
        for (i, chunk) in vram_tileset.chunks_exact(16).enumerate() {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(chunk);

            let tile = Tile::new(bytes);
            self.master_tileset[i] = tile;
        }
    }

    #[inline]
    fn update_tilemaps(&mut self, memory: &mut Memory) {
        let vram = memory.vram().as_slice();
        let vram_tilemap0 = &vram[0x9800 - 0x8000..0x9C00 - 0x8000];
        let vram_tilemap1 = &vram[0x9C00 - 0x8000..0xA000 - 0x8000];

        self.tilemap0.copy_from_slice(vram_tilemap0);
        self.tilemap1.copy_from_slice(vram_tilemap1);
    }

    fn oam_search(&mut self, memory: &mut Memory) {
        self.scanline_objects.clear();
        let ly = memory.read(memreg::addresses::LY);
        let lcdc = Self::get_lcdc(memory);

        let oam = &memory.oam()[..];
        for chunk in oam.chunks_exact(4) {
            let mut bytes = [0; 4];
            bytes.copy_from_slice(chunk);

            if lcdc.double_height_objects() {
                bytes[2] &= 0b1111_1110;
            }

            let ly = ly as i16;
            let obj_attributes = ObjectAttributes::new(bytes).unwrap();
            if obj_attributes.y_top_left() > ly - if lcdc.double_height_objects() { 16 } else { 8 }
                && obj_attributes.y_top_left() <= ly
            {
                self.scanline_objects.push(obj_attributes);

                if self.scanline_objects.len() >= 10 {
                    return;
                }
            }
        }
    }

    #[inline]
    fn get_bg_pixel(&self, memory: &mut Memory, pixel_position: (u8, u8)) -> BackgroundPixel {
        // algoritmo:
        //  -> transformar a posição do pixel do screen space para o tilemap space
        //  -> transformar a posição do pixel do tilemap space para tile space
        //  -> colocar o pixel no buffer

        let lcdc = Self::get_lcdc(memory);
        let scx = memory.read(memreg::addresses::SCX);
        let scy = memory.read(memreg::addresses::SCY);
        let bg_tilemap = match lcdc.background_tilemap() {
            Tilemap::Tilemap0 => &self.tilemap0,
            Tilemap::Tilemap1 => &self.tilemap1,
        };

        // convertendo para tilemap space
        let pixel_position_tilemap = (
            pixel_position.0.wrapping_add(scx),
            pixel_position.1.wrapping_add(scy),
        );

        // convertendo para tile space
        let (tile_position_tilemap, pixel_position_tile) = {
            let (tile_position_x, pixel_position_x) =
                crate::util::div_rem(pixel_position_tilemap.0, 8);
            let (tile_position_y, pixel_position_y) =
                crate::util::div_rem(pixel_position_tilemap.1, 8);
            (
                (tile_position_x, tile_position_y),
                (pixel_position_x, pixel_position_y),
            )
        };

        // convertendo a posiçao do pixel em tile space para a posiçao no tilemap do tile que o contem e a posiçao relativa do pixel ao tile
        let tile_index_tilemap =
            tile_position_tilemap.1 as usize * 32 + tile_position_tilemap.0 as usize;
        let tile_tileset_index = bg_tilemap[tile_index_tilemap];

        let tile = if lcdc.alternative_addressing_mode() {
            match tile_tileset_index {
                0..=127 => &self.master_tileset[0x1000 / 16 + tile_tileset_index as usize],
                128..=255 => &self.master_tileset[tile_tileset_index as usize],
            }
        } else {
            &self.master_tileset[tile_tileset_index as usize]
        };

        // obtendo a cor do pixel
        let color_index = tile
            .get_pixel_color_index(pixel_position_tile.0, pixel_position_tile.1)
            .unwrap();

        BackgroundPixel { color_index }
    }

    #[inline]
    fn get_obj_pixel(&self, memory: &mut Memory, pixel_position: (u8, u8)) -> Option<ObjectPixel> {
        let pixel_position = (pixel_position.0 as i16, pixel_position.1 as i16);
        let lcdc = Self::get_lcdc(memory);

        let mut objs: smallvec::SmallVec<[&ObjectAttributes; 10]> = self
            .scanline_objects
            .iter()
            .filter(|obj| {
                pixel_position.0 >= obj.x_top_left() && pixel_position.0 < obj.x_top_left() + 8
            })
            .collect();

        objs.sort_by_key(|obj| obj.x_top_left());

        let mut obj_pixel = None;
        for obj in objs {
            let mut pixel_position_tile = (
                pixel_position.0 - obj.x_top_left(),
                pixel_position.1 - obj.y_top_left(),
            );

            let tile = if lcdc.double_height_objects() {
                if obj.flip_y() {
                    pixel_position_tile.1 = 15 - pixel_position_tile.1;
                }

                if pixel_position_tile.1 > 7 {
                    pixel_position_tile.1 -= 8;
                    &self.master_tileset[obj.tile_index as usize + 1]
                } else {
                    &self.master_tileset[obj.tile_index as usize]
                }
            } else {
                if obj.flip_y() {
                    pixel_position_tile.1 = 7 - pixel_position_tile.1;
                }
                &self.master_tileset[obj.tile_index as usize]
            };

            if obj.flip_x() {
                pixel_position_tile.0 = 7 - pixel_position_tile.0;
            }

            // obtendo o índice da cor do pixel
            let color_index = tile
                .get_pixel_color_index(pixel_position_tile.0 as u8, pixel_position_tile.1 as u8)
                .unwrap();

            if color_index != 0 {
                obj_pixel = Some(ObjectPixel {
                    color_index,
                    palette: obj.dmg_palette(),
                    under_bg_window: obj.under_bg_window(),
                });
                break;
            }
        }

        obj_pixel
    }

    #[inline]
    fn get_window_pixel(
        &self,
        memory: &mut Memory,
        pixel_position: (u8, u8),
    ) -> Option<BackgroundPixel> {
        let lcdc = Self::get_lcdc(memory);
        let wx = memory.read(memreg::addresses::WX);
        let wy = memory.read(memreg::addresses::WY);
        let window_tilemap = match lcdc.window_tilemap() {
            Tilemap::Tilemap0 => &self.tilemap0,
            Tilemap::Tilemap1 => &self.tilemap1,
        };

        // converter a posiçao do pixel pra posiçao relativa à window
        let pixel_position_window = if pixel_position.0 + 7 >= wx && pixel_position.1 >= wy {
            (pixel_position.0 + 7 - wx, self.window_line_counter)
        } else {
            return None;
        };

        // convertendo para tile space
        let (tile_position_tilemap, pixel_position_tile) = {
            let (tile_position_x, pixel_position_x) =
                crate::util::div_rem(pixel_position_window.0, 8);
            let (tile_position_y, pixel_position_y) =
                crate::util::div_rem(pixel_position_window.1, 8);
            (
                (tile_position_x, tile_position_y),
                (pixel_position_x, pixel_position_y),
            )
        };

        // convertendo a posiçao do pixel em tile space para a posiçao no tilemap do tile que o contem e a posiçao relativa do pixel ao tile
        let tile_index_tilemap =
            tile_position_tilemap.1 as usize * 32 + tile_position_tilemap.0 as usize;
        let tile_tileset_index = window_tilemap[tile_index_tilemap];

        let tile = if lcdc.alternative_addressing_mode() {
            match tile_tileset_index {
                0..=127 => &self.master_tileset[0x1000 / 16 + tile_tileset_index as usize],
                128..=255 => &self.master_tileset[tile_tileset_index as usize],
            }
        } else {
            &self.master_tileset[tile_tileset_index as usize]
        };

        // obtendo a cor do pixel
        let color_index = tile
            .get_pixel_color_index(pixel_position_tile.0, pixel_position_tile.1)
            .unwrap();

        Some(BackgroundPixel { color_index })
    }

    fn render_scanline(&mut self, memory: &mut Memory) {
        #[inline]
        fn get_color(index: u8, palette: Palette) -> u8 {
            match index {
                0 => palette.color_0(),
                1 => palette.color_1(),
                2 => palette.color_2(),
                3 => palette.color_3(),
                _ => unreachable!(),
            }
        }

        self.update_master_tileset(memory);
        self.update_tilemaps(memory);

        let lcdc = Self::get_lcdc(memory);
        if !lcdc.screen_enabled() {
            self.buffers.back_mut().clear();
            return;
        }

        let ly = memory.read(memreg::addresses::LY);
        let bg_palette = Palette::from(memory.read(memreg::addresses::BGP));
        let obj_palette0 = Palette::from(memory.read(memreg::addresses::OBP0));
        let obj_palette1 = Palette::from(memory.read(memreg::addresses::OBP1));

        let mut window_drawn = false;
        for x in 0..160u8 {
            let pixel_position = (x, ly);

            let bg_pixel = if lcdc.background_window_priority() {
                self.get_bg_pixel(memory, pixel_position)
            } else {
                BackgroundPixel { color_index: 0 }
            };

            let window_pixel = if lcdc.window_enabled() && lcdc.background_window_priority() {
                self.get_window_pixel(memory, pixel_position)
            } else {
                None
            };
            window_drawn = window_pixel.is_some();

            let bg_pixel = window_pixel.unwrap_or(bg_pixel);

            let obj_pixel = if lcdc.objects_enabled() {
                self.get_obj_pixel(memory, pixel_position)
            } else {
                None
            };

            let final_color = if let Some(obj_pixel) = obj_pixel {
                if obj_pixel.color_index != 0
                    && !(obj_pixel.under_bg_window && bg_pixel.color_index != 0)
                {
                    get_color(
                        obj_pixel.color_index,
                        if obj_pixel.palette == 0 {
                            obj_palette0
                        } else {
                            obj_palette1
                        },
                    )
                } else {
                    get_color(bg_pixel.color_index, bg_palette)
                }
            } else {
                get_color(bg_pixel.color_index, bg_palette)
            };

            self.buffers
                .back_mut()
                .set_pixel(
                    pixel_position.0 as usize,
                    pixel_position.1 as usize,
                    final_color,
                )
                .unwrap();
        }

        if window_drawn {
            self.window_line_counter += 1;
        }
    }

    pub fn cycle(&mut self, memory: &mut Memory) {
        self.update_stat_interrupt(memory);

        if self.cycles > 0 {
            self.cycles -= 1;
            return;
        }

        let ly = memory.read(memreg::addresses::LY);
        match ly {
            0..=143 => match self.mode {
                PPUMode::HBlank => {
                    if ly == 143 {
                        self.set_mode(memory, PPUMode::VBlank);
                        self.cycles = 456;

                        self.buffers.switch();
                        Self::increment_ly(memory);
                        memory.request_interrupt(memreg::Interrupt::VBlank);
                    } else {
                        self.set_mode(memory, PPUMode::OAMSearch);
                        self.cycles = 80;

                        Self::increment_ly(memory);
                        self.oam_search(memory);
                    }
                }
                PPUMode::VBlank => unreachable!(),
                PPUMode::OAMSearch => {
                    self.set_mode(memory, PPUMode::Rendering);
                    self.cycles = 168;

                    self.render_scanline(memory);
                }
                PPUMode::Rendering => {
                    self.set_mode(memory, PPUMode::HBlank);
                    self.cycles = 208;
                }
            },
            _ => match self.mode {
                PPUMode::HBlank => unreachable!(),
                PPUMode::VBlank => match ly {
                    152 => {
                        self.cycles = 4;
                        Self::increment_ly(memory);
                    }
                    153 => {
                        self.window_line_counter = 0;
                        self.set_mode(memory, PPUMode::OAMSearch);
                        self.cycles = 80 + 456 - 4;

                        Self::increment_ly(memory);
                        self.oam_search(memory);
                    }
                    _ => {
                        self.cycles = 456;
                        Self::increment_ly(memory);
                    }
                },
                PPUMode::OAMSearch => unreachable!(),
                PPUMode::Rendering => unreachable!(),
            },
        }
    }
}

// debug
#[cfg(feature = "tdebugger")]
impl Ppu {
    pub fn dbg_save_master_tileset(&self) {
        fn tile_to_img(tile: &Tile) -> image::RgbImage {
            image::RgbImage::from_fn(8, 8, |x, y| {
                let index = tile.get_pixel_color_index(x as u8, y as u8).unwrap();
                let c = (3 - index) * 85;
                image::Rgb([c, c, c])
            })
        }

        for (i, tile) in self.master_tileset.iter().enumerate() {
            let img = tile_to_img(tile);
            img.save(format!("/dump/{}.png", i)).unwrap();
        }
    }

    pub fn dbg_save_current_buffer(&self) {
        let buffer = self.buffers.back();
        let img = image::RgbImage::from_fn(160, 144, |x, y| {
            let c = buffer.get_pixel(x as usize, y as usize).unwrap() * 85;
            image::Rgb([c, c, c])
        });

        img.save("/dump/buffer_dump.png").unwrap();
    }
}
