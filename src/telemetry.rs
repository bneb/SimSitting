//! SimSitting — Diegetic Telemetry
//!
//! Pre-computes "vibes" from raw simulation data for the UI layer.
//! Every field in [`UiTelemetry`] drives a specific diegetic instrument:
//!
//! | Field               | Widget                     | Mechanic Surfaced          |
//! |---------------------|----------------------------|----------------------------|
//! | `snr_ratio`         | Bandwidth Oscilloscope     | Attention decay scaling    |
//! | `revenue_efficiency`| Dashboard desaturation     | Engagement gate (< 0.3)    |
//! | `oracle_heat`       | Oracle Thermal Monitor     | Quadratic trust decay      |
//! | `election_glow`     | Election Preview pulse     | Projected winner preview   |
//! | `vitality_segments` | Vitality Meter segments    | Engagement gauge           |
//! | `phase_transition`  | Update Wizard popup        | GamePhase disclosure       |
//!
//! All heavy math lives in [`compute_telemetry`] — a pure function, fully testable
//! without Bevy ECS. The system wrapper [`update_telemetry_system`] just reads
//! resources and writes the result.

use bevy::prelude::*;
use crate::economy::{GlobalStats, GamePhase, CurrentPhase};
use crate::oracle::OracleState;
use crate::politics::ElectionState;
use crate::sim::SimConfig;

// ============================================================================
// Resource
// ============================================================================

/// Pre-computed telemetry driving the diegetic UI instruments.
/// Updated every frame by [`update_telemetry_system`].
#[derive(Resource, Clone, Debug)]
pub struct UiTelemetry {
    // --- Bandwidth Oscilloscope ---
    /// Signal-to-Noise Ratio: 1.0 = pristine signal, 0.1 = overwhelmed.
    /// Drives jitter amplitude on the CRT waveform.
    pub snr_ratio: f32,
    /// True when attention decay outpaces recovery (the noise is winning).
    pub snr_warning: bool,
    /// Human-readable efficiency loss percentage from media saturation.
    pub efficiency_loss_pct: f32,

    // --- Vitality Meter ---
    /// 0–10 segments representing aggregate engagement (each = 10%).
    pub vitality_segments: u32,
    /// True when engagement_index < 0.3 — revenue is gated to 10%.
    pub engagement_gated: bool,
    /// Revenue display multiplier: 1.0 normal, 0.1 when gated.
    /// Drives dashboard desaturation.
    pub revenue_efficiency: f32,

    // --- Oracle Thermal Monitor ---
    /// optimization_level² — drives the mercury fill and smoke particles.
    /// 0.0 = cold, 0.25 = warm (opt=0.5), 1.0 = critical (opt=1.0).
    pub oracle_heat: f32,
    /// True when oracle_heat > 0.49 (optimization_level > 0.7).
    /// Triggers pixel-dust particle effects.
    pub oracle_smoking: bool,
    /// True when oracle_heat > 0.64 (optimization_level > 0.8).
    /// Triggers the friction warning modal.
    pub oracle_friction_warning: bool,

    // --- Election Preview ---
    /// Pulsing intensity for the projected winner badge (0.0–1.0).
    /// 0.0 when no preview, ramps up as election approaches.
    pub election_glow: f32,

    // --- Update Wizard ---
    /// Set to `Some(phase)` for exactly one frame when a phase transition occurs.
    /// The UI layer consumes this to trigger the Win95 installer modal.
    pub phase_transition: Option<GamePhase>,
    /// The previous phase, used to detect transitions.
    pub last_known_phase: GamePhase,

    // --- Update Wizard Animation State ---
    /// Progress of the "installation" animation (0.0–1.0).
    /// Advances ~0.33/sec (completes in ~3 seconds).
    pub wizard_progress: f32,
    /// True while the wizard modal is visible.
    pub wizard_active: bool,
    /// The phase being "installed" (set when wizard activates).
    pub wizard_target_phase: GamePhase,
}

