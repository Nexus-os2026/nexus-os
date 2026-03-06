use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StealthProfile {
    pub mean_action_delay_secs: f64,
    pub action_delay_stddev_secs: f64,
    pub min_action_delay_ms: u64,
    pub min_typing_delay_ms: u64,
    pub max_typing_delay_ms: u64,
    pub max_session_duration_secs: u64,
    pub max_posts_per_hour: u32,
}

impl Default for StealthProfile {
    fn default() -> Self {
        Self {
            mean_action_delay_secs: 2.0,
            action_delay_stddev_secs: 0.7,
            min_action_delay_ms: 500,
            min_typing_delay_ms: 50,
            max_typing_delay_ms: 150,
            max_session_duration_secs: 2 * 60 * 60,
            max_posts_per_hour: 20,
        }
    }
}

pub fn gaussian_action_delays_ms(count: usize, profile: &StealthProfile) -> Vec<u64> {
    let mut rng = rand::thread_rng();
    gaussian_action_delays_ms_with_rng(count, profile, &mut rng)
}

pub fn gaussian_action_delays_ms_seeded(
    count: usize,
    profile: &StealthProfile,
    seed: u64,
) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(seed);
    gaussian_action_delays_ms_with_rng(count, profile, &mut rng)
}

pub fn typing_delays_ms(text: &str, profile: &StealthProfile) -> Vec<u64> {
    let mut rng = rand::thread_rng();
    typing_delays_ms_with_rng(text, profile, &mut rng)
}

pub fn typing_delays_ms_seeded(text: &str, profile: &StealthProfile, seed: u64) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(seed);
    typing_delays_ms_with_rng(text, profile, &mut rng)
}

pub fn bezier_mouse_path(
    start: (f64, f64),
    end: (f64, f64),
    control_1: (f64, f64),
    control_2: (f64, f64),
    steps: usize,
) -> Vec<(f64, f64)> {
    let samples = steps.max(2);
    let mut points = Vec::with_capacity(samples);
    for idx in 0..samples {
        let t = idx as f64 / (samples.saturating_sub(1)) as f64;
        let one_minus_t = 1.0 - t;
        let x = one_minus_t.powi(3) * start.0
            + 3.0 * one_minus_t.powi(2) * t * control_1.0
            + 3.0 * one_minus_t * t.powi(2) * control_2.0
            + t.powi(3) * end.0;
        let y = one_minus_t.powi(3) * start.1
            + 3.0 * one_minus_t.powi(2) * t * control_1.1
            + 3.0 * one_minus_t * t.powi(2) * control_2.1
            + t.powi(3) * end.1;
        points.push((x, y));
    }
    points
}

#[derive(Debug, Clone)]
pub struct SessionGuard {
    profile: StealthProfile,
    posted_timestamps_secs: Vec<u64>,
}

impl SessionGuard {
    pub fn new(profile: StealthProfile) -> Self {
        Self {
            profile,
            posted_timestamps_secs: Vec::new(),
        }
    }

    pub fn allow_session(&self, elapsed_secs: u64) -> bool {
        elapsed_secs <= self.profile.max_session_duration_secs
    }

    pub fn allow_post(&mut self, now_secs: u64) -> bool {
        let one_hour_ago = now_secs.saturating_sub(3600);
        self.posted_timestamps_secs
            .retain(|timestamp| *timestamp >= one_hour_ago);
        if self.posted_timestamps_secs.len() as u32 >= self.profile.max_posts_per_hour {
            return false;
        }
        self.posted_timestamps_secs.push(now_secs);
        true
    }
}

fn gaussian_action_delays_ms_with_rng<R: Rng + ?Sized>(
    count: usize,
    profile: &StealthProfile,
    rng: &mut R,
) -> Vec<u64> {
    let sigma = profile.action_delay_stddev_secs.max(0.05);

    let mut delays = Vec::with_capacity(count);
    for _ in 0..count {
        let sample_secs = gaussian_sample(profile.mean_action_delay_secs, sigma, rng).abs();
        let mut millis = (sample_secs * 1000.0).round() as u64;
        if millis < profile.min_action_delay_ms {
            millis = profile.min_action_delay_ms;
        }
        delays.push(millis);
    }
    delays
}

fn typing_delays_ms_with_rng<R: Rng + ?Sized>(
    text: &str,
    profile: &StealthProfile,
    rng: &mut R,
) -> Vec<u64> {
    let words = text
        .split_whitespace()
        .map(|word| word.chars().count())
        .collect::<Vec<_>>();
    let mut word_index = 0_usize;
    let mut chars_in_word_seen = 0_usize;

    let mut delays = Vec::with_capacity(text.chars().count());
    for ch in text.chars() {
        let current_word_len = words.get(word_index).copied().unwrap_or(4);
        let base_min = if current_word_len > 8 {
            profile.min_typing_delay_ms.saturating_add(15)
        } else {
            profile.min_typing_delay_ms
        };
        let base_max = if current_word_len <= 4 {
            profile.max_typing_delay_ms.saturating_sub(10)
        } else {
            profile.max_typing_delay_ms
        };
        let min = base_min.min(profile.max_typing_delay_ms);
        let max = base_max.max(min);

        let mut delay = rng.gen_range(min..=max);
        if ch.is_whitespace() {
            delay = delay.saturating_add(20).min(profile.max_typing_delay_ms);
        }
        delays.push(delay.max(profile.min_typing_delay_ms));

        if ch.is_whitespace() {
            word_index = word_index.saturating_add(1);
            chars_in_word_seen = 0;
        } else {
            chars_in_word_seen += 1;
            if chars_in_word_seen >= current_word_len {
                chars_in_word_seen = 0;
            }
        }
    }

    delays
}

fn gaussian_sample<R: Rng + ?Sized>(mean: f64, std_dev: f64, rng: &mut R) -> f64 {
    // Box-Muller transform using two independent uniforms in (0, 1].
    let u1 = (1.0 - rng.gen::<f64>()).max(f64::MIN_POSITIVE);
    let u2 = 1.0 - rng.gen::<f64>();
    let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    mean + std_dev * z0
}
