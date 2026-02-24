//! SimSitting — Airwave Synthesizer
//!
//! Procedural audio using Web Audio API via `web-sys`. Musical modes
//! are selected based on real-time societal data:
//!
//! - **Dorian** (The Parking Lot): Minor but hopeful — default mode
//! - **Lydian** (The Shopping Mall): Major but "off-kilter" and ethereal
//! - **Phrygian** (The Mike Tyson Punch): Dark, tense, and urgent
//!
//! The synthesizer bridges Bevy's ECS with the browser's `AudioContext`,
//! scheduling beats on a loop that adapts to the simulation's emotional state.

use bevy::prelude::*;

/// Audio state resource — platform-agnostic wrapper.
/// On WASM, this will hold the Web Audio API context.
/// On native, it's a no-op for now.
#[derive(Resource, Default)]
pub struct AirwaveState {
    pub mode: MusicalMode,
    pub volume: f32,
    pub filter_freq: f32,
    pub is_glitching: bool,
    pub beat_timer: f32,
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MusicalMode {
    #[default]
    Lydian,   // High profit, ethereal corporate
    Dorian,   // Neutral, melancholic parking lot
    Phrygian, // Mike Tyson knockout, dark and urgent
}

impl MusicalMode {
    /// Scale degrees as semitone offsets from root
    pub fn scale(&self) -> &[i32] {
        match self {
            MusicalMode::Dorian   => &[0, 2, 3, 5, 7, 9, 10],
            MusicalMode::Lydian   => &[0, 2, 4, 6, 7, 9, 11],
            MusicalMode::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
        }
    }

    /// Convert a scale degree + octave to frequency (A4 = 440Hz)
    pub fn freq(&self, degree: usize, octave: i32) -> f32 {
        let scale = self.scale();
        let semitone = scale[degree % scale.len()] as f32 + (octave as f32 * 12.0);
        // D3 as root = MIDI 50 = 146.83 Hz
        146.83 * 2.0f32.powf(semitone / 12.0)
    }
}

/// Determine the appropriate musical mode from simulation stats
pub fn choose_mode(polarization: f32, revenue_growth: f32) -> MusicalMode {
    if polarization > 0.85 {
        MusicalMode::Phrygian
    } else if revenue_growth > 0.1 {
        MusicalMode::Lydian
    } else {
        MusicalMode::Dorian
    }
}

/// Determine the low-pass filter frequency based on game state.
///
/// Diegetic audio cues:
/// - `engagement_gated`: When engagement < 0.3, clamp to 400Hz (muffled apathy)
/// - `snr_ratio`: Scale filter down as media saturation rises (1.0 = clear, 0.1 = static)
pub fn choose_filter_freq(
    mode: &MusicalMode,
    polarization: f32,
    engagement_gated: bool,
    snr_ratio: f32,
) -> f32 {
    // Engagement gate: the game literally sounds "muffled"
    if engagement_gated {
        return 400.0;
    }

    let base = match mode {
        MusicalMode::Lydian   => 1200.0 + polarization * 1800.0, // Opens up with profit
        MusicalMode::Dorian   => 1200.0,                          // Muffled ennui
        MusicalMode::Phrygian => 200.0 + (1.0 - polarization) * 400.0, // Bass thud
    };

    // SNR degradation: as noise rises, the filter closes
    // At SNR 1.0: no change. At SNR 0.1: filter drops to 30% of base.
    let snr_factor = 0.3 + 0.7 * snr_ratio;
    base * snr_factor
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AirwaveState>();
        app.add_systems(Update, update_airwave_state);
    }
}

fn update_airwave_state(
    stats: Res<crate::economy::GlobalStats>,
    mut airwave: ResMut<AirwaveState>,
    time: Res<Time>,
    telemetry: Res<crate::telemetry::UiTelemetry>,
) {
    let growth = if stats.prev_quarterly_revenue > 0.0 {
        ((stats.quarterly_revenue - stats.prev_quarterly_revenue) / stats.prev_quarterly_revenue) as f32
    } else {
        0.0
    };

    airwave.mode = choose_mode(stats.polarization_heat, growth);
    airwave.filter_freq = choose_filter_freq(
        &airwave.mode,
        stats.polarization_heat,
        telemetry.engagement_gated,
        telemetry.snr_ratio,
    );
    airwave.is_glitching = stats.polarization_heat > 0.85;
    airwave.volume = 0.1;
    airwave.beat_timer += time.delta_secs();
}