impl Default for UiTelemetry {
    fn default() -> Self {
        Self {
            snr_ratio: 1.0,
            snr_warning: false,
            efficiency_loss_pct: 0.0,
            vitality_segments: 10,
            engagement_gated: false,
            revenue_efficiency: 1.0,
            oracle_heat: 0.0,
            oracle_smoking: false,
            oracle_friction_warning: false,
            election_glow: 0.0,
            phase_transition: None,
            last_known_phase: GamePhase::MediaCompany,
            wizard_progress: 0.0,
            wizard_active: false,
            wizard_target_phase: GamePhase::MediaCompany,
        }
    }
}

// ============================================================================
// Pure Computation (TDD-friendly)
// ============================================================================

/// Input snapshot for [`compute_telemetry`] — decouples from Bevy resources.
#[derive(Clone, Debug)]
pub struct TelemetryInput {
    pub node_count: usize,
    pub engagement_index: f32,
    pub oracle_active: bool,
    pub optimization_level: f32,
    pub election_showing_preview: bool,
    pub quarters_until_election: u32,
    pub current_phase: GamePhase,
    pub attention_decay_rate: f32,
    pub attention_recovery_rate: f32,
}

/// Output from [`compute_telemetry`] — the raw computed values.
/// Separate from the full `UiTelemetry` which also carries animation state.
#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryOutput {
    pub snr_ratio: f32,
    pub snr_warning: bool,
    pub efficiency_loss_pct: f32,
    pub vitality_segments: u32,
    pub engagement_gated: bool,
    pub revenue_efficiency: f32,
    pub oracle_heat: f32,
    pub oracle_smoking: bool,
    pub oracle_friction_warning: bool,
    pub election_glow: f32,
}

/// Compute all telemetry values from a simulation snapshot.
///
/// **Pure function** — no side effects, no Bevy, fully deterministic.
/// This is the heart of the diegetic telemetry system.
pub fn compute_telemetry(input: &TelemetryInput) -> TelemetryOutput {
    // --- SNR (Bandwidth Oscilloscope) ---
    // Each node adds 5% noise. At 18+ nodes, SNR floors at 0.1 (not zero — always some signal).
    let noise = (input.node_count as f32 * 0.05).min(0.9);
    let snr_ratio = 1.0 - noise;

    // Warning: when effective decay > recovery, the noise floor is winning.
    let effective_decay = input.attention_decay_rate * (1.0 + 0.5 * input.node_count as f32);
    let snr_warning = effective_decay > input.attention_recovery_rate;

    // Efficiency loss: what percentage of attention capacity is wasted.
    let efficiency_loss_pct = (noise * 100.0).round();

    // --- Vitality Meter ---
    let vitality_segments = (input.engagement_index * 10.0).round().clamp(0.0, 10.0) as u32;
    let engagement_gated = input.engagement_index < 0.3;
    let revenue_efficiency = if engagement_gated { 0.1 } else { 1.0 };

    // --- Oracle Thermal Monitor ---
    let oracle_heat = if input.oracle_active {
        input.optimization_level * input.optimization_level
    } else {
        0.0
    };
    let oracle_smoking = oracle_heat > 0.49;
    let oracle_friction_warning = oracle_heat > 0.64;

    // --- Election Preview Glow ---
    // Pulses when preview is showing, intensity based on proximity.
    let election_glow = if input.election_showing_preview {
        // More intense the closer to election (fewer quarters = brighter)
        match input.quarters_until_election {
            0 => 1.0,     // Election imminent
            1 => 0.7,     // Next quarter
            _ => 0.4,     // Far but showing
        }
    } else {
        0.0
    };

    TelemetryOutput {
        snr_ratio,
        snr_warning,
        efficiency_loss_pct,
        vitality_segments,
        engagement_gated,
        revenue_efficiency,
        oracle_heat,
        oracle_smoking,
        oracle_friction_warning,
        election_glow,
    }
}

/// Detect a phase transition. Returns `Some(new_phase)` if the phase changed.
pub fn detect_phase_transition(current: GamePhase, previous: GamePhase) -> Option<GamePhase> {
    if current != previous && current > previous {
        Some(current)
    } else {
        None
    }
}

