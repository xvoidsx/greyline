// audio engine — rodio 0.22 Player + ffmpeg PCM pipe for radio streams

use anyhow::{Context, Result};
use rodio::{buffer::SamplesBuffer, Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::fs::File;
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::num::{NonZeroU16, NonZeroU32};

pub struct GreyPlayer {
    _sink: MixerDeviceSink,
    player: Arc<Player>,
    pub now_playing: Option<String>,
    // keep the ffmpeg child alive while streaming
    _stream_child: Option<std::process::Child>,
}

impl GreyPlayer {
    pub fn new() -> Result<Self> {
        let sink = DeviceSinkBuilder::open_default_sink().context("no audio device")?;
        let player = Arc::new(Player::connect_new(sink.mixer()));
        Ok(Self {
            _sink: sink,
            player,
            now_playing: None,
            _stream_child: None,
        })
    }

    fn clear_stream(&mut self) {
        if let Some(mut child) = self._stream_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.player.clear();
    }

    /// Play an internet radio stream URL via ffmpeg -> PCM pipe.
    pub fn play_stream(&mut self, url: &str, label: &str) -> Result<()> {
        self.clear_stream();

        let mut child = Command::new("ffmpeg")
            .args([
                "-v", "quiet", "-i", url,
                "-f", "s16le", "-ac", "2", "-ar", "44100",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("ffmpeg not found — needed for radio streams")?;

        let mut stdout = child.stdout.take().context("no ffmpeg stdout")?;

        // pump PCM chunks into the player on a background thread
        let player = Arc::clone(&self.player);
        std::thread::spawn(move || {
            let mut buf = [0u8; 16384]; // 4096 stereo samples
            loop {
                match stdout.read_exact(&mut buf) {
                    Ok(()) => {
                        let samples: Vec<f32> = buf
                            .chunks_exact(2)
                            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                            .collect();
                        let source = SamplesBuffer::new(
                            NonZeroU16::new(2).unwrap(),
                            NonZeroU32::new(44100).unwrap(),
                            samples,
                        );
                        player.append(source);
                    }
                    Err(_) => break,
                }
            }
        });

        self._stream_child = Some(child);
        self.player.play(); // ensure we're not stuck paused from before
        self.now_playing = Some(label.to_string());
        Ok(())
    }

    /// Play a local audio file.
    pub fn play_file(&mut self, path: &std::path::Path) -> Result<()> {
        self.clear_stream();
        let file = File::open(path).context("could not open file")?;
        let source = Decoder::new(BufReader::new(file)).context("could not decode file")?;
        self.player.append(source);
        self.player.play();
        self.now_playing = Some(
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
        Ok(())
    }

    pub fn toggle_pause(&self) {
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
        }
    }

    pub fn stop(&mut self) {
        self.clear_stream();
        self.now_playing = None;
    }

    pub fn set_volume(&self, v: f32) {
        self.player.set_volume(v.clamp(0.0, 1.0));
    }

    pub fn volume(&self) -> f32 {
        self.player.volume()
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn is_playing(&self) -> bool {
        self.now_playing.is_some()
    }
}
