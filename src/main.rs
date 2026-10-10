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
    widgets::{Block, Borders, BorderType, Clear, List, ListItem, ListState, Paragraph},
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

#[derive(Clone, Copy, PartialEq)]
enum View {
    List,
    NowPlaying,
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
    show_help: bool,
    viz_mode: usize, // 0=off, 1=bars, 2=wave, 3=dots
    viz_tick: u64,
    view: View,
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
            show_help: false,
            viz_mode: 1,
            viz_tick: 0,
            view: View::List,
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
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if app.show_help {
                            app.show_help = false;
                        } else if app.view == View::NowPlaying {
                            app.view = View::List;
                        } else if app.searching {
                            app.searching = false;
                            app.search_input.clear();
                        } else {
                            break;
                        }
                    }
                    KeyCode::Char('?') => {
                        app.show_help = !app.show_help;
                    }
                    KeyCode::Enter => {
                        if app.view == View::List {
                            app.play_selected();
                            // jump to now-playing view on successful play
                            if app.player.is_playing() {
                                app.view = View::NowPlaying;
                            }
                        }
                    }
                    KeyCode::Char('v') => {
                        app.viz_mode = (app.viz_mode + 1) % 4;
                        let names = ["off", "bars", "wave", "dots"];
                        app.status = format!("visualizer: {}", names[app.viz_mode]);
                    }
                    KeyCode::Tab => {
                        app.tab = match app.tab {
                            Tab::Radio => Tab::Local,
                            Tab::Local => Tab::Radio,
                        };
                    }
                    KeyCode::Char('1') => app.tab = Tab::Radio,
                    KeyCode::Char('2') => app.tab = Tab::Local,
                    KeyCode::Char('/') => {
                        if app.tab == Tab::Radio {
                            app.searching = true;
                            app.search_input.clear();
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
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

    // Clear widget properly resets the buffer — merge(Buffer::empty)
    // does NOT clear (it skips empty cells)
    f.render_widget(Clear, area);

    match app.view {
        View::List => draw_list_view(f, app, area),
        View::NowPlaying => draw_now_playing(f, app, area),
    }

    // help overlay on top of everything
    if app.show_help {
        draw_help(f, area);
    }
}

fn draw_list_view(f: &mut Frame, app: &mut App, area: Rect) {
    // rain DISABLED for debugging — testing if UI stabilizes without it
    // {
    //     let buf = f.buffer_mut();
    //     app.rain.tick(area, buf);
    // }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(8),    // list
            Constraint::Length(5), // now playing + help
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_list(f, app, chunks[1]);
    draw_bottom(f, app, chunks[2]);

    app.viz_tick += 1;
}

fn draw_now_playing(f: &mut Frame, app: &mut App, area: Rect) {
    // rain DISABLED for debugging
    // {
    //     let buf = f.buffer_mut();
    //     app.rain.tick(area, buf);
    // }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(4)
        .constraints([
            Constraint::Length(3),  // title
            Constraint::Length(10), // big visualizer (fixed, not Min)
            Constraint::Length(3),  // controls
            Constraint::Min(0),     // spacer (absorbs extra height)
        ])
        .split(area);

    // station/file name, big and pink
    let name = app.player.now_playing.clone().unwrap_or_default();
    let title = Paragraph::new(Line::from(vec![Span::styled(
        name,
        Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
    )]))
    .alignment(ratatui::layout::Alignment::Center)
    .style(Style::default().bg(Color::Rgb(5, 5, 8)));
    f.render_widget(title, chunks[0]);

    // big visualizer
    draw_big_visualizer(f, app, chunks[1]);

    // controls
    let controls = Paragraph::new(Line::from(vec![
        Span::styled("space", Style::default().fg(NEON_CYAN)),
        Span::raw(" pause · "),
        Span::styled("esc", Style::default().fg(NEON_CYAN)),
        Span::raw(" back · "),
        Span::styled("v", Style::default().fg(NEON_CYAN)),
        Span::raw(" visualizer · "),
        Span::styled("q", Style::default().fg(NEON_CYAN)),
        Span::raw(" quit"),
    ]))
    .alignment(ratatui::layout::Alignment::Center)
    .style(Style::default().bg(Color::Rgb(5, 5, 8)));
    f.render_widget(controls, chunks[2]);

    app.viz_tick += 1;
}

fn draw_big_visualizer(f: &mut Frame, app: &App, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }
    let playing = app.player.is_playing() && !app.player.is_paused();
    let width = area.width as usize;
    let height = area.height as usize;
    let tick = app.viz_tick;

    let mut lines = Vec::new();
    for row in 0..height {
        let line: String = (0..width)
            .map(|x| {
                if !playing {
                    return ' ';
                }
                // layered sines create organic motion, -1.0..1.0
                let t = tick as f32 * 0.12;
                let wave = (t + x as f32 * 0.15 + row as f32 * 0.5).sin() * 0.5
                    + (t * 1.3 + x as f32 * 0.07).sin() * 0.3
                    + (t * 0.7 + row as f32 * 0.3).sin() * 0.2;
                match app.viz_mode {
                    1 => {
                        // bars grow from the bottom: bar height 0..height
                        let bar_h = ((wave * 0.5 + 0.5) * height as f32) as usize;
                        if row >= height.saturating_sub(bar_h) {
                            '█'
                        } else {
                            ' '
                        }
                    }
                    2 => {
                        // wave: line where wave crosses row
                        let y = wave * height as f32 / 2.0 + height as f32 / 2.0;
                        if (y as usize) == row { '─' } else { ' ' }
                    }
                    3 => {
                        // dots: sparse sparkle
                        if (tick as usize + x * 7 + row * 13) % 11 == 0 && wave > 0.0 {
                            '･'
                        } else {
                            ' '
                        }
                    }
                    _ => ' ',
                }
            })
            .collect();
        let color = match app.viz_mode {
            1 => NEON_CYAN,
            2 => NEON_GREEN,
            3 => NEON_PINK,
            _ => DIM,
        };
        lines.push(Line::from(vec![Span::styled(line, Style::default().fg(color))]));
    }

    let viz = Paragraph::new(lines);
    f.render_widget(viz, area);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let tab_style = |active: bool| {
        if active {
            Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        }
    };
    // solid bg — rain stays in the margins, text stays pristine
    let header = Paragraph::new(Line::from(vec![
        Span::styled("G R E Y L I N E", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled("[1:radio]", tab_style(app.tab == Tab::Radio)),
        Span::raw(" "),
        Span::styled("[2:local]", tab_style(app.tab == Tab::Local)),
        Span::styled("  (? for help)", Style::default().fg(DIM)),
    ]))
    .style(Style::default().bg(Color::Rgb(5, 5, 8)));
    f.render_widget(header, area);
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
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
                .border_style(Style::default().fg(NEON_GREEN))
                .style(Style::default().bg(Color::Rgb(5, 5, 8))),
        )
        .highlight_style(
            Style::default()
                .fg(NEON_PINK)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, state);
}

