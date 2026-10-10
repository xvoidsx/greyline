// greyline v0.1 — xvoidsx terminal music player
// radio (ffmpeg pipe) + local $HOME/Music, katakana rain, nightshadeNeon

mod music;
mod player;
mod radio;
mod rain;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use player::GreyPlayer;
use radio::Station;
use rain::Rain;
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

// nightshadeNeon
const NEON_PINK: Color = Color::Rgb(255, 45, 149);
const NEON_GREEN: Color = Color::Rgb(57, 255, 20);
const NEON_CYAN: Color = Color::Rgb(0, 255, 255);
const DIM: Color = Color::Rgb(110, 110, 110);

struct FallbackStation {
    name: &'static str,
    url: &'static str,
}

// v0.1 fallback: curated. Live discovery via radio-browser is primary.
const FALLBACK_STATIONS: &[FallbackStation] = &[
    FallbackStation { name: "SomaFM Groove Salad", url: "https://ice1.somafm.com/groovesalad-128-mp3" },
    FallbackStation { name: "SomaFM Deep Space One", url: "https://ice1.somafm.com/deepspaceone-128-mp3" },
    FallbackStation { name: "lofi (laut.fm)", url: "https://stream.laut.fm/lofi" },
    FallbackStation { name: "Dance Wave!", url: "https://dancewave.online/dance.mp3" },
    FallbackStation { name: "REYFM #original", url: "https://listen.reyfm.de/original_192kbps.mp3" },
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Radio,
    Local,
}

struct App {
    tab: Tab,
    stations: Vec<Station>,
    radio_state: ListState,
    local_state: ListState,
    local_files: Vec<PathBuf>,
    player: GreyPlayer,
    rain: Rain,
    volume: f32,
    status: String,
    // search mode
    searching: bool,
    search_input: String,
}

impl App {
    fn new() -> Result<Self> {
        let mut radio_state = ListState::default();
        radio_state.select(Some(0));
        let mut local_state = ListState::default();
        local_state.select(Some(0));
        let local_files = music::scan_music_dir();
        let player = GreyPlayer::new()?;

        // try live station discovery, fall back to curated list
        let stations = radio::top_voted(20).unwrap_or_else(|_| {
            FALLBACK_STATIONS
                .iter()
                .map(|s| Station {
                    name: s.name.to_string(),
                    url: s.url.to_string(),
                    codec: String::new(),
                    bitrate: 0,
                    country: String::new(),
                    tags: String::new(),
                })
                .collect()
        });

        Ok(Self {
            tab: Tab::Radio,
            stations,
            radio_state,
            local_state,
            local_files,
            player,
            rain: Rain::new(),
            volume: 0.8,
            status: String::new(),
            searching: false,
            search_input: String::new(),
        })
    }

    fn selected_station(&self) -> Option<&Station> {
        self.radio_state.selected().and_then(|i| self.stations.get(i))
    }

    fn do_search(&mut self) {
        let query = self.search_input.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.status = format!("searching for '{query}'...");
        match radio::search(&query, 20) {
            Ok(results) => {
                if results.is_empty() {
                    self.status = format!("no stations found for '{query}'");
                } else {
                    self.status = format!("{} stations for '{query}'", results.len());
                    self.stations = results;
                    self.radio_state.select(Some(0));
                }
            }
            Err(e) => self.status = format!("search failed: {e:#}"),
        }
        self.searching = false;
        self.search_input.clear();
    }

    fn selected_file(&self) -> Option<&PathBuf> {
        self.local_state.selected().and_then(|i| self.local_files.get(i))
    }

    fn play_selected(&mut self) {
        // clone selection data first to satisfy the borrow checker
        let radio_pick = match self.tab {
            Tab::Radio => self
                .selected_station()
                .map(|s| (s.name.clone(), s.url.clone())),
            Tab::Local => None,
        };
        let local_pick = match self.tab {
            Tab::Local => self.selected_file().cloned(),
            Tab::Radio => None,
        };

        if let Some((name, url)) = radio_pick {
            match self.player.play_stream(&url, &name) {
                Ok(()) => {
                    self.player.set_volume(self.volume);
                    self.status = format!("tuning in: {}", name);
                }
                Err(e) => self.status = format!("stream failed: {e:#}"),
            }
        } else if let Some(path) = local_pick {
            match self.player.play_file(&path) {
                Ok(()) => {
                    self.player.set_volume(self.volume);
                    self.status.clear();
                }
                Err(e) => self.status = format!("play failed: {e:#}"),
            }
        }
    }