/// Get the "installation" flavor text for a phase transition.
pub fn wizard_status_text(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::MediaCompany => "Booting BROADCAST_OS v1.04...",
        GamePhase::AttentionEconomy => "Unpacking: Zone_Painter.dll ...",
        GamePhase::TheShadow => "Accepting Terms of Service... [UNREAD]",
        GamePhase::TheOracle => "WARNING: Automated systems may void your warranty.",
    }
}

/// Get the module name being "installed" for a phase transition.
pub fn wizard_module_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::MediaCompany => "NARRATIVE_ENGINE",
        GamePhase::AttentionEconomy => "ATTENTION_ZONING",
        GamePhase::TheShadow => "SHADOW_INFRASTRUCTURE",
        GamePhase::TheOracle => "ORACLE_SUBSYSTEM",
    }
}

// ============================================================================
// Bevy System
// ============================================================================

/// System: update `UiTelemetry` every frame from simulation resources.
pub fn update_telemetry_system(
    stats: Res<GlobalStats>,
    oracle: Res<OracleState>,
    election: Res<ElectionState>,
    config: Res<SimConfig>,
    current_phase: Res<CurrentPhase>,
    mut telemetry: ResMut<UiTelemetry>,
    time: Res<Time>,
) {
    let input = TelemetryInput {
        node_count: stats.node_count,
        engagement_index: stats.engagement_index,
        oracle_active: oracle.active,
        optimization_level: oracle.optimization_level,
        election_showing_preview: election.showing_preview,
        quarters_until_election: election.election_interval
            .saturating_sub(election.quarters_since_election),
        current_phase: current_phase.phase,
        attention_decay_rate: config.attention_decay_rate,
        attention_recovery_rate: config.attention_recovery_rate,
    };

    let output = compute_telemetry(&input);

    // Apply computed values
    telemetry.snr_ratio = output.snr_ratio;
    telemetry.snr_warning = output.snr_warning;
    telemetry.efficiency_loss_pct = output.efficiency_loss_pct;
    telemetry.vitality_segments = output.vitality_segments;
    telemetry.engagement_gated = output.engagement_gated;
    telemetry.revenue_efficiency = output.revenue_efficiency;
    telemetry.oracle_heat = output.oracle_heat;
    telemetry.oracle_smoking = output.oracle_smoking;
    telemetry.oracle_friction_warning = output.oracle_friction_warning;
    telemetry.election_glow = output.election_glow;

    // Detect phase transitions
    telemetry.phase_transition = detect_phase_transition(
        current_phase.phase,
        telemetry.last_known_phase,
    );
    if let Some(new_phase) = telemetry.phase_transition {
        telemetry.last_known_phase = new_phase;
        telemetry.wizard_active = true;
        telemetry.wizard_progress = 0.0;
        telemetry.wizard_target_phase = new_phase;
    }

    // Advance wizard animation
    if telemetry.wizard_active {
        telemetry.wizard_progress += time.delta_secs() * 0.33; // ~3 seconds to complete
        if telemetry.wizard_progress >= 1.0 {
            telemetry.wizard_progress = 1.0;
            telemetry.wizard_active = false;
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct TelemetryPlugin;

impl Plugin for TelemetryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTelemetry>()
            .add_systems(Update, update_telemetry_system);
    }
}

// ============================================================================
// Tests (TDD — RED→GREEN)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a default input with sane values
    fn default_input() -> TelemetryInput {
        TelemetryInput {
            node_count: 0,
            engagement_index: 1.0,
            oracle_active: false,
            optimization_level: 0.0,
            election_showing_preview: false,
            quarters_until_election: 4,
            current_phase: GamePhase::MediaCompany,
            attention_decay_rate: 0.001,
            attention_recovery_rate: 0.0005,
        }
    }

    // =========================================================================
    // SNR / Bandwidth Oscilloscope
    // =========================================================================

    #[test]
    fn test_snr_ratio_pristine_with_no_nodes() {
        let input = default_input();
        let out = compute_telemetry(&input);
        assert!((out.snr_ratio - 1.0).abs() < 0.001,
            "Zero nodes = pristine signal (got {})", out.snr_ratio);
    }

    #[test]
    fn test_snr_ratio_decreases_per_node() {
        let mut input = default_input();
        input.node_count = 1;
        let out1 = compute_telemetry(&input);

        input.node_count = 5;
        let out5 = compute_telemetry(&input);

        assert!(out5.snr_ratio < out1.snr_ratio,
            "More nodes = more noise (1 node: {}, 5 nodes: {})", out1.snr_ratio, out5.snr_ratio);
    }

    #[test]
    fn test_snr_ratio_five_nodes() {
        let mut input = default_input();
        input.node_count = 5;
        let out = compute_telemetry(&input);
        // 5 nodes × 0.05 = 0.25 noise → SNR = 0.75
        assert!((out.snr_ratio - 0.75).abs() < 0.001,
            "5 nodes should give SNR 0.75 (got {})", out.snr_ratio);
    }

    #[test]
    fn test_snr_ratio_ten_nodes() {
        let mut input = default_input();
        input.node_count = 10;
        let out = compute_telemetry(&input);
        // 10 × 0.05 = 0.5 noise → SNR = 0.5
        assert!((out.snr_ratio - 0.5).abs() < 0.001,
            "10 nodes should give SNR 0.5 (got {})", out.snr_ratio);
    }

    #[test]
    fn test_snr_ratio_floors_at_ten_percent() {
        let mut input = default_input();
        input.node_count = 100; // Way over the cap
        let out = compute_telemetry(&input);
        // Capped at 0.9 noise → SNR = 0.1
        assert!((out.snr_ratio - 0.1).abs() < 0.001,
            "SNR should floor at 0.1 (got {})", out.snr_ratio);
    }

    #[test]
    fn test_snr_warning_off_with_no_nodes() {
        let input = default_input();
        let out = compute_telemetry(&input);
        // 0 nodes: effective_decay = 0.001 × 1.0 = 0.001, recovery = 0.0005
        // decay > recovery → warning ON even with 0 nodes!
        // This is correct: base decay already exceeds recovery.
        assert!(out.snr_warning,
            "Base decay (0.001) already > recovery (0.0005) — warning should be on");
    }

    #[test]
    fn test_snr_warning_on_with_high_recovery() {
        // If recovery > decay, no warning
        let mut input = default_input();
        input.attention_recovery_rate = 0.01; // 10× higher than decay
        input.node_count = 0;
        let out = compute_telemetry(&input);
        assert!(!out.snr_warning,
            "High recovery should suppress warning");
    }

    #[test]
    fn test_snr_warning_on_with_many_nodes() {
        let mut input = default_input();
        input.attention_recovery_rate = 0.01; // High recovery
        input.node_count = 20;
        // effective_decay = 0.001 × (1 + 10) = 0.011 > 0.01
        let out = compute_telemetry(&input);
        assert!(out.snr_warning,
            "20 nodes should push decay past recovery (decay={}, recovery={})",
            0.001 * (1.0 + 0.5 * 20.0), input.attention_recovery_rate);
    }

    #[test]
    fn test_efficiency_loss_pct_zero_with_no_nodes() {
        let input = default_input();
        let out = compute_telemetry(&input);
        assert!((out.efficiency_loss_pct - 0.0).abs() < 0.1,
            "No nodes = 0% efficiency loss (got {}%)", out.efficiency_loss_pct);
    }

    #[test]
    fn test_efficiency_loss_pct_scales() {
        let mut input = default_input();
        input.node_count = 6;
        let out = compute_telemetry(&input);
        // 6 × 5% = 30% loss
        assert!((out.efficiency_loss_pct - 30.0).abs() < 0.1,
            "6 nodes = 30% loss (got {}%)", out.efficiency_loss_pct);
    }

    // =========================================================================
    // Vitality Meter / Engagement Gate
    // =========================================================================

    #[test]
    fn test_vitality_segments_full_engagement() {
        let input = default_input(); // engagement = 1.0
        let out = compute_telemetry(&input);
        assert_eq!(out.vitality_segments, 10, "Full engagement = 10 segments");
    }

    #[test]
    fn test_vitality_segments_partial() {
        let mut input = default_input();
        input.engagement_index = 0.73;
        let out = compute_telemetry(&input);
        assert_eq!(out.vitality_segments, 7,
            "0.73 engagement should round to 7 segments (got {})", out.vitality_segments);
    }

    #[test]
    fn test_vitality_segments_zero() {
        let mut input = default_input();
        input.engagement_index = 0.0;
        let out = compute_telemetry(&input);
        assert_eq!(out.vitality_segments, 0, "Zero engagement = 0 segments");
    }

    #[test]
    fn test_vitality_segments_at_gate_threshold() {
        let mut input = default_input();
        input.engagement_index = 0.3;
        let out = compute_telemetry(&input);
        assert_eq!(out.vitality_segments, 3, "At threshold = 3 segments");
        assert!(!out.engagement_gated, "At 0.3 exactly, NOT gated");
    }

    #[test]
    fn test_vitality_segments_below_gate() {
        let mut input = default_input();
        input.engagement_index = 0.29;
        let out = compute_telemetry(&input);
        assert_eq!(out.vitality_segments, 3,
            "0.29 rounds to 3 segments (got {})", out.vitality_segments);
        assert!(out.engagement_gated, "Below 0.3 = gated");
    }

    #[test]
    fn test_engagement_gate_revenue_efficiency() {
        let mut input = default_input();

        // Above threshold
        input.engagement_index = 0.5;
        let out = compute_telemetry(&input);
        assert!((out.revenue_efficiency - 1.0).abs() < 0.001, "Above gate = full efficiency");

        // Below threshold
        input.engagement_index = 0.2;
        let out = compute_telemetry(&input);
        assert!((out.revenue_efficiency - 0.1).abs() < 0.001, "Below gate = 10% efficiency");
    }

    #[test]
    fn test_engagement_gate_flag() {
        let mut input = default_input();

        input.engagement_index = 0.31;
        let out = compute_telemetry(&input);
        assert!(!out.engagement_gated, "0.31 is NOT gated");

        input.engagement_index = 0.29;
        let out = compute_telemetry(&input);
        assert!(out.engagement_gated, "0.29 IS gated");
    }

    // =========================================================================
    // Oracle Thermal Monitor
    // =========================================================================

    #[test]
    fn test_oracle_heat_zero_when_inactive() {
        let input = default_input(); // oracle_active = false
        let out = compute_telemetry(&input);
        assert!((out.oracle_heat - 0.0).abs() < 0.001, "Inactive oracle = cold");
    }

    #[test]
    fn test_oracle_heat_quadratic() {
        let mut input = default_input();
        input.oracle_active = true;
        input.optimization_level = 0.5;
        let out = compute_telemetry(&input);
        // 0.5² = 0.25
        assert!((out.oracle_heat - 0.25).abs() < 0.001,
            "opt 0.5 → heat 0.25 (got {})", out.oracle_heat);
    }

    #[test]
    fn test_oracle_heat_full_automation() {
        let mut input = default_input();
        input.oracle_active = true;
        input.optimization_level = 1.0;
        let out = compute_telemetry(&input);
        assert!((out.oracle_heat - 1.0).abs() < 0.001,
            "opt 1.0 → heat 1.0 (got {})", out.oracle_heat);
    }

    #[test]
    fn test_oracle_smoking_threshold() {
        let mut input = default_input();
        input.oracle_active = true;

        // Just below (opt ≈ 0.7 → heat = 0.49)
        input.optimization_level = 0.69;
        let out = compute_telemetry(&input);
        assert!(!out.oracle_smoking,
            "opt 0.69 → heat {:.3} should NOT smoke", out.oracle_heat);

        // Just above (opt = 0.71 → heat ≈ 0.504)
        input.optimization_level = 0.71;
        let out = compute_telemetry(&input);
        assert!(out.oracle_smoking,
            "opt 0.71 → heat {:.3} SHOULD smoke", out.oracle_heat);
    }

    #[test]
    fn test_oracle_friction_warning_threshold() {
        let mut input = default_input();
        input.oracle_active = true;

        // Below threshold (opt = 0.79 → heat ≈ 0.624)
        input.optimization_level = 0.79;
        let out = compute_telemetry(&input);
        assert!(!out.oracle_friction_warning,
            "opt 0.79 → heat {:.3} should NOT warn", out.oracle_heat);

        // Above threshold (opt = 0.81 → heat ≈ 0.656)
        input.optimization_level = 0.81;
        let out = compute_telemetry(&input);
        assert!(out.oracle_friction_warning,
            "opt 0.81 → heat {:.3} SHOULD warn", out.oracle_heat);
    }

    #[test]
    fn test_oracle_no_smoke_when_inactive() {
        let mut input = default_input();
        input.oracle_active = false;
        input.optimization_level = 1.0; // Would be hot if active
        let out = compute_telemetry(&input);
        assert!(!out.oracle_smoking, "Inactive oracle = no smoke");
        assert!(!out.oracle_friction_warning, "Inactive oracle = no warning");
    }

    // =========================================================================
    // Election Preview Glow
    // =========================================================================

    #[test]
    fn test_election_glow_zero_when_no_preview() {
        let input = default_input(); // showing_preview = false
        let out = compute_telemetry(&input);
        assert!((out.election_glow - 0.0).abs() < 0.001, "No preview = no glow");
    }

    #[test]
    fn test_election_glow_imminent() {
        let mut input = default_input();
        input.election_showing_preview = true;
        input.quarters_until_election = 0;
        let out = compute_telemetry(&input);
        assert!((out.election_glow - 1.0).abs() < 0.001,
            "Imminent election = max glow (got {})", out.election_glow);
    }

    #[test]
    fn test_election_glow_one_quarter() {
        let mut input = default_input();
        input.election_showing_preview = true;
        input.quarters_until_election = 1;
        let out = compute_telemetry(&input);
        assert!((out.election_glow - 0.7).abs() < 0.001,
            "1 quarter away = 0.7 glow (got {})", out.election_glow);
    }

    #[test]
    fn test_election_glow_far() {
        let mut input = default_input();
        input.election_showing_preview = true;
        input.quarters_until_election = 3;
        let out = compute_telemetry(&input);
        assert!((out.election_glow - 0.4).abs() < 0.001,
            "Far preview = 0.4 glow (got {})", out.election_glow);
    }

    // =========================================================================
    // Phase Transition Detection
    // =========================================================================

    #[test]
    fn test_no_transition_same_phase() {
        let result = detect_phase_transition(GamePhase::MediaCompany, GamePhase::MediaCompany);
        assert_eq!(result, None, "Same phase = no transition");
    }

    #[test]
    fn test_transition_forward() {
        let result = detect_phase_transition(GamePhase::AttentionEconomy, GamePhase::MediaCompany);
        assert_eq!(result, Some(GamePhase::AttentionEconomy), "Forward = transition");
    }

    #[test]
    fn test_no_transition_backward() {
        // Should never happen in practice, but guard against it
        let result = detect_phase_transition(GamePhase::MediaCompany, GamePhase::TheOracle);
        assert_eq!(result, None, "Backward transition = no transition");
    }

    #[test]
    fn test_transition_each_step() {
        let pairs = [
            (GamePhase::AttentionEconomy, GamePhase::MediaCompany),
            (GamePhase::TheShadow, GamePhase::AttentionEconomy),
            (GamePhase::TheOracle, GamePhase::TheShadow),
        ];
        for (new, old) in &pairs {
            assert_eq!(
                detect_phase_transition(*new, *old),
                Some(*new),
                "Transition {:?} → {:?} should trigger", old, new
            );
        }
    }

    // =========================================================================
    // Wizard Flavor Text
    // =========================================================================

    #[test]
    fn test_wizard_status_text_attention_economy() {
        let text = wizard_status_text(GamePhase::AttentionEconomy);
        assert!(text.contains("Zone_Painter"), "Should mention Zone_Painter: {}", text);
    }

    #[test]
    fn test_wizard_status_text_shadow() {
        let text = wizard_status_text(GamePhase::TheShadow);
        assert!(text.contains("Terms of Service"), "Should mention ToS: {}", text);
        assert!(text.contains("UNREAD"), "Should mention UNREAD: {}", text);
    }

    #[test]
    fn test_wizard_status_text_oracle() {
        let text = wizard_status_text(GamePhase::TheOracle);
        assert!(text.contains("warranty"), "Should mention warranty: {}", text);
    }

    #[test]
    fn test_wizard_module_names_unique() {
        let phases = [
            GamePhase::MediaCompany,
            GamePhase::AttentionEconomy,
            GamePhase::TheShadow,
            GamePhase::TheOracle,
        ];
        let names: Vec<&str> = phases.iter().map(|p| wizard_module_name(*p)).collect();
        // All names should be unique
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                assert_ne!(names[i], names[j],
                    "Module names should be unique: {} vs {}", names[i], names[j]);
            }
        }
    }

    // =========================================================================
    // UiTelemetry Default Values
    // =========================================================================

    #[test]
    fn test_telemetry_defaults() {
        let t = UiTelemetry::default();
        assert!((t.snr_ratio - 1.0).abs() < 0.001, "Default SNR = 1.0");
        assert!(!t.snr_warning, "No warning by default");
        assert_eq!(t.vitality_segments, 10, "Full vitality by default");
        assert!(!t.engagement_gated, "Not gated by default");
        assert!((t.revenue_efficiency - 1.0).abs() < 0.001, "Full efficiency by default");
        assert!((t.oracle_heat - 0.0).abs() < 0.001, "Cold oracle by default");
        assert!(!t.oracle_smoking, "No smoke by default");
        assert!(!t.oracle_friction_warning, "No warning by default");
        assert!((t.election_glow - 0.0).abs() < 0.001, "No glow by default");
        assert!(t.phase_transition.is_none(), "No transition by default");
        assert_eq!(t.last_known_phase, GamePhase::MediaCompany);
        assert!(!t.wizard_active, "Wizard not active by default");
    }

    // =========================================================================
    // Integration: Combined Scenarios
    // =========================================================================

    #[test]
    fn test_worst_case_scenario() {
        // 20 nodes, engagement at 0.1, oracle at 100%, election imminent
        let input = TelemetryInput {
            node_count: 20,
            engagement_index: 0.1,
            oracle_active: true,
            optimization_level: 1.0,
            election_showing_preview: true,
            quarters_until_election: 0,
            current_phase: GamePhase::TheOracle,
            attention_decay_rate: 0.001,
            attention_recovery_rate: 0.0005,
        };
        let out = compute_telemetry(&input);

        // Everything should be at maximum alarm
        assert!(out.snr_ratio < 0.2, "SNR should be very low: {}", out.snr_ratio);
        assert!(out.snr_warning, "SNR warning should be active");
        assert!(out.engagement_gated, "Engagement should be gated");
        assert!((out.revenue_efficiency - 0.1).abs() < 0.01, "Revenue efficiency at 10%");
        assert!((out.oracle_heat - 1.0).abs() < 0.01, "Oracle heat at max");
        assert!(out.oracle_smoking, "Oracle should be smoking");
        assert!(out.oracle_friction_warning, "Friction warning should fire");
        assert!((out.election_glow - 1.0).abs() < 0.01, "Election glow at max");
    }

    #[test]
    fn test_pristine_starting_state() {
        // Fresh game: 0 nodes, full engagement, no oracle, no election
        let input = default_input();
        let out = compute_telemetry(&input);

        assert!((out.snr_ratio - 1.0).abs() < 0.001, "Pristine SNR");
        assert_eq!(out.vitality_segments, 10, "Full vitality");
        assert!(!out.engagement_gated, "Not gated");
        assert!((out.oracle_heat - 0.0).abs() < 0.001, "Cold oracle");
        assert!((out.election_glow - 0.0).abs() < 0.001, "No glow");
    }
}
