use super::memory::registers as memreg;
use super::memory::Memory;

struct Tac {
    data: u8,
}

impl Tac {
    pub fn new(data: u8) -> anyhow::Result<Self> {
        if data & 0b1111_1000 != 0b0000_0000 {
            anyhow::bail!("Invalid bits");
        }

        Ok(Self { data })
    }

    pub fn timer_enabled(&self) -> bool {
        self.data & 0b0000_0100 == 0b0000_0100
    }

    pub fn tima_divider(&self) -> u16 {
        match self.data & 0b0000_0011 {
            0b0000_0000 => 1024,
            0b0000_0001 => 16,
            0b0000_0010 => 64,
            0b0000_0011 => 256,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}

pub struct Timer {
    div_cycle_count: u32,
    tima_cycle_count: u16,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            div_cycle_count: 0,
            tima_cycle_count: 0,
        }
    }

    fn update_div(&mut self, memory: &mut Memory) {
        if self.div_cycle_count < 256 {
            self.div_cycle_count += 1;
            return;
        }

        self.div_cycle_count = 0;

        let div = memory.read(memreg::addresses::DIV);
        let new_div = div.wrapping_add(1);

        memory.write(memreg::addresses::DIV, new_div);
    }

    fn update_tima(&mut self, memory: &mut Memory) {
        let tac = Tac::new(memory.read(memreg::addresses::TAC) & 0b0000_0111).unwrap();
        if !tac.timer_enabled() {
            return;
        }

        if self.tima_cycle_count <= tac.tima_divider() {
            self.tima_cycle_count += 1;
            return;
        }

        self.tima_cycle_count = 0;

        let tima = memory.read(memreg::addresses::TIMA);
        let (new_tima, overflow) = tima.overflowing_add(1);

        let new_tima = if overflow {
            memory.request_interrupt(memreg::Interrupt::Timer);
            memory.read(memreg::addresses::TMA)
        } else {
            new_tima
        };

        memory.write(memreg::addresses::TIMA, new_tima);
    }

    pub fn cycle(&mut self, memory: &mut Memory) {
        self.update_div(memory);
        self.update_tima(memory);
    }
}