/// Beat interval in seconds. Synced to a slow "corporate clock" ~72 BPM.
pub const BEAT_INTERVAL: f32 = 0.833; // 60/72 ≈ 0.833s

/// Generate the chord voicing (3 frequencies) for the current beat.
/// Returns (root, third, fifth) frequencies for a triad.
pub fn generate_chord(mode: &MusicalMode, beat_index: u32) -> (f32, f32, f32) {
    let degree = (beat_index % 4) as usize; // Cycle through I, ii, iii, IV
    let chord_degrees = match degree {
        0 => (0, 2, 4), // I chord
        1 => (1, 3, 5), // ii chord
        2 => (2, 4, 6), // iii chord
        _ => (3, 5, 0), // IV chord (wraps)
    };

    let octave = 0; // D3 register
    (
        mode.freq(chord_degrees.0, octave),
        mode.freq(chord_degrees.1, octave),
        mode.freq(chord_degrees.2, octave),
    )
}

/// Check if a beat should fire this frame (edge detection on beat_timer)
pub fn should_fire_beat(timer: f32, dt: f32) -> bool {
    if dt <= 0.0 { return false; }
    let prev = timer - dt;
    let beat_prev = (prev / BEAT_INTERVAL).floor();
    let beat_now = (timer / BEAT_INTERVAL).floor();
    beat_now > beat_prev
}

