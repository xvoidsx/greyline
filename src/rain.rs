//! katakana rain — borrowed from cyberia's splash, standalone.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub struct XorShift32(pub u32);

impl XorShift32 {
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: u32) -> u32 {
        self.next() % n.max(1)
    }

    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() as f32 / u32::MAX as f32) * (hi - lo)
    }
}

pub fn kana(rng: &mut XorShift32) -> char {
    char::from_u32(0x30A0 + rng.below(96)).unwrap_or('\u{30A0}')
}

pub struct RainColumn {
    pub x: u16,
    pub head_y: f32,
    pub speed: f32,
    pub len: u16,
    pub delay: u8,
    pub glyphs: Vec<char>,
}

impl RainColumn {
    pub fn new(rng: &mut XorShift32, width: u16, stagger: bool) -> Self {
        let len = 5 + rng.below(11) as u16;
        let glyphs = (0..len).map(|_| kana(rng)).collect();
        Self {
            x: (rng.below(width as u32)).min(width.saturating_sub(1) as u32) as u16,
            head_y: -(rng.below(24) as f32),
            speed: rng.range_f32(0.4, 1.2),
            len,
            delay: if stagger { rng.below(6) as u8 } else { 0 },
            glyphs,
        }
    }

    pub fn respawn(&mut self, rng: &mut XorShift32, width: u16) {
        *self = Self::new(rng, width, false);
        self.head_y = -(rng.below(10) as f32);
        self.delay = rng.below(4) as u8;
    }

    pub fn substep(&mut self, rng: &mut XorShift32, height: u16, width: u16) {
        if self.delay > 0 {
            self.delay -= 1;
            return;
        }
        self.head_y += self.speed;
        if rng.below(4) == 0 && !self.glyphs.is_empty() {
            let i = rng.below(self.glyphs.len() as u32) as usize;
            self.glyphs[i] = kana(rng);
        }
        if self.head_y - self.len as f32 > height as f32 {
            self.respawn(rng, width);
        }
    }
}

/// neon green head -> cyan -> dark teal -> black
pub fn rain_color(t: f32) -> Color {
    const STOPS: [(f32, (u8, u8, u8)); 4] = [
        (0.0, (57, 255, 20)),
        (0.35, (0, 255, 255)),
        (0.7, (0, 100, 100)),
        (1.0, (0, 0, 0)),
    ];
    let t = t.clamp(0.0, 1.0);
    let mut prev = STOPS[0];
    for &stop in &STOPS[1..] {
        if t <= stop.0 {
            let span = (stop.0 - prev.0).max(f32::EPSILON);
            let f = (t - prev.0) / span;
            let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f) as u8;
            return Color::Rgb(
                lerp(prev.1 .0, stop.1 .0),
                lerp(prev.1 .1, stop.1 .1),
                lerp(prev.1 .2, stop.1 .2),
            );
        }
        prev = stop;
    }
    Color::Rgb(0, 0, 0)
}

pub struct Rain {
    pub cols: Vec<RainColumn>,
    pub rng: XorShift32,
    size: Option<(u16, u16)>,
}

impl Rain {
    pub fn new() -> Self {
        Self {
            cols: Vec::new(),
            rng: XorShift32(0x6AE11E),
            size: None,
        }
    }

    /// Advance physics and draw onto buf. Dimmer than splash rain —
    /// it's a background, not the show.
    pub fn tick(&mut self, area: Rect, buf: &mut Buffer) {
        let size = (area.width, area.height);
        if self.size != Some(size) {
            self.size = Some(size);
            let ncols = (area.width / 4).max(4) as usize;
            self.cols = (0..ncols)
                .map(|_| RainColumn::new(&mut self.rng, area.width, true))
                .collect();
        }

        // split borrows: advance all, then draw all
        {
            let (cols, rng) = (&mut self.cols, &mut self.rng);
            for col in cols.iter_mut() {
                for _ in 0..2 {
                    col.substep(rng, area.height, area.width);
                }
            }
        }
        {
            let (cols, rng) = (&mut self.cols, &mut self.rng);
            for col in cols.iter_mut() {
                if rng.below(8) == 0 && !col.glyphs.is_empty() {
                    let i = rng.below(col.glyphs.len() as u32) as usize;
                    col.glyphs[i] = kana(rng);
                }
                for i in 0..col.len {
                    let gy = (col.head_y - i as f32).round() as i32;
                    if gy < 0 || gy >= area.height as i32 {
                        continue;
                    }
                    let x = area.x + col.x;
                    let y = area.y + gy as u16;
                    if x >= area.x + area.width {
                        continue;
                    }
                    let t = i as f32 / col.len as f32;
                    let (r, g, b) = match rain_color(t) {
                        Color::Rgb(r, g, b) => (r / 3, g / 3, b / 3),
                        _ => (0, 0, 0),
                    };
                    let cell = &mut buf[(x, y)];
                    // only draw on empty cells — never stomp UI text
                    if cell.symbol() == " " {
                        cell.set_char(col.glyphs[i as usize % col.glyphs.len()]);
                        cell.set_style(
                            ratatui::style::Style::default().fg(Color::Rgb(r, g, b)),
                        );
                    }
                }
            }
        }
    }
}