    fn move_selection(&mut self, dir: i32) {
        let (state, len) = match self.tab {
            Tab::Radio => (&mut self.radio_state, self.stations.len()),
            Tab::Local => (&mut self.local_state, self.local_files.len()),
        };
        if len == 0 {
            return;
        }
        let cur = state.selected().unwrap_or(0) as i32;
        let next = (cur + dir).rem_euclid(len as i32) as usize;
        state.select(Some(next));
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;
    let res = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("greyline: {e:#}");
    }
    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // search input mode: type, Enter to search, Esc to cancel
                if app.searching {
                    match key.code {
                        KeyCode::Enter => app.do_search(),
                        KeyCode::Esc => {
                            app.searching = false;
                            app.search_input.clear();
                        }
                        KeyCode::Backspace => {
                            app.search_input.pop();
                        }
                        KeyCode::Char(c) => {
                            app.search_input.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => {
                        app.tab = match app.tab {
                            Tab::Radio => Tab::Local,
                            Tab::Local => Tab::Radio,
                        };
                    }
                    KeyCode::Char('/') => {
                        if app.tab == Tab::Radio {
                            app.searching = true;
                            app.search_input.clear();
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Enter => app.play_selected(),
                    KeyCode::Char(' ') => {
                        app.player.toggle_pause();
                    }
                    KeyCode::Char('s') => app.player.stop(),
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        app.volume = (app.volume + 0.05).min(1.0);
                        app.player.set_volume(app.volume);
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        app.volume = (app.volume - 0.05).max(0.0);
                        app.player.set_volume(app.volume);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let buf = f.buffer_mut();

    // clear + rain background (translucent — rain shows through gaps)
    buf.merge(&Buffer::empty(area));
    app.rain.tick(area, buf);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(8),    // list
            Constraint::Length(5), // now playing + help
        ])
        .split(area);

    // header: GREYLINE + tabs
    let tab_style = |active: bool| {
        if active {
            Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        }
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled("G R E Y L I N E", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled("[radio]", tab_style(app.tab == Tab::Radio)),
        Span::raw(" "),
        Span::styled("[local]", tab_style(app.tab == Tab::Local)),
        Span::styled("  (tab to switch)", Style::default().fg(DIM)),
    ]));
    f.render_widget(header, chunks[0]);

    // main list
    let (items, state, title) = match app.tab {
        Tab::Radio => {
            let items: Vec<ListItem> = app
                .stations
                .iter()
                .map(|s| {
                    let label = if s.bitrate > 0 {
                        format!("{} ({}k)", s.name, s.bitrate)
                    } else {
                        s.name.clone()
                    };
                    ListItem::new(Line::from(vec![Span::raw(label)]))
                })
                .collect();
            (items, &mut app.radio_state, "internet radio  (/ to search)")
        }
        Tab::Local => {
            let items: Vec<ListItem> = app
                .local_files
                .iter()
                .map(|p| ListItem::new(Line::from(vec![Span::raw(music::display_name(p))])))
                .collect();
            let items = if items.is_empty() {
                vec![ListItem::new(Line::from(vec![Span::styled(
                    "~/Music is empty — drop some files in there",
                    Style::default().fg(DIM),
                )]))]
            } else {
                items
            };
            (items, &mut app.local_state, "$HOME/Music")
        }
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NEON_GREEN)),
        )
        .highlight_style(
            Style::default()
                .fg(NEON_PINK)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[1], state);

    // now playing + help
    let np = if let Some(name) = &app.player.now_playing {
        let state = if app.player.is_paused() { "paused" } else { "playing" };
        format!("{state}: {name}")
    } else {
        "nothing playing — pick something and hit enter".to_string()
    };
    let vol = format!("vol {}%", (app.volume * 100.0) as u8);
    let status = if app.searching {
        format!("search stations: {}_", app.search_input)
    } else if app.status.is_empty() {
        "j/k move · enter play · space pause · s stop · / search · +/- volume · q quit".to_string()
    } else {
        app.status.clone()
    };
    let bottom = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("♪ ", Style::default().fg(NEON_CYAN)),
            Span::styled(np, Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(vol, Style::default().fg(DIM)),
        ]),
        Line::from(vec![Span::styled(status, Style::default().fg(DIM))]),
    ]);
    f.render_widget(bottom, chunks[2]);
}