fn draw_bottom(f: &mut Frame, app: &App, area: Rect) {
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
        "j/k move · enter play · space pause · s stop · / search · ? help · q quit".to_string()
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
    ])
    .style(Style::default().bg(Color::Rgb(5, 5, 8)));
    f.render_widget(bottom, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled("greyline help", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD))]),
        Line::from(""),
        Line::from(vec![Span::styled("j/k or arrows", Style::default().fg(NEON_CYAN)), Span::raw("  move selection")]),
        Line::from(vec![Span::styled("enter", Style::default().fg(NEON_CYAN)), Span::raw("         play selected")]),
        Line::from(vec![Span::styled("space", Style::default().fg(NEON_CYAN)), Span::raw("         pause/resume")]),
        Line::from(vec![Span::styled("s", Style::default().fg(NEON_CYAN)), Span::raw("             stop")]),
        Line::from(vec![Span::styled("tab / 1 / 2", Style::default().fg(NEON_CYAN)), Span::raw("   switch radio/local")]),
        Line::from(vec![Span::styled("/", Style::default().fg(NEON_CYAN)), Span::raw("             search stations (radio tab)")]),
        Line::from(vec![Span::styled("+ / -", Style::default().fg(NEON_CYAN)), Span::raw("         volume up/down")]),
        Line::from(vec![Span::styled("?", Style::default().fg(NEON_CYAN)), Span::raw("             this help")]),
        Line::from(vec![Span::styled("q / esc", Style::default().fg(NEON_CYAN)), Span::raw("       quit")]),
        Line::from(""),
        Line::from(vec![Span::styled("press ? or esc to close", Style::default().fg(DIM))]),
    ];

    let width = 40;
    let height = help_text.len() as u16 + 2;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let help_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    // clear behind help
    let buf = f.buffer_mut();
    for hy in help_area.y..help_area.y + help_area.height {
        for hx in help_area.x..help_area.x + help_area.width {
            let cell = &mut buf[(hx, hy)];
            cell.set_char(' ');
            cell.set_style(Style::default().bg(Color::Rgb(10, 10, 15)));
        }
    }

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(NEON_PINK)),
        );
    f.render_widget(help, help_area);
}
