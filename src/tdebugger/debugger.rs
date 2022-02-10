use super::tui_helper::*;
use crate::gameboy::{
    cpu::{self, MasterInterrupt},
    memory::registers as memreg,
    Gameboy,
};
use atomic::Atomic;
use flagset::FlagSet;
use parking_lot::Mutex;
use std::{
    io,
    sync::{atomic::AtomicBool, Arc},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState, Row, Table},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DebuggerEmulationState {
    Paused,
    DrawingDebugger,
    Stepping,
}

/// Data that's shared between the app tabs and the emulation thread.
pub struct DebuggerShared {
    pub gameboy: Mutex<Gameboy>,
    pub state: Atomic<DebuggerEmulationState>,
    pub exit: AtomicBool,
}

struct SummaryTabInner {
    address_op_cache: Box<[Option<cpu::operation::Operation>; 0xFFFF]>,
}

impl SummaryTabInner {
    pub fn new() -> Self {
        Self {
            address_op_cache: Box::new([None; 0xFFFF]),
        }
    }

    fn render_registers_area(
        &mut self,
        f: &mut tui::Frame<CrosstermBackend<io::Stdout>>,
        area: tui::layout::Rect,
        shared: &DebuggerShared,
    ) -> anyhow::Result<()> {
        // render outer block
        let block = Block::default()
            .title("Registers")
            .title_alignment(Alignment::Center)
            .borders(Borders::BOTTOM);
        f.render_widget(block, area);

        // divide registers area into two
        let registers_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .horizontal_margin(2)
            .vertical_margin(1)
            .split(area);

        let gameboy_lock = shared.gameboy.lock();
        let registers = gameboy_lock.cpu().registers();

        // render left side
        let table_left = Table::new(vec![
            Row::new(vec![
                tui::widgets::Cell::from("AF: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::AF)).into(),
            ]),
            Row::new(vec![
                tui::widgets::Cell::from("BC: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::BC)).into(),
            ]),
            Row::new(vec![
                tui::widgets::Cell::from("DE: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::DE)).into(),
            ]),
        ])
        .style(Style::default().fg(Color::White))
        .widths(&[Constraint::Length(3), Constraint::Min(6)])
        .column_spacing(1);
        f.render_widget(table_left, registers_chunks[0]);

        // render right side
        let table_right = Table::new(vec![
            Row::new(vec![
                tui::widgets::Cell::from("HL: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::HL)).into(),
            ]),
            Row::new(vec![
                tui::widgets::Cell::from("SP: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::SP)).into(),
            ]),
            Row::new(vec![
                tui::widgets::Cell::from("PC: ").style(Style::default().fg(Color::LightCyan)),
                format!("{:#06X}", registers.get_reg_16(cpu::WordRegister::PC)).into(),
            ]),
        ])
        .style(Style::default().fg(Color::White))
        .widths(&[Constraint::Length(3), Constraint::Min(6)])
        .column_spacing(1);
        f.render_widget(table_right, registers_chunks[1]);

        Ok(())
    }

    fn render_interrupts_area(
        &mut self,
        f: &mut tui::Frame<CrosstermBackend<io::Stdout>>,
        area: tui::layout::Rect,
        shared: &DebuggerShared,
    ) -> anyhow::Result<()> {
        // render outer block
        let block = Block::default()
            .title("Interrupts")
            .title_alignment(Alignment::Center);
        f.render_widget(block, area);

        // fake split area to add margin
        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .horizontal_margin(2)
            .vertical_margin(1)
            .split(area)[0];

        // collect interrupts
        let gameboy_lock = shared.gameboy.lock();
        let master = gameboy_lock.cpu().master_interrupt_flag();

        let enabled = FlagSet::<memreg::Interrupt>::new(
            gameboy_lock
                .memory()
                .read(memreg::addresses::INTERRUPT_ENABLE)
                & 0b00011111,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        let requested = FlagSet::<memreg::Interrupt>::new(
            gameboy_lock
                .memory()
                .read(memreg::addresses::INTERRUPT_REQUEST)
                & 0b00011111,
        )
        .map_err(|e| anyhow::anyhow!(e))?;

        let interrupt_list = [
            memreg::Interrupt::VBlank,
            memreg::Interrupt::STAT,
            memreg::Interrupt::Timer,
            memreg::Interrupt::Serial,
            memreg::Interrupt::Joypad,
        ];

        let mut items = Vec::with_capacity(6);
        items.push(tui::widgets::ListItem::new("Master").style(
            if let MasterInterrupt::On = master {
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::UNDERLINED)
            },
        ));

        let interrupt_items = interrupt_list
            .into_iter()
            .map(|i| (i, enabled.contains(i), requested.contains(i)))
            .map(|(i, enabled, requested)| {
                if requested {
                    (format!("{:?} (REQUESTED)", i), enabled)
                } else {
                    (format!("{:?}", i), enabled)
                }
            })
            .map(|(i, enabled)| {
                if enabled {
                    tui::widgets::ListItem::new(i).style(Style::default().fg(Color::LightGreen))
                } else {
                    tui::widgets::ListItem::new(i).style(Style::default().fg(Color::LightRed))
                }
            });
        items.extend(interrupt_items);

        let list = List::new(items)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">>");

        f.render_widget(list, area);
        Ok(())
    }

    pub fn render_cpu_area(
        &mut self,
        f: &mut tui::Frame<CrosstermBackend<io::Stdout>>,
        area: tui::layout::Rect,
        shared: &DebuggerShared,
    ) -> anyhow::Result<()> {
        // render outer block
        let block = Block::default()
            .title("CPU")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL);
        f.render_widget(block, area);

        // divide area vertically: top side for registers, bottom side for interrupts
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(10)])
            .margin(1)
            .split(area);

        self.render_registers_area(f, chunks[0], shared)?;
        self.render_interrupts_area(f, chunks[1], shared)?;

        Ok(())
    }

    pub fn render_memory_area(
        &mut self,
        f: &mut tui::Frame<CrosstermBackend<io::Stdout>>,
        area: tui::layout::Rect,
        shared: &DebuggerShared,
    ) -> anyhow::Result<()> {
        let gameboy_lock = shared.gameboy.lock();
        let boot_mode = gameboy_lock.memory().boot_mode();

        // render outer block
        let block = if boot_mode {
            Block::default()
                .title("Memory (Boot enabled)")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
        } else {
            Block::default()
                .title("Memory")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
        };
        f.render_widget(block, area);

        // fake split area to add margin
        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .horizontal_margin(2)
            .vertical_margin(1)
            .split(area)[0];

        let middle = area.height.saturating_div(2);
        let items = {
            let pc = gameboy_lock
                .cpu()
                .registers()
                .get_reg_16(cpu::WordRegister::PC);

            Vec::from_iter(
                ((pc.wrapping_sub(middle))..(pc.wrapping_add(area.height - middle)))
                    .into_iter()
                    .map(|i| (i, gameboy_lock.memory().read(i)))
                    .map(|(i, value)| {
                        let op = match i.cmp(&pc) {
                            std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
                                // let op = crate::gameboy::cpu::operation::Operation::from(value);
                                // Some(op)
                                self.address_op_cache[i as usize]
                            }
                            std::cmp::Ordering::Equal => {
                                let op = crate::gameboy::cpu::operation::Operation::from(value);
                                self.address_op_cache[i as usize] = Some(op);

                                Some(op)
                            }
                        };

                        if let Some(op) = op {
                            tui::widgets::ListItem::new(format!(
                                "({:#06X}): {:#04X} | {:?}",
                                i, value, op
                            ))
                            .style(Style::default().fg(Color::LightGreen))
                        } else {
                            tui::widgets::ListItem::new(format!("({:#06X}): {:#04X}", i, value))
                                .style(Style::default().fg(Color::LightGreen))
                        }
                    }),
            )
        };
        let list = List::new(items)
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::DIM),
            )
            .highlight_style(Style::default().remove_modifier(Modifier::DIM))
            .highlight_symbol("(PC) ");

        let mut state = ListState::default();
        state.select(Some(middle as usize));

        f.render_stateful_widget(list, area, &mut state);
        Ok(())
    }
}