/// Get the current beat index from the timer
pub fn beat_index(timer: f32) -> u32 {
    (timer / BEAT_INTERVAL).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Musical Mode Tests ===

    #[test]
    fn test_dorian_scale() {
        let mode = MusicalMode::Dorian;
        assert_eq!(mode.scale(), &[0, 2, 3, 5, 7, 9, 10]);
    }

    #[test]
    fn test_lydian_scale() {
        let mode = MusicalMode::Lydian;
        assert_eq!(mode.scale(), &[0, 2, 4, 6, 7, 9, 11]);
    }

    #[test]
    fn test_phrygian_scale() {
        let mode = MusicalMode::Phrygian;
        assert_eq!(mode.scale(), &[0, 1, 3, 5, 7, 8, 10]);
    }

    #[test]
    fn test_freq_root_note() {
        let mode = MusicalMode::Dorian;
        let root = mode.freq(0, 0);
        // D3 = 146.83 Hz
        assert!((root - 146.83).abs() < 0.1);
    }

    #[test]
    fn test_freq_octave_up() {
        let mode = MusicalMode::Dorian;
        let root = mode.freq(0, 0);
        let octave_up = mode.freq(0, 1);
        assert!((octave_up / root - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_freq_fifth() {
        let mode = MusicalMode::Dorian;
        let root = mode.freq(0, 0);
        let fifth = mode.freq(4, 0);
        let ratio = fifth / root;
        assert!((ratio - 1.498).abs() < 0.01);
    }

    // === Mode Selection Tests ===

    #[test]
    fn test_choose_mode_phrygian_high_polarization() {
        assert_eq!(choose_mode(0.9, 0.5), MusicalMode::Phrygian);
    }

    #[test]
    fn test_choose_mode_lydian_high_growth() {
        assert_eq!(choose_mode(0.3, 0.2), MusicalMode::Lydian);
    }

    #[test]
    fn test_choose_mode_dorian_neutral() {
        assert_eq!(choose_mode(0.3, 0.05), MusicalMode::Dorian);
    }

    #[test]
    fn test_phrygian_overrides_lydian() {
        assert_eq!(choose_mode(0.9, 0.5), MusicalMode::Phrygian);
    }

    // === Filter Frequency Tests ===

    #[test]
    fn test_filter_lydian_opens_with_polarization() {
        let low = choose_filter_freq(&MusicalMode::Lydian, 0.0, false, 1.0);
        let high = choose_filter_freq(&MusicalMode::Lydian, 1.0, false, 1.0);
        assert!(high > low);
        assert!((low - 1200.0).abs() < 1.0);
        assert!((high - 3000.0).abs() < 1.0);
    }

    #[test]
    fn test_filter_dorian_is_muffled() {
        let freq = choose_filter_freq(&MusicalMode::Dorian, 0.5, false, 1.0);
        assert!((freq - 1200.0).abs() < 1.0);
    }

    #[test]
    fn test_filter_phrygian_is_bass_thud() {
        let freq = choose_filter_freq(&MusicalMode::Phrygian, 1.0, false, 1.0);
        assert!(freq < 300.0);
    }

    // === Diegetic Audio Tests ===

    #[test]
    fn test_engagement_gate_clamps_to_400hz() {
        // When engagement is gated, filter clamps to 400Hz regardless of mode
        let freq_lydian = choose_filter_freq(&MusicalMode::Lydian, 0.5, true, 1.0);
        let freq_dorian = choose_filter_freq(&MusicalMode::Dorian, 0.5, true, 1.0);
        let freq_phrygian = choose_filter_freq(&MusicalMode::Phrygian, 0.5, true, 1.0);
        assert!((freq_lydian - 400.0).abs() < 0.1, "Gated Lydian should be 400Hz (got {})", freq_lydian);
        assert!((freq_dorian - 400.0).abs() < 0.1, "Gated Dorian should be 400Hz");
        assert!((freq_phrygian - 400.0).abs() < 0.1, "Gated Phrygian should be 400Hz");
    }

    #[test]
    fn test_snr_degradation_reduces_filter() {
        // Low SNR = filter closes. Compare full SNR vs degraded.
        let full = choose_filter_freq(&MusicalMode::Lydian, 0.5, false, 1.0);
        let degraded = choose_filter_freq(&MusicalMode::Lydian, 0.5, false, 0.3);
        assert!(degraded < full,
            "Degraded SNR should lower filter (full={}, degraded={})", full, degraded);
        // At SNR 0.1: factor = 0.3 + 0.7*0.1 = 0.37
        let very_low = choose_filter_freq(&MusicalMode::Lydian, 0.5, false, 0.1);
        assert!(very_low < degraded, "Very low SNR should further reduce filter");
    }

    #[test]
    fn test_snr_pristine_no_change() {
        // At SNR 1.0: factor = 0.3 + 0.7*1.0 = 1.0, so base unchanged
        let base = choose_filter_freq(&MusicalMode::Dorian, 0.0, false, 1.0);
        assert!((base - 1200.0).abs() < 1.0, "Pristine SNR should leave filter unchanged");
    }

    #[test]
    fn test_engagement_gate_overrides_snr() {
        // Gate takes priority over SNR
        let freq = choose_filter_freq(&MusicalMode::Lydian, 1.0, true, 0.1);
        assert!((freq - 400.0).abs() < 0.1, "Gate should override SNR (got {})", freq);
    }

    // === Beat Scheduling Tests ===

    #[test]
    fn test_beat_fires_on_crossing() {
        // At t=0.83s with dt=0.016s, we should cross the 0.833 boundary
        assert!(!should_fire_beat(0.5, 0.016));
        assert!(should_fire_beat(0.84, 0.016)); // Crosses 0.833
    }

    #[test]
    fn test_beat_does_not_fire_within_interval() {
        assert!(!should_fire_beat(0.4, 0.016));
        assert!(!should_fire_beat(0.8, 0.016));
    }

    #[test]
    fn test_beat_index_increments() {
        assert_eq!(beat_index(0.0), 0);
        assert_eq!(beat_index(0.9), 1);
        assert_eq!(beat_index(1.7), 2);
        assert_eq!(beat_index(2.5), 3);
    }

    // === Chord Generation Tests ===

    #[test]
    fn test_chord_returns_three_frequencies() {
        let (root, third, fifth) = generate_chord(&MusicalMode::Dorian, 0);
        assert!(root > 0.0);
        assert!(third > 0.0);
        assert!(fifth > 0.0);
        // Root should be D3 ≈ 146.83
        assert!((root - 146.83).abs() < 0.1);
    }

    #[test]
    fn test_chord_cycles_through_degrees() {
        let c0 = generate_chord(&MusicalMode::Dorian, 0);
        let c1 = generate_chord(&MusicalMode::Dorian, 1);
        let c4 = generate_chord(&MusicalMode::Dorian, 4); // Should wrap to beat 0
        // Different beats should give different chords
        assert!((c0.0 - c1.0).abs() > 1.0); // Different root
        // Beat 4 should equal beat 0 (cycle of 4)
        assert!((c0.0 - c4.0).abs() < 0.01);
    }

    #[test]
    fn test_phrygian_chord_is_darker() {
        let dorian = generate_chord(&MusicalMode::Dorian, 0);
        let phrygian = generate_chord(&MusicalMode::Phrygian, 1);
        // Phrygian degree 1 = semitone 1 (minor 2nd), should be lower than Dorian degree 1 = semitone 2
        assert!(phrygian.0 < dorian.1);
    }
}

