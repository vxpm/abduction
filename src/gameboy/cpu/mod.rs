pub mod operation;

use self::operation::*;
use super::memory::{self, Memory};
use flagset::{flags, FlagSet};

flags! {
    pub enum CpuFlag: u8 {
        Zero = 0b1000_0000,
        Negative = 0b0100_0000,
        Half = 0b0010_0000,
        Carry = 0b0001_0000,
    }
}

/// A 8-bit Register
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ByteRegister {
    A,
    B,
    C,
    D,
    E,
    F,
    H,
    L,
}

/// A 16-bit Register
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum WordRegister {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

/// Collection of Gameboy registers.
#[derive(Default, Clone)]
pub struct Registers {
    af: [u8; 2],
    bc: [u8; 2],
    de: [u8; 2],
    hl: [u8; 2],
    sp: u16,
    pc: u16,
}

impl std::fmt::Debug for Registers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AF: {:#06X} BC: {:#06X} DE: {:#06X} HL: {:#06X} SP: {:#06X} PC: {:#06X}",
            u16::from_be_bytes(self.af),
            u16::from_be_bytes(self.bc),
            u16::from_be_bytes(self.de),
            u16::from_be_bytes(self.hl),
            self.sp,
            self.pc
        )
    }
}

impl Registers {
    pub fn new() -> Self {
        Self {
            af: Default::default(),
            bc: Default::default(),
            de: Default::default(),
            hl: Default::default(),
            sp: 0xFFFE,
            pc: 0x0000,
        }
    }

    #[inline]
    pub fn get_reg_8(&self, register: ByteRegister) -> u8 {
        match register {
            ByteRegister::A => self.af[0],
            ByteRegister::B => self.bc[0],
            ByteRegister::C => self.bc[1],
            ByteRegister::D => self.de[0],
            ByteRegister::E => self.de[1],
            ByteRegister::F => self.af[1],
            ByteRegister::H => self.hl[0],
            ByteRegister::L => self.hl[1],
        }
    }

    #[inline]
    pub fn set_reg_8(&mut self, register: ByteRegister, value: u8) {
        match register {
            ByteRegister::A => self.af[0] = value,
            ByteRegister::B => self.bc[0] = value,
            ByteRegister::C => self.bc[1] = value,
            ByteRegister::D => self.de[0] = value,
            ByteRegister::E => self.de[1] = value,
            ByteRegister::F => self.af[1] = value,
            ByteRegister::H => self.hl[0] = value,
            ByteRegister::L => self.hl[1] = value,
        }
    }

    #[inline]
    pub fn get_reg_16(&self, register: WordRegister) -> u16 {
        match register {
            WordRegister::AF => u16::from_be_bytes(self.af),
            WordRegister::BC => u16::from_be_bytes(self.bc),
            WordRegister::DE => u16::from_be_bytes(self.de),
            WordRegister::HL => u16::from_be_bytes(self.hl),
            WordRegister::SP => self.sp,
            WordRegister::PC => self.pc,
        }
    }

    #[inline]
    pub fn set_reg_16(&mut self, register: WordRegister, value: u16) {
        match register {
            WordRegister::AF => self.af = value.to_be_bytes(),
            WordRegister::BC => self.bc = value.to_be_bytes(),
            WordRegister::DE => self.de = value.to_be_bytes(),
            WordRegister::HL => self.hl = value.to_be_bytes(),
            WordRegister::SP => self.sp = value,
            WordRegister::PC => self.pc = value,
        }
    }

    #[inline]
    pub fn get_flag(&self, flag: CpuFlag) -> bool {
        let set = FlagSet::<CpuFlag>::new(self.af[1]).unwrap();
        set.contains(flag)
    }

