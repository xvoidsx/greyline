// audio engine — unified ffmpeg PCM pipeline for streams AND local files
// samples are tapped to a shared ring buffer for the FFT visualizer

use anyhow::{Context, Result};
use rodio::{buffer::SamplesBuffer, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::collections::VecDeque;
use std::io::Read;
use std::num::{NonZeroU16, NonZeroU32};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// Shared ring buffer of recent mono samples for the visualizer.
/// Written by the ffmpeg pump thread, read by the UI thread.
pub type SampleBuffer = Arc<Mutex<VecDeque<f32>>>;

pub struct GreyPlayer {
    _sink: MixerDeviceSink,
    player: Arc<Player>,
    pub now_playing: Option<String>,
    pub samples: SampleBuffer,
    // keep the ffmpeg child alive while playing
    _ffmpeg_child: Option<std::process::Child>,
}

impl GreyPlayer {
    pub fn new() -> Result<Self> {
        let sink = DeviceSinkBuilder::open_default_sink().context("no audio device")?;
        let player = Arc::new(Player::connect_new(sink.mixer()));
        Ok(Self {
            _sink: sink,
            player,
            now_playing: None,
            samples: Arc::new(Mutex::new(VecDeque::with_capacity(8192))),
            _ffmpeg_child: None,
        })
    }

    fn clear_playback(&mut self) {
        if let Some(mut child) = self._ffmpeg_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.player.clear();
        if let Ok(mut buf) = self.samples.lock() {
            buf.clear();
        }
    }

    /// Spawn ffmpeg decoding `input` (URL or file path) to PCM,
    /// pump it to the player, and tap samples for the visualizer.
    fn play_ffmpeg(&mut self, input: &str, label: &str) -> Result<()> {
        self.clear_playback();

        let mut child = Command::new("ffmpeg")
            .args([
                "-v", "quiet", "-i", input,
                "-f", "s16le", "-ac", "1", "-ar", "44100", // mono for FFT
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("ffmpeg not found")?;

        let mut stdout = child.stdout.take().context("no ffmpeg stdout")?;

        let player = Arc::clone(&self.player);
        let samples = Arc::clone(&self.samples);

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192]; // 4096 mono samples
            loop {
                match stdout.read_exact(&mut buf) {
                    Ok(()) => {
                        let pcm: Vec<f32> = buf
                            .chunks_exact(2)
                            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                            .collect();

                        // tap for visualizer (keep last 8192)
                        if let Ok(mut ring) = samples.lock() {
                            ring.extend(pcm.iter().copied());
                            while ring.len() > 8192 {
                                ring.pop_front();
                            }
                        }

                        // stereo-expand for playback (mono -> stereo)
                        let stereo: Vec<f32> = pcm.iter().flat_map(|&s| [s, s]).collect();
                        let source = SamplesBuffer::new(
                            NonZeroU16::new(2).unwrap(),
                            NonZeroU32::new(44100).unwrap(),
                            stereo,
                        );
                        player.append(source);
                    }
                    Err(_) => break,
                }
            }
        });

        self._ffmpeg_child = Some(child);
        self.player.play();
        self.now_playing = Some(label.to_string());
        Ok(())
    }

    /// Play an internet radio stream URL.
    pub fn play_stream(&mut self, url: &str, label: &str) -> Result<()> {
        self.play_ffmpeg(url, label)
    }

    /// Play a local audio file (via ffmpeg — unified pipeline).
    pub fn play_file(&mut self, path: &std::path::Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.play_ffmpeg(&path_str, &label)
    }

    pub fn toggle_pause(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn stop(&mut self) {
        self.clear_playback();
        self.now_playing = None;
    }

    pub fn set_volume(&self, v: f32) {
        self.player.set_volume(v.clamp(0.0, 1.0));
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn is_playing(&self) -> bool {
        self.now_playing.is_some()
    }
}
