use crossterm::event::{Event, KeyCode};
use std::io;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Tabs},
    Frame,
};

pub trait Tab<'a> {
    fn title(&self) -> &'a str;
    fn draw(
        &mut self,
        f: &mut Frame<CrosstermBackend<io::Stdout>>,
        area: Rect,
    ) -> anyhow::Result<AppAction>;
    fn input(&mut self, event: Event) -> anyhow::Result<AppAction>;
    fn focus(&mut self) -> anyhow::Result<AppAction>;
}

pub enum AppAction {
    None,
    FocusTabs,
    Quit,
}

pub struct App<'a> {
    target_fps: u8,
    tabs: Vec<Box<dyn Tab<'a>>>,
    selected_tab: usize,
    tab_focused: bool,
    should_quit: bool,
}

impl<'a> App<'a> {
    pub fn new(tabs: Vec<Box<dyn Tab>>, target_fps: u8) -> App {
        App {
            target_fps,
            tabs,
            selected_tab: 0,
            should_quit: false,
            tab_focused: false,
        }
    }

    pub fn target_fps(&self) -> u8 {
        self.target_fps
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn previous_tab(&mut self) {
        if self.selected_tab == 0 {
            self.selected_tab = self.tabs.len() - 1;
        } else {
            self.selected_tab -= 1;
        }
    }

    pub fn next_tab(&mut self) {
        if self.selected_tab == self.tabs.len() - 1 {
            self.selected_tab = 0;
        } else {
            self.selected_tab += 1;
        }
    }

    fn handle_action(&mut self, action: AppAction) {
        match action {
            AppAction::None => (),
            AppAction::FocusTabs => self.tab_focused = false,
            AppAction::Quit => self.should_quit = true,
        }
    }

    pub fn draw(&mut self, f: &mut Frame<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
        // draw tab menu, then draw active tab
        let size = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(size);

        let titles = self
            .tabs
            .iter()
            .map(|t| t.title())
            .map(Spans::from)
            .collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tabs")
                    .title_alignment(Alignment::Center),
            )
            .select(self.selected_tab)
            .style(Style::default().fg(if self.tab_focused {
                Color::DarkGray
            } else {
                Color::White
            }))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    .fg(if self.tab_focused {
                        Color::Green
                    } else {
                        Color::LightGreen
                    }),
            );

        f.render_widget(tabs, chunks[0]);
        let action = self.tabs[self.selected_tab].draw(f, chunks[1])?;
        self.handle_action(action);

        Ok(())
    }

    pub fn input(&mut self, event: Event) -> anyhow::Result<()> {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Esc {
                self.should_quit = true;
            }
        }

        // if tab focused, forward input
        if self.tab_focused {
            let action = self.tabs[self.selected_tab].input(event)?;
            self.handle_action(action);
            return Ok(());
        }

        // else, tab selector is focused
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Left => self.previous_tab(),
                KeyCode::Right => self.next_tab(),
                KeyCode::Down => {
                    // focus on tab
                    self.tab_focused = true;
                    let action = self.tabs[self.selected_tab].focus()?;
                    self.handle_action(action);
                }
                _ => (),
            }
        }

        Ok(())
    }
}

pub fn run_app(
    terminal: &mut tui::Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> anyhow::Result<()> {
    let frame_time = std::time::Duration::from_secs_f64(1.0 / app.target_fps() as f64);
    loop {
        let start_instant = std::time::Instant::now();
        let mut res = Ok(());
        terminal.draw(|f| {
            res = app.draw(f);
        })?;

        res?;

        let mut left = frame_time.saturating_sub(start_instant.elapsed());
        while !left.is_zero() {
            if app.should_quit() {
                return Ok(());
            }

            if crossterm::event::poll(left)? {
                app.input(crossterm::event::read()?)?;
            }

            left = frame_time.saturating_sub(start_instant.elapsed());
        }
    }
}
