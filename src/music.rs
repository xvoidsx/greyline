// $HOME/Music scanner — flat list of playable files

use std::path::PathBuf;

const EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "opus"];

pub fn scan_music_dir() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(music) = dirs::audio_dir() else {
        return out;
    };
    scan_dir(&music, &mut out);
    out.sort();
    out
}

fn scan_dir(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                out.push(path);
            }
        }
    }
}

pub fn display_name(path: &std::path::Path) -> String {
    // "Artist/Album/01 - Title.flac" -> "Title"
    // fallback: file stem
    path.file_stem()
        .map(|s| {
            let s = s.to_string_lossy().to_string();
            // strip leading track numbers like "01 - "
            s.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '-' || c == '.')
                .to_string()
        })
        .unwrap_or_default()
}
