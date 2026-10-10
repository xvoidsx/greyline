# greyline

xvoidsx's terminal music player. navi-radio's terminal cousin.

## v0.1 — tonight's build

- **internet radio** — live discovery via radio-browser.info (top voted on
  start, `/` to search 30k+ stations), ffmpeg PCM pipe for playback
- **local music** — scans `$HOME/Music`, plays mp3/flac/ogg/wav/m4a/opus
- **nightshadeNeon TUI** — katakana rain drifting behind everything, neon pink/cyan
- **keyboard-first** — `j/k` move, `enter` play, `space` pause, `s` stop,
  `tab` switch tabs, `/` search stations, `+/-` volume, `q` quit

## roadmap

- [x] searchable stations (radio-browser.info — live in v0.1!)
- [ ] visualizer (FFT via rustfft on the PCM stream)
- [ ] favorites + config persistence
- [ ] mpris integration for desktop media keys

## build

```bash
cargo build --release
```

Requires `ffmpeg` for radio streams (local files don't need it).

## license

MIT — xvoidsx