pub struct SummaryTab {
    shared: Arc<DebuggerShared>,
    inner: SummaryTabInner,
}

impl SummaryTab {
    pub fn new(shared: Arc<DebuggerShared>) -> Self {
        Self {
            shared,
            inner: SummaryTabInner::new(),
        }
    }
}

impl<'a> Tab<'a> for SummaryTab {
    fn title(&self) -> &'a str {
        "Summary"
    }

    fn draw(
        &mut self,
        f: &mut tui::Frame<CrosstermBackend<io::Stdout>>,
        area: tui::layout::Rect,
    ) -> anyhow::Result<AppAction> {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),
                Constraint::Min(5),
                Constraint::Length(45),
            ])
            .split(area);

        let emulation_state = self.shared.state.load(std::sync::atomic::Ordering::SeqCst);
        match emulation_state {
            DebuggerEmulationState::Paused => {
                self.inner.render_cpu_area(f, chunks[0], &self.shared)?;
                self.inner.render_memory_area(f, chunks[1], &self.shared)?;
            }
            DebuggerEmulationState::DrawingDebugger => unreachable!(),
            DebuggerEmulationState::Stepping => {
                self.shared.state.store(
                    DebuggerEmulationState::DrawingDebugger,
                    std::sync::atomic::Ordering::SeqCst,
                );

                self.inner.render_cpu_area(f, chunks[0], &self.shared)?;
                self.inner.render_memory_area(f, chunks[1], &self.shared)?;

                self.shared.state.store(
                    DebuggerEmulationState::Stepping,
                    std::sync::atomic::Ordering::SeqCst,
                );
            }
        }

        if self.shared.exit.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(AppAction::Quit);
        }

        let block = Block::default()
            .title("Block 3")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL);
        f.render_widget(block, chunks[2]);

        Ok(AppAction::None)
    }

    fn input(&mut self, event: crossterm::event::Event) -> anyhow::Result<AppAction> {
        if let crossterm::event::Event::Key(key) = event {
            match key.code {
                crossterm::event::KeyCode::Char(c) => match c {
                    'p' => self.shared.state.store(
                        DebuggerEmulationState::Paused,
                        std::sync::atomic::Ordering::SeqCst,
                    ),
                    'r' => self.shared.state.store(
                        DebuggerEmulationState::Stepping,
                        std::sync::atomic::Ordering::SeqCst,
                    ),
                    's' => {
                        self.shared.gameboy.lock().step();
                    }
                    'v' => {
                        let lock = self.shared.gameboy.lock();
                        let mut data = vec![];
                        for i in 0x8000..0x9800 {
                            data.push(lock.memory().read(i as u16));
                        }

                        let mut file = std::fs::File::create("vram.dump").unwrap();
                        io::Write::write_all(&mut file, &data).unwrap();
                    }
                    't' => {
                        let lock = self.shared.gameboy.lock();
                        lock.ppu().dbg_save_master_tileset();
                        lock.ppu().dbg_save_current_buffer();
                    }
                    _ => (),
                },
                crossterm::event::KeyCode::Up => return Ok(AppAction::FocusTabs),
                _ => (),
            }
        }
        Ok(AppAction::None)
    }

    fn focus(&mut self) -> anyhow::Result<AppAction> {
        // ok ok
        Ok(AppAction::None)
    }
}
