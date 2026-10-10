// nightshadeNeon TUI — katakana rain, station browser, now-playing bar.
// (ratatui layout + cyberia-style rain; full UI lands in phase 2.)

// nightshadeNeon palette (shared with cyberia/navi)
pub mod palette {
    use ratatui::style::Color;
    pub const NEON_PINK: Color = Color::Rgb(255, 45, 149);
    pub const NEON_GREEN: Color = Color::Rgb(57, 255, 20);
    pub const NEON_CYAN: Color = Color::Rgb(0, 255, 255);
    pub const VOID: Color = Color::Rgb(5, 5, 8);
}
