// FFT visualizer — real frequency data from the audio tap buffer

use crate::player::SampleBuffer;
use rustfft::{num_complex::Complex, FftPlanner};

/// Compute `n_bars` frequency magnitudes from the sample ring buffer.
/// Uses logarithmic frequency scaling (bass gets more bars).
pub fn fft_bars(samples: &SampleBuffer, n_bars: usize) -> Vec<f32> {
    let data: Vec<f32> = match samples.lock() {
        Ok(ring) => {
            if ring.len() < 1024 {
                return vec![0.0; n_bars];
            }
            // take the most recent 2048 samples
            ring.iter().rev().take(2048).rev().copied().collect()
        }
        Err(_) => return vec![0.0; n_bars],
    };

    let n = 2048;
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);

    // apply Hann window to reduce spectral leakage
    let mut buffer: Vec<Complex<f32>> = data
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos());
            Complex::new(s * w, 0.0)
        })
        .collect();

    // zero-pad if we have fewer than n samples
    while buffer.len() < n {
        buffer.push(Complex::new(0.0, 0.0));
    }
    buffer.truncate(n);

    fft.process(&mut buffer);

    // magnitudes of first half (positive frequencies)
    let mags: Vec<f32> = buffer[..n / 2].iter().map(|c| c.norm()).collect();

    // logarithmic binning: bass (20-250Hz) gets more bars
    // sample rate 44100, n=2048 → each bin = 21.5Hz
    let mut bars = vec![0.0; n_bars];
    for (i, bar) in bars.iter_mut().enumerate() {
        // log scale: bin = exp(ln(1) + i/n * ln(1024))
        let lo = (1.0f32.exp() * (i as f32 / n_bars as f32 * 6.93).exp()) as usize;
        let hi = (1.0f32.exp() * ((i + 1) as f32 / n_bars as f32 * 6.93).exp()) as usize;
        let lo = lo.min(mags.len() - 1).max(1);
        let hi = hi.min(mags.len()).max(lo + 1);
        let avg: f32 = mags[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        // normalize and compress (log scale for perceptual loudness)
        *bar = (avg / 50.0).min(1.0);
        *bar = (*bar * 9.0 + 1.0).ln() / 9.0f32.ln(); // log compression
    }

    bars
}
