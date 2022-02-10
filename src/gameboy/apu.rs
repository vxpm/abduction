use super::memory::Memory;

pub struct Apu;

impl Apu {
    pub fn new() -> Self {
        Self
    }

    pub fn cycle(&mut self, _memory: &mut Memory) {
        // do nothing
    }
}