    #[inline]
    pub fn set_flag(&mut self, flag: CpuFlag, value: bool) {
        let set = FlagSet::<CpuFlag>::new(self.af[1] & 0b1111_0000).unwrap(); // unwrap because invalid flags are a implementation error
        self.af[1] = if value {
            (set | flag).bits()
        } else {
            (set & !flag).bits()
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MasterInterrupt {
    Off,
    TurningOn,
    On,
}

/// CPU (Central Processing Unit) component of the Gameboy.
pub struct Cpu {
    registers: Registers,
    master_interrupt_flag: MasterInterrupt,
    halt: bool,
}

pub trait OnMachineCycle = FnMut(&mut Memory);

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: Registers::new(),
            master_interrupt_flag: MasterInterrupt::Off,
            halt: false,
        }
    }

    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }

    pub fn master_interrupt_flag(&self) -> MasterInterrupt {
        self.master_interrupt_flag
    }

    #[inline]
    fn mem_read(memory: &Memory, address: u16) -> u8 {
        // TODO: add restrictions regarding PPU modes
        memory.read(address)
    }

    #[inline]
    fn mem_write(memory: &mut Memory, address: u16, data: u8) {
        match address {
            memory::registers::addresses::LY => (),
            memory::registers::addresses::DIV => memory.write(address, 0x00),
            memory::registers::addresses::STAT => {
                memory.write(address, data & !0b0000_0111);
            }
            _ => memory.write(address, data),
        }
    }

    /// Read data at PC and increment PC by one.
    #[inline]
    pub fn fetch(&mut self, memory: &Memory) -> u8 {
        let pc = self.registers.get_reg_16(WordRegister::PC);
        self.registers
            .set_reg_16(WordRegister::PC, pc.wrapping_add(1));

        Self::mem_read(memory, pc)
    }

    /// Step the CPU emulation. This is equivalent to one "fetch, decode, execute" cycle.
    pub fn step<F>(&mut self, memory: &mut Memory, on_machine_cycle: &mut F)
    where
        F: OnMachineCycle,
    {
        on_machine_cycle(memory);

        let turn_master_interrupt_on = self.master_interrupt_flag == MasterInterrupt::TurningOn;

        // halt behaviour
        if self.halt {
            let enabled = Self::mem_read(memory, memory::registers::addresses::INTERRUPT_ENABLE)
                & 0b0001_1111;
            let requested = Self::mem_read(memory, memory::registers::addresses::INTERRUPT_REQUEST)
                & 0b0001_1111;

            if enabled & requested == 0 {
                if turn_master_interrupt_on {
                    self.master_interrupt_flag = MasterInterrupt::On;
                }
                return;
            }

            self.halt = false;
        }

        // fetch
        let opcode = self.fetch(memory);

        if let MasterInterrupt::On = self.master_interrupt_flag {
            if self.handle_interrupts(memory, on_machine_cycle) {
                return;
            }
        }

        // decode and execute
        let op = Operation::from(opcode);
        self.execute(op, memory, on_machine_cycle);

        // only turn master interrupt on if it was turning on at the start of the function and if it
        // wasn't turned off by the last instruction
        if turn_master_interrupt_on && self.master_interrupt_flag == MasterInterrupt::TurningOn {
            self.master_interrupt_flag = MasterInterrupt::On;
        }
    }

    pub fn handle_interrupts<F>(&mut self, memory: &mut Memory, on_machine_cycle: &mut F) -> bool
    where
        F: OnMachineCycle,
    {
        const INTERRUPT_PRIORITY: [memory::registers::Interrupt; 5] = [
            memory::registers::Interrupt::VBlank,
            memory::registers::Interrupt::STAT,
            memory::registers::Interrupt::Timer,
            memory::registers::Interrupt::Serial,
            memory::registers::Interrupt::Joypad,
        ];

        let enabled = FlagSet::<memory::registers::Interrupt>::new(
            Self::mem_read(memory, memory::registers::addresses::INTERRUPT_ENABLE) & 0b0001_1111,
        )
        .unwrap();
        let requested = FlagSet::<memory::registers::Interrupt>::new(
            Self::mem_read(memory, memory::registers::addresses::INTERRUPT_REQUEST) & 0b0001_1111,
        )
        .unwrap();

        let interrupt_to_handle = if let Some(int) = INTERRUPT_PRIORITY
            .into_iter()
            .find(|&i| requested.contains(i) && enabled.contains(i))
        {
            int
        } else {
            return false;
        };

        let address = match interrupt_to_handle {
            memory::registers::Interrupt::VBlank => 0x40,
            memory::registers::Interrupt::STAT => 0x48,
            memory::registers::Interrupt::Timer => 0x50,
            memory::registers::Interrupt::Serial => 0x58,
            memory::registers::Interrupt::Joypad => 0x60,
        };

        // decrement PC
        on_machine_cycle(memory);
        let current_pc = self.registers.get_reg_16(WordRegister::PC);
        self.registers
            .set_reg_16(WordRegister::PC, current_pc.wrapping_sub(1));

        // same as a CALL
        on_machine_cycle(memory);
        let current_pc = self.registers.get_reg_16(WordRegister::PC).to_le_bytes();
        let current_sp = self.registers.get_reg_16(WordRegister::SP);

        on_machine_cycle(memory);
        Self::mem_write(memory, current_sp.wrapping_sub(1), current_pc[1]);

        on_machine_cycle(memory);
        Self::mem_write(memory, current_sp.wrapping_sub(2), current_pc[0]);

        self.registers
            .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
        self.registers.set_reg_16(WordRegister::PC, address);

        // clear interrupt request for the handled interrupt
        let requested = requested & !interrupt_to_handle;
        Self::mem_write(
            memory,
            memory::registers::addresses::INTERRUPT_REQUEST,
            requested.bits(),
        );

        // turn off master interrupt
        self.master_interrupt_flag = MasterInterrupt::Off;
        true
    }

    pub fn execute<F>(
        &mut self,
        operation: Operation,
        memory: &mut Memory,
        on_machine_cycle: &mut F,
    ) where
        F: OnMachineCycle,
    {
        // TODO: Make implementations more consistent in conventions. Also, some could be simplified if
        // registers cannot be accessed from outside the CPU.
        match operation {
            Operation::Noop => {
                // noop
            }
            Operation::LoadImmediateIntoWordReg(wreg) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                self.registers
                    .set_reg_16(wreg, u16::from_le_bytes([low, high]));
            }
            Operation::LoadRegIntoAddressInWordReg(reg, wreg) => {
                on_machine_cycle(memory);
                Self::mem_write(
                    memory,
                    self.registers.get_reg_16(wreg),
                    self.registers.get_reg_8(reg),
                );
            }
            Operation::IncrementWordReg(wreg) => {
                on_machine_cycle(memory);
                let current = self.registers.get_reg_16(wreg);
                let new = current.wrapping_add(1);
                self.registers.set_reg_16(wreg, new);
            }
            Operation::IncrementReg(reg) => {
                let current = self.registers.get_reg_8(reg);
                let new = current.wrapping_add(1);
                self.registers.set_reg_8(reg, new);

                self.registers.set_flag(CpuFlag::Zero, new == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, (current & 0x0F) + 1 > 0x0F);
            }
            Operation::DecrementReg(reg) => {
                let current = self.registers.get_reg_8(reg);
                let new = current.wrapping_sub(1);
                self.registers.set_reg_8(reg, new);

                self.registers.set_flag(CpuFlag::Zero, new == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers.set_flag(CpuFlag::Half, (current & 0x0F) < 1);
            }
            Operation::LoadImmediateIntoReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                self.registers.set_reg_8(reg, byte);
            }
            Operation::RotateAccLeft => {
                let acc = self.registers.get_reg_8(ByteRegister::A);
                let carry = acc & 0x80 == 0x80;
                self.registers.set_reg_8(
                    ByteRegister::A,
                    if carry { (acc << 1) | 0x01 } else { acc << 1 },
                );

                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::LoadSPIntoImmediateAddress => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);

                let address = u16::from_le_bytes([low, high]);
                let current_sp = self.registers.get_reg_16(WordRegister::SP).to_le_bytes();

                on_machine_cycle(memory);
                Self::mem_write(memory, address, current_sp[0]);
                on_machine_cycle(memory);
                Self::mem_write(memory, address.wrapping_add(1), current_sp[1]);
            }
            Operation::AddWordRegIntoWordReg(wreg_a, wreg_b) => {
                on_machine_cycle(memory);
                let a = self.registers.get_reg_16(wreg_a);
                let b = self.registers.get_reg_16(wreg_b);
                let (new, overflow) = b.overflowing_add(a);
                self.registers.set_reg_16(wreg_b, new);

                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(
                    CpuFlag::Half,
                    ((b & 0x0FFF).saturating_add(a & 0x0FFF)) > 0x0FFF,
                );
                self.registers.set_flag(CpuFlag::Carry, overflow);
            }
            Operation::LoadAtAddressInWordRegIntoReg(address_wreg, reg) => {
                on_machine_cycle(memory);
                let byte = Self::mem_read(memory, self.registers.get_reg_16(address_wreg));
                self.registers.set_reg_8(reg, byte);
            }
            Operation::DecrementWordReg(wreg) => {
                on_machine_cycle(memory);
                let current = self.registers.get_reg_16(wreg);
                let new = current.wrapping_sub(1);
                self.registers.set_reg_16(wreg, new);
            }
            Operation::RotateAccRight => {
                let acc = self.registers.get_reg_8(ByteRegister::A);
                let carry = acc & 1 == 1;
                self.registers.set_reg_8(
                    ByteRegister::A,
                    if carry { (acc >> 1) | 0x80 } else { acc >> 1 },
                );

                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::Stop => {
                // currently, no op
            }
            Operation::RotateAccLeftThroughCarry => {
                let acc = self.registers.get_reg_8(ByteRegister::A);
                let carry_old = self.registers.get_flag(CpuFlag::Carry);
                let carry_new = acc & 0x80 == 0x80;
                self.registers.set_reg_8(
                    ByteRegister::A,
                    if carry_old {
                        (acc << 1) | 0x01
                    } else {
                        acc << 1
                    },
                );

                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, carry_new);
            }
            Operation::RelativeJumpImmediateOffset => {
                let address = self.fetch(memory) as i8;
                on_machine_cycle(memory);
                on_machine_cycle(memory);
                self.registers.set_reg_16(
                    WordRegister::PC,
                    self.registers
                        .get_reg_16(WordRegister::PC)
                        .wrapping_add_signed(address as i16),
                );
            }
            Operation::RotateAccRightThroughCarry => {
                let acc = self.registers.get_reg_8(ByteRegister::A);
                let carry_old = self.registers.get_flag(CpuFlag::Carry);
                let carry_new = acc & 1 == 1;
                self.registers.set_reg_8(
                    ByteRegister::A,
                    if carry_old {
                        (acc >> 1) | 0x80
                    } else {
                        acc >> 1
                    },
                );

                self.registers.set_flag(CpuFlag::Carry, carry_new);
                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
            }
            Operation::ConditionalRelativeJumpImmediateOffset(flag) => {
                on_machine_cycle(memory);
                let offset = self.fetch(memory) as i8;
                if self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    self.registers.set_reg_16(
                        WordRegister::PC,
                        self.registers
                            .get_reg_16(WordRegister::PC)
                            .wrapping_add_signed(offset as i16),
                    );
                }
            }
            Operation::NegativeConditionalRelativeJumpImmediateOffset(flag) => {
                on_machine_cycle(memory);
                let offset = self.fetch(memory) as i8;
                if !self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    self.registers.set_reg_16(
                        WordRegister::PC,
                        self.registers
                            .get_reg_16(WordRegister::PC)
                            .wrapping_add_signed(offset as i16),
                    );
                }
            }
            Operation::LoadRegIntoAddressInWordRegAndIncrementWordReg(reg, wreg) => {
                on_machine_cycle(memory);
                let byte = self.registers.get_reg_8(reg);
                let address = self.registers.get_reg_16(wreg);
                Self::mem_write(memory, address, byte);
                self.registers.set_reg_16(wreg, address.wrapping_add(1));
            }
            Operation::DecimalAdjustAcc => {
                // when i wrote this, i knew why and how it worked. i definitely don't anymore.
                // thanks https://ehaskins.com/2018-01-30%20Z80%20DAA/
                let a = self.registers.get_reg_8(ByteRegister::A);
                let neg = self.registers.get_flag(CpuFlag::Negative);

                let mut carry = false;
                let mut correction = 0x00;
                if !neg && (a > 0x99) || self.registers.get_flag(CpuFlag::Carry) {
                    correction |= 0x60;
                    carry = true;
                }
                if !neg && (a & 0x0F > 9) || self.registers.get_flag(CpuFlag::Half) {
                    correction |= 0x06;
                }
                let corrected = if neg {
                    a.wrapping_sub(correction)
                } else {
                    a.wrapping_add(correction)
                };

                self.registers.set_reg_8(ByteRegister::A, corrected);

                self.registers.set_flag(CpuFlag::Zero, corrected == 0);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::LoadAtAddressInWordRegIntoRegAndIncrementWordReg(wreg, reg) => {
                on_machine_cycle(memory);
                let address = self.registers.get_reg_16(wreg);
                let byte = Self::mem_read(memory, address);
                self.registers.set_reg_8(reg, byte);
                self.registers.set_reg_16(wreg, address.wrapping_add(1));
            }
            Operation::ComplementAcc => {
                let acc = self.registers.get_reg_8(ByteRegister::A);
                self.registers.set_reg_8(ByteRegister::A, !acc);

                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers.set_flag(CpuFlag::Half, true);
            }
            Operation::LoadRegIntoAddressInWordRegAndDecrementWordReg(reg, wreg) => {
                on_machine_cycle(memory);
                let byte = self.registers.get_reg_8(reg);
                let address = self.registers.get_reg_16(wreg);
                Self::mem_write(memory, address, byte);
                self.registers.set_reg_16(wreg, address.wrapping_sub(1));
            }
            Operation::IncrementAtAddressInWordReg(wreg) => {
                let address = self.registers.get_reg_16(wreg);
                on_machine_cycle(memory);
                let data = Self::mem_read(memory, address);
                on_machine_cycle(memory);
                let res = data.wrapping_add(1);
                Self::mem_write(memory, address, res);

                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers
                    .set_flag(CpuFlag::Half, ((data & 0x0F) + 1) & 0x10 == 0x10);
            }
            Operation::DecrementAtAddressInWordReg(wreg) => {
                let address = self.registers.get_reg_16(wreg);
                on_machine_cycle(memory);
                let data = Self::mem_read(memory, address);
                on_machine_cycle(memory);
                let res = data.wrapping_sub(1);
                Self::mem_write(memory, address, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers.set_flag(CpuFlag::Half, (data & 0x0F) < 1);
            }
            Operation::LoadImmediateIntoAddressInWordReg(wreg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                on_machine_cycle(memory);
                let address = self.registers.get_reg_16(wreg);
                Self::mem_write(memory, address, byte);
            }
            Operation::SetCarry => {
                self.registers.set_flag(CpuFlag::Carry, true);

                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
            }
            Operation::LoadAtAddressInWordRegIntoRegAndDecrementWordReg(wreg, reg) => {
                on_machine_cycle(memory);
                let address = self.registers.get_reg_16(wreg);
                let byte = Self::mem_read(memory, address);
                self.registers.set_reg_8(reg, byte);
                self.registers.set_reg_16(wreg, address.wrapping_sub(1));
            }
            Operation::ComplementCarry => {
                let carry = self.registers.get_flag(CpuFlag::Carry);
                self.registers.set_flag(CpuFlag::Carry, !carry);

                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
            }
            Operation::LoadRegIntoReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                self.registers.set_reg_8(reg_b, a);
            }
            Operation::Halt => {
                // if IME is set:
                //      halt pauses the CPU until an interrupt is pending.
                // if IME is not set:
                //      if a interrupt is pending, halt does nothing but the halt bug can happen. (note: bug not emulated here)
                //      if no interrupt is pending, halt pauses the CPU until one is (just like when IME is set).
                if self.master_interrupt_flag == MasterInterrupt::On {
                    self.halt = true;
                } else {
                    let enabled =
                        Self::mem_read(memory, memory::registers::addresses::INTERRUPT_ENABLE)
                            & 0b0001_1111;
                    let requested =
                        Self::mem_read(memory, memory::registers::addresses::INTERRUPT_REQUEST)
                            & 0b0001_1111;

                    if enabled & requested == 0 {
                        self.halt = true;
                    }
                }
            }
            Operation::AddRegIntoReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);
                let (res, carry) = b.overflowing_add(a);
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, ((b & 0x0F) + (a & 0x0F)) > 0x0F);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::AddAtAddressInWordRegIntoReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);
                let (res, carry) = b.overflowing_add(a);
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, ((b & 0x0F) + (a & 0x0F)) > 0x0F);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::AddRegIntoRegWithCarry(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = b.overflowing_add(a);
                let (res, carry_2) = res.overflowing_add(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, ((b & 0x0F) + (a & 0x0F) + carry_flag) > 0x0F);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::AddAtAddressInWordRegIntoRegWithCarry(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = b.overflowing_add(a);
                let (res, carry_2) = res.overflowing_add(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, ((b & 0x0F) + (a & 0x0F) + carry_flag) > 0x0F);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::SubRegFromReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);
                let (res, carry) = b.overflowing_sub(a);
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::SubAtAddressInWordRegFromReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);
                let (res, carry) = b.overflowing_sub(a);
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::SubRegFromRegWithCarry(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = b.overflowing_sub(a);
                let (res, carry_2) = res.overflowing_sub(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F) + carry_flag);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::SubAtAddressInWordRegFromRegWithCarry(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = b.overflowing_sub(a);
                let (res, carry_2) = res.overflowing_sub(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F) + carry_flag);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::AndRegIntoReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);

                let res = a & b;
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, true);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::AndAtAddressInWordRegIntoReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);

                let res = a & b;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, true);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::XorRegIntoReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);

                let res = a ^ b;
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::XorAtAddressInWordRegIntoReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);

                let res = a ^ b;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::OrRegIntoReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);

                let res = a | b;
                self.registers.set_reg_8(reg_b, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::OrAtAddressInWordRegIntoReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);

                let res = a | b;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::CompareRegAndReg(reg_a, reg_b) => {
                let a = self.registers.get_reg_8(reg_a);
                let b = self.registers.get_reg_8(reg_b);
                let (res, carry) = b.overflowing_sub(a);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::CompareAtAddressInWordRegAndReg(wreg, reg) => {
                on_machine_cycle(memory);
                let a = Self::mem_read(memory, self.registers.get_reg_16(wreg));
                let b = self.registers.get_reg_8(reg);
                let (res, carry) = b.overflowing_sub(a);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (b & 0x0F) < (a & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::ConditionalReturn(flag) => {
                on_machine_cycle(memory);
                if self.registers.get_flag(flag) {
                    let current_sp = self.registers.get_reg_16(WordRegister::SP);

                    on_machine_cycle(memory);
                    let low = Self::mem_read(memory, current_sp);

                    on_machine_cycle(memory);
                    let high = Self::mem_read(memory, current_sp.wrapping_add(1));

                    self.registers
                        .set_reg_16(WordRegister::SP, current_sp.wrapping_add(2));
                    self.registers
                        .set_reg_16(WordRegister::PC, u16::from_le_bytes([low, high]));
                }
            }
            Operation::NegativeConditionalReturn(flag) => {
                on_machine_cycle(memory);
                if !self.registers.get_flag(flag) {
                    let current_sp = self.registers.get_reg_16(WordRegister::SP);

                    on_machine_cycle(memory);
                    let low = Self::mem_read(memory, current_sp);

                    on_machine_cycle(memory);
                    let high = Self::mem_read(memory, current_sp.wrapping_add(1));

                    self.registers
                        .set_reg_16(WordRegister::SP, current_sp.wrapping_add(2));
                    self.registers
                        .set_reg_16(WordRegister::PC, u16::from_le_bytes([low, high]));
                }
            }
            Operation::PopStackIntoWordReg(wreg) => {
                let current_sp = self.registers.get_reg_16(WordRegister::SP);

                on_machine_cycle(memory);
                let low = Self::mem_read(memory, current_sp);

                on_machine_cycle(memory);
                let high = Self::mem_read(memory, current_sp.wrapping_add(1));

                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_add(2));
                self.registers
                    .set_reg_16(wreg, u16::from_le_bytes([low, high]));

                if wreg == WordRegister::AF {
                    self.registers.set_flag(CpuFlag::Zero, (low >> 7) & 1 == 1);
                    self.registers
                        .set_flag(CpuFlag::Negative, (low >> 6) & 1 == 1);
                    self.registers.set_flag(CpuFlag::Half, (low >> 5) & 1 == 1);
                    self.registers.set_flag(CpuFlag::Carry, (low >> 4) & 1 == 1);
                }
            }
            Operation::ConditionalJumpImmediateAddress(flag) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                if self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    let address = u16::from_le_bytes([low, high]);
                    self.registers.set_reg_16(WordRegister::PC, address);
                }
            }
            Operation::NegativeConditionalJumpImmediateAddress(flag) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                if !self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    let address = u16::from_le_bytes([low, high]);
                    self.registers.set_reg_16(WordRegister::PC, address);
                }
            }
            Operation::JumpImmediateAddress => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                on_machine_cycle(memory);
                self.registers
                    .set_reg_16(WordRegister::PC, u16::from_le_bytes([low, high]));
            }
            Operation::ConditionalCallImmediateAddress(flag) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                if self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    let current_pc = self.registers.get_reg_16(WordRegister::PC).to_le_bytes();
                    let current_sp = self.registers.get_reg_16(WordRegister::SP);

                    on_machine_cycle(memory);
                    Self::mem_write(memory, current_sp.wrapping_sub(1), current_pc[1]);

                    on_machine_cycle(memory);
                    Self::mem_write(memory, current_sp.wrapping_sub(2), current_pc[0]);

                    let address = u16::from_le_bytes([low, high]);
                    self.registers.set_reg_16(WordRegister::PC, address);
                    self.registers
                        .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
                }
            }
            Operation::NegativeConditionalCallImmediateAddress(flag) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);
                if !self.registers.get_flag(flag) {
                    on_machine_cycle(memory);
                    let current_pc = self.registers.get_reg_16(WordRegister::PC).to_le_bytes();
                    let current_sp = self.registers.get_reg_16(WordRegister::SP);

                    on_machine_cycle(memory);
                    Self::mem_write(memory, current_sp.wrapping_sub(1), current_pc[1]);

                    on_machine_cycle(memory);
                    Self::mem_write(memory, current_sp.wrapping_sub(2), current_pc[0]);

                    let address = u16::from_le_bytes([low, high]);
                    self.registers.set_reg_16(WordRegister::PC, address);
                    self.registers
                        .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
                }
            }
            Operation::PushWordRegIntoStack(wreg) => {
                on_machine_cycle(memory);
                let current_sp = self.registers.get_reg_16(WordRegister::SP);
                let wr = self.registers.get_reg_16(wreg).to_le_bytes();

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(1), wr[1]);

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(2), wr[0]);

                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
            }
            Operation::AddImmediateIntoReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);
                let (res, carry) = r.overflowing_add(byte);
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers
                    .set_flag(CpuFlag::Half, ((r & 0x0F) + (byte & 0x0F)) > 0x0F);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::CallFixedAddress(address) => {
                on_machine_cycle(memory);
                let current_pc = self.registers.get_reg_16(WordRegister::PC).to_le_bytes();
                let current_sp = self.registers.get_reg_16(WordRegister::SP);

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(1), current_pc[1]);

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(2), current_pc[0]);

                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
                self.registers.set_reg_16(WordRegister::PC, address);
            }
            Operation::Return => {
                let current_sp = self.registers.get_reg_16(WordRegister::SP);

                on_machine_cycle(memory);
                let low = Self::mem_read(memory, current_sp);

                on_machine_cycle(memory);
                let high = Self::mem_read(memory, current_sp.wrapping_add(1));

                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_add(2));

                on_machine_cycle(memory);
                self.registers
                    .set_reg_16(WordRegister::PC, u16::from_le_bytes([low, high]));
            }
            Operation::Prefixed => {
                on_machine_cycle(memory);
                let prefixed_operation = PrefixedOperation::from(self.fetch(memory));

                match prefixed_operation {
                    PrefixedOperation::RotateRegLeft(reg) => {
                        let current = self.registers.get_reg_8(reg);
                        let carry = current & 0x80 == 0x80;
                        let new = if carry {
                            (current << 1) | 0x01
                        } else {
                            current << 1
                        };
                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry);
                    }
                    PrefixedOperation::RotateAtAddressInWordRegLeft(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let current = Self::mem_read(memory, address);
                        let carry = current & 0x80 == 0x80;
                        let new = if carry {
                            (current << 1) | 0x01
                        } else {
                            current << 1
                        };

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry);
                    }
                    PrefixedOperation::RotateRegRight(reg) => {
                        let current = self.registers.get_reg_8(reg);
                        let carry = current & 1 == 1;
                        let new = if carry {
                            (current >> 1) | 0x80
                        } else {
                            current >> 1
                        };
                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry);
                    }
                    PrefixedOperation::RotateAtAddressInWordRegRight(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let current = Self::mem_read(memory, address);
                        let carry = current & 1 == 1;
                        let new = if carry {
                            (current >> 1) | 0x80
                        } else {
                            current >> 1
                        };

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry);
                    }
                    PrefixedOperation::RotateRegLeftThroughCarry(reg) => {
                        let current = self.registers.get_reg_8(reg);
                        let carry_old = self.registers.get_flag(CpuFlag::Carry);
                        let carry_new = current & 0x80 == 0x80;
                        let new = if carry_old {
                            (current << 1) | 0x01
                        } else {
                            current << 1
                        };
                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry_new);
                    }
                    PrefixedOperation::RotateAtAddressInWordRegLeftThroughCarry(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let current = Self::mem_read(memory, address);
                        let carry_old = self.registers.get_flag(CpuFlag::Carry);
                        let carry_new = current & 0x80 == 0x80;
                        let new = if carry_old {
                            (current << 1) | 0x01
                        } else {
                            current << 1
                        };

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry_new);
                    }
                    PrefixedOperation::RotateRegRightThroughCarry(reg) => {
                        let current = self.registers.get_reg_8(reg);
                        let carry_old = self.registers.get_flag(CpuFlag::Carry);
                        let carry_new = current & 1 == 1;
                        let new = if carry_old {
                            (current >> 1) | 0x80
                        } else {
                            current >> 1
                        };
                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry_new);
                    }
                    PrefixedOperation::RotateAtAddressInWordRegRightThroughCarry(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let current = Self::mem_read(memory, address);
                        let carry_old = self.registers.get_flag(CpuFlag::Carry);
                        let carry_new = current & 1 == 1;
                        let new = if carry_old {
                            (current >> 1) | 0x80
                        } else {
                            current >> 1
                        };
                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, carry_new);
                    }
                    PrefixedOperation::ShiftRegLeftArithmetically(reg) => {
                        let r = self.registers.get_reg_8(reg);
                        self.registers.set_reg_8(reg, r << 1);

                        self.registers.set_flag(CpuFlag::Zero, (r << 1) == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, r & 0x80 == 0x80);
                    }
                    PrefixedOperation::ShiftAtAddressInWordRegLeftArithmetically(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let byte = Self::mem_read(memory, address);

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, byte << 1);

                        self.registers.set_flag(CpuFlag::Zero, (byte << 1) == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, byte & 0x80 == 0x80);
                    }
                    PrefixedOperation::ShiftRegRightArithmetically(reg) => {
                        let r = self.registers.get_reg_8(reg);
                        let shifted = ((r as i8) >> 1) as u8;
                        self.registers.set_reg_8(reg, shifted);

                        self.registers.set_flag(CpuFlag::Zero, shifted == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, r & 1 == 1);
                    }
                    PrefixedOperation::ShiftAtAddressInWordRegRightArithmetically(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let byte = Self::mem_read(memory, address);
                        let shifted = ((byte as i8) >> 1) as u8;

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, shifted);

                        self.registers.set_flag(CpuFlag::Zero, shifted == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, byte & 1 == 1);
                    }
                    PrefixedOperation::SwapRegNibbles(reg) => {
                        let r = self.registers.get_reg_8(reg);
                        let new = ((r & 0x0F) << 4) | ((r & 0xF0) >> 4);
                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, false);
                    }
                    PrefixedOperation::SwapAtAddressInWordRegNibbles(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let byte = Self::mem_read(memory, address);

                        on_machine_cycle(memory);
                        let new = ((byte & 0x0F) << 4) | ((byte & 0xF0) >> 4);
                        Self::mem_write(memory, address, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, false);
                    }
                    PrefixedOperation::ShiftRegRightLogically(reg) => {
                        let r = self.registers.get_reg_8(reg);
                        let new = r >> 1;

                        self.registers.set_reg_8(reg, new);

                        self.registers.set_flag(CpuFlag::Zero, new == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, r & 1 == 1);
                    }
                    PrefixedOperation::ShiftAtAddressInWordRegRightLogically(wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let byte = Self::mem_read(memory, address);
                        let shifted = byte >> 1;

                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, shifted);

                        self.registers.set_flag(CpuFlag::Zero, shifted == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, false);
                        self.registers.set_flag(CpuFlag::Carry, byte & 1 == 1);
                    }
                    PrefixedOperation::TestForBitInReg(bit, reg) => {
                        let current = self.registers.get_reg_8(reg);

                        self.registers
                            .set_flag(CpuFlag::Zero, (current >> bit) & 0x01 == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, true);
                    }
                    PrefixedOperation::TestForBitInAtAddressInWordReg(bit, wreg) => {
                        on_machine_cycle(memory);
                        let address = self.registers.get_reg_16(wreg);
                        let byte = Self::mem_read(memory, address);

                        self.registers
                            .set_flag(CpuFlag::Zero, (byte >> bit) & 0x01 == 0);
                        self.registers.set_flag(CpuFlag::Negative, false);
                        self.registers.set_flag(CpuFlag::Half, true);
                    }
                    PrefixedOperation::ClearBitInReg(bit, reg) => {
                        let current = self.registers.get_reg_8(reg);
                        self.registers.set_reg_8(reg, current & !(1 << bit));
                    }
                    PrefixedOperation::ClearBitInAtAddressInWordReg(bit, wreg) => {
                        let address = self.registers.get_reg_16(wreg);
                        on_machine_cycle(memory);
                        let current = Self::mem_read(memory, address);
                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, current & !(1 << bit));
                    }
                    PrefixedOperation::SetBitInReg(bit, reg) => {
                        let current = self.registers.get_reg_8(reg);
                        self.registers.set_reg_8(reg, current | (1 << bit));
                    }
                    PrefixedOperation::SetBitInAtAddressInWordReg(bit, wreg) => {
                        let address = self.registers.get_reg_16(wreg);
                        on_machine_cycle(memory);
                        let current = Self::mem_read(memory, address);
                        on_machine_cycle(memory);
                        Self::mem_write(memory, address, current | (1 << bit));
                    }
                }
            }
            Operation::CallImmediateAddress => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);

                on_machine_cycle(memory);
                let current_pc = self.registers.get_reg_16(WordRegister::PC).to_le_bytes();
                let current_sp = self.registers.get_reg_16(WordRegister::SP);

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(1), current_pc[1]);

                on_machine_cycle(memory);
                Self::mem_write(memory, current_sp.wrapping_sub(2), current_pc[0]);

                let address = u16::from_le_bytes([low, high]);
                self.registers.set_reg_16(WordRegister::PC, address);
                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_sub(2));
            }
            Operation::AddImmediateIntoRegWithCarry(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = r.overflowing_add(byte);
                let (res, carry_2) = res.overflowing_add(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(
                    CpuFlag::Half,
                    ((r & 0x0F) + (byte & 0x0F) + carry_flag) > 0x0F,
                );
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::SubImmediateFromReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);
                let (res, carry) = r.overflowing_sub(byte);
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (r & 0x0F) < (byte & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::ReturnAndEnableInterrupts => {
                self.master_interrupt_flag = MasterInterrupt::On;
                let current_sp = self.registers.get_reg_16(WordRegister::SP);

                on_machine_cycle(memory);
                let low = Self::mem_read(memory, current_sp);

                on_machine_cycle(memory);
                let high = Self::mem_read(memory, current_sp.wrapping_add(1));

                self.registers
                    .set_reg_16(WordRegister::SP, current_sp.wrapping_add(2));
                self.registers
                    .set_reg_16(WordRegister::PC, u16::from_le_bytes([low, high]));
            }
            Operation::SubImmediateFromRegWithCarry(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);
                let carry_flag = if self.registers.get_flag(CpuFlag::Carry) {
                    1
                } else {
                    0
                };
                let (res, carry_1) = r.overflowing_sub(byte);
                let (res, carry_2) = res.overflowing_sub(carry_flag);
                let carry = carry_1 || carry_2;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (r & 0x0F) < (byte & 0x0F) + carry_flag);
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
            Operation::LoadRegIntoImmediateIORegister(reg) => {
                on_machine_cycle(memory);
                let register = self.fetch(memory);
                on_machine_cycle(memory);
                Self::mem_write(
                    memory,
                    0xFF00u16.wrapping_add(register as u16),
                    self.registers.get_reg_8(reg),
                );
            }
            Operation::LoadRegIntoRegIORegister(reg_a, reg_b) => {
                on_machine_cycle(memory);
                let b = self.registers.get_reg_8(reg_b) as u16;
                Self::mem_write(memory, 0xFF00u16 + b, self.registers.get_reg_8(reg_a));
            }
            Operation::AndImmediateIntoReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);

                let res = r & byte;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, true);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::AddSignedImmediateIntoWordReg(reg) => {
                on_machine_cycle(memory);
                let signed = self.fetch(memory) as i8;
                let r = self.registers.get_reg_16(reg);
                let res = r.wrapping_add_signed(signed as i16);

                // maybe inaccurate
                on_machine_cycle(memory);
                on_machine_cycle(memory);
                self.registers.set_reg_16(reg, res);

                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(
                    CpuFlag::Carry,
                    ((r & 0x00FF).wrapping_add(signed as u16 & 0x00FF)) > 0x00FF,
                );
                self.registers.set_flag(
                    CpuFlag::Half,
                    ((r & 0x000F).wrapping_add(signed as u16 & 0x000F)) > 0x000F,
                );
            }
            Operation::JumpToAddressInWordReg(wreg) => {
                let address = self.registers.get_reg_16(wreg);
                self.registers.set_reg_16(WordRegister::PC, address);
            }
            Operation::LoadRegIntoImmediateAddress(reg) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);

                on_machine_cycle(memory);
                let address = u16::from_le_bytes([low, high]);
                let byte = self.registers.get_reg_8(reg);
                Self::mem_write(memory, address, byte);
            }
            Operation::XorImmediateIntoReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);

                let res = r ^ byte;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::LoadImmediateIORegisterIntoReg(reg) => {
                on_machine_cycle(memory);
                let register = self.fetch(memory);

                on_machine_cycle(memory);
                let byte = Self::mem_read(memory, 0xFF00u16.wrapping_add(register as u16));

                self.registers.set_reg_8(reg, byte);
            }
            Operation::LoadRegIORegisterIntoReg(reg_a, reg_b) => {
                on_machine_cycle(memory);
                let a = self.registers.get_reg_8(reg_a) as u16;
                let byte = Self::mem_read(memory, 0xFF00u16 + a);

                self.registers.set_reg_8(reg_b, byte);
            }
            Operation::DisableInterrupts => {
                self.master_interrupt_flag = MasterInterrupt::Off;
            }
            Operation::OrImmediateIntoReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);

                let res = r | byte;
                self.registers.set_reg_8(reg, res);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Half, false);
                self.registers.set_flag(CpuFlag::Carry, false);
            }
            Operation::LoadSumOfWordRegAndSignedImmediateIntoWordReg(wreg_a, wreg_b) => {
                on_machine_cycle(memory);
                let signed = self.fetch(memory) as i8;

                on_machine_cycle(memory);
                let a = self.registers.get_reg_16(wreg_a);
                let res = a.wrapping_add_signed(signed as i16);
                self.registers.set_reg_16(wreg_b, res);

                self.registers.set_flag(CpuFlag::Negative, false);
                self.registers.set_flag(CpuFlag::Zero, false);
                self.registers.set_flag(
                    CpuFlag::Half,
                    ((a & 0x000F).wrapping_add(signed as u16 & 0x000F)) > 0x000F,
                );
                self.registers.set_flag(
                    CpuFlag::Carry,
                    ((a & 0x00FF).wrapping_add(signed as u16 & 0x00FF)) > 0x00FF,
                );
            }
            Operation::LoadWordRegIntoWordReg(wreg_a, wreg_b) => {
                on_machine_cycle(memory);
                let a = self.registers.get_reg_16(wreg_a);
                self.registers.set_reg_16(wreg_b, a);
            }
            Operation::LoadAtImmediateAddressIntoReg(reg) => {
                on_machine_cycle(memory);
                let low = self.fetch(memory);
                on_machine_cycle(memory);
                let high = self.fetch(memory);

                on_machine_cycle(memory);
                let address = u16::from_le_bytes([low, high]);
                self.registers
                    .set_reg_8(reg, Self::mem_read(memory, address));
            }
            Operation::EnableInterrupts => {
                self.master_interrupt_flag = MasterInterrupt::TurningOn;
            }
            Operation::CompareImmediateAndReg(reg) => {
                on_machine_cycle(memory);
                let byte = self.fetch(memory);
                let r = self.registers.get_reg_8(reg);
                let (res, carry) = r.overflowing_sub(byte);

                self.registers.set_flag(CpuFlag::Zero, res == 0);
                self.registers.set_flag(CpuFlag::Negative, true);
                self.registers
                    .set_flag(CpuFlag::Half, (r & 0x0F) < (byte & 0x0F));
                self.registers.set_flag(CpuFlag::Carry, carry);
            }
        }
    }
}
