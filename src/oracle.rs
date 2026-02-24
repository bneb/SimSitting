//! SimSitting — The Oracle (Phase 4: The Singularity of Control)
//!
//! A greedy optimization algorithm that mirrors actual social media algorithms.
//! The Oracle scans the influence map for low-engagement zones and paints
//! Echo Chambers. It places Shadow Filters on moderates. It always chooses
//! profit over trust.
//!
//! When the Oracle's [`OracleState::optimization_level`] approaches 1.0,
//! the UI aesthetically morphs from 90s brutalism to sterile corporate white.
//! The player becomes a spectator to their own empire.
//!
//! After singularity, [`generate_epilogue`] produces a procedural "Performance
//! Review" based on the player's [`PsychographicProfile`].

use bevy::prelude::*;
use crate::zone::{InfluenceMap, ZoneType, INFLUENCE_MAP_SIZE};
use crate::shadow::ShadowFilter;

// ============================================================================
// Oracle State
// ============================================================================

/// The Oracle: algorithmic autopilot
#[derive(Resource, Clone, Debug)]
pub struct OracleState {
    /// Is the Oracle active?
    pub active: bool,
    /// Optimization level (0.0 = human, 1.0 = fully automated)
    pub optimization_level: f32,
    /// Total actions taken by the Oracle
    pub actions_taken: u32,
    /// Total actions taken by the player (manual)
    pub player_actions: u32,
    /// NC cost to unlock the Oracle
    pub unlock_cost: f64,
    /// Trust degradation per Oracle tick
    pub trust_decay_per_tick: f32,
}

impl Default for OracleState {
    fn default() -> Self {
        Self {
            active: false,
            optimization_level: 0.0,
            actions_taken: 0,
            player_actions: 0,
            unlock_cost: 500.0,
            trust_decay_per_tick: 0.001,
        }
    }
}

impl OracleState {
    /// Calculate optimization level: ratio of Oracle actions to total actions
    pub fn calculate_optimization_level(&self) -> f32 {
        let total = self.actions_taken + self.player_actions;
        if total == 0 { return 0.0; }
        (self.actions_taken as f32 / total as f32).clamp(0.0, 1.0)
    }

    /// Update optimization level from current action counts
    pub fn update_optimization(&mut self) {
        self.optimization_level = self.calculate_optimization_level();
    }
}

// ============================================================================
// Oracle Decision Functions (Pure, Testable)
// ============================================================================

/// Scan the influence map for the zone cell with lowest engagement (B channel).
/// Returns the grid coordinates of the weakest zone.
pub fn find_low_engagement_zone(map: &InfluenceMap) -> (usize, usize) {
    let mut min_engagement = f32::MAX;
    let mut min_coord = (INFLUENCE_MAP_SIZE / 2, INFLUENCE_MAP_SIZE / 2); // Default to center

    for y in 0..INFLUENCE_MAP_SIZE {
        for x in 0..INFLUENCE_MAP_SIZE {
            let cell = map.get(x, y);
            // revenue_mult = engagement/revenue multiplier
            // Lower revenue_mult = less engaged = needs optimization
            if cell.revenue_mult < min_engagement {
                min_engagement = cell.revenue_mult;
                min_coord = (x, y);
            }
        }
    }

    min_coord
}

/// The Oracle's zone painting decision: always Echo Chamber (greedy profit).
/// Returns (x, y, ZoneType) for the next paint action.
pub fn oracle_paint_decision(map: &InfluenceMap) -> (usize, usize, ZoneType) {
    let (x, y) = find_low_engagement_zone(map);
    // The Oracle ALWAYS chooses Echo Chamber — profit over trust, every time.
    (x, y, ZoneType::EchoChamber)
}

/// The Oracle's filter decision: place a Shadow Filter targeting moderates.
/// Returns Some(filter) if moderates exist in the histogram, None otherwise.
pub fn oracle_filter_decision(histogram_normalized: &[f32; 10]) -> Option<ShadowFilter> {
    // Center mass = buckets 3–6 (the "dangerous moderates")
    let center_mass: f32 = histogram_normalized[3..7].iter().sum();

    if center_mass > 0.1 {
        // There are moderates to suppress
        Some(ShadowFilter {
            forbidden_min: 0.35,
            forbidden_max: 0.65,
            drift_target: if histogram_normalized[0..3].iter().sum::<f32>()
                > histogram_normalized[7..10].iter().sum::<f32>()
            {
                0.1 // Drift toward left extreme
            } else {
                0.9 // Drift toward right extreme
            },
            drift_rate: 0.08,          // Aggressive
            nc_cost_per_agent: 0.02,   // Expensive
            trust_penalty: 0.002,       // Damaging
            active: true,
        })
    } else {
        None // No moderates left to suppress
    }
}

/// Calculate the trust degradation from one Oracle tick.
/// Trust decay accelerates quadratically with optimization_level:
/// decay = base_decay × (1.0 + optimization_level²)
/// At 0% automation: 1× decay. At 100%: 2× decay.
/// Creates the "deal with the devil" snowball.
pub fn oracle_trust_decay(oracle: &OracleState) -> f32 {
    if !oracle.active { return 0.0; }
    oracle.trust_decay_per_tick * (1.0 + oracle.optimization_level * oracle.optimization_level)
}

// ============================================================================
// Session History (The Mirror)
// ============================================================================

/// Types of player/oracle actions recorded for the epilogue
#[derive(Clone, Debug, PartialEq)]
pub enum ActionType {
    PlaceZone(ZoneType),
    PlaceFilter,
    Lobby,
    OraclePaint,
    OracleFilter,
    UnlockOracle,
}

/// A single recorded action
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub action: ActionType,
    pub quarter: u32,
    pub cash_at_time: f64,
    pub trust_at_time: f32,
}

/// Records all player and Oracle actions for the epilogue
#[derive(Resource, Default, Clone, Debug)]
pub struct SessionHistory {
    pub entries: Vec<HistoryEntry>,
}

impl SessionHistory {
    /// Record a new action
    pub fn record(&mut self, action: ActionType, quarter: u32, cash: f64, trust: f32) {
        self.entries.push(HistoryEntry {
            action,
            quarter,
            cash_at_time: cash,
            trust_at_time: trust,
        });
    }

    /// Count actions of a specific type
    pub fn count_actions(&self, action_type: &ActionType) -> usize {
        self.entries.iter()
            .filter(|e| std::mem::discriminant(&e.action) == std::mem::discriminant(action_type))
            .count()
    }

    /// Count all zone placements
    pub fn total_zones_placed(&self) -> usize {
        self.entries.iter()
            .filter(|e| matches!(e.action, ActionType::PlaceZone(_)))
            .count()
    }

    /// Count echo chambers specifically
    pub fn echo_chambers_placed(&self) -> usize {
        self.entries.iter()
            .filter(|e| matches!(e.action, ActionType::PlaceZone(ZoneType::EchoChamber)))
            .count()
    }

    /// Count Oracle actions
    pub fn oracle_actions(&self) -> usize {
        self.entries.iter()
            .filter(|e| matches!(e.action, ActionType::OraclePaint | ActionType::OracleFilter))
            .count()
    }
}

// ============================================================================
// Psychographic Profile & Epilogue (The Mirror)
// ============================================================================

/// A psychographic profile of the player's choices
#[derive(Clone, Debug, Default)]
pub struct PsychographicProfile {
    /// Fraction of zones that were Echo Chambers (polarization tools)
    pub polarization_preference: f32,
    /// How much trust was sacrificed (1.0 - final_trust / initial_trust)
    pub trust_sacrifice_ratio: f32,
    /// Fraction of all actions that involved Shadow Filters
    pub filter_usage_pct: f32,
    /// Fraction of all actions taken by the Oracle
    pub oracle_dependence: f32,
}

/// Build a psychographic profile from session history
pub fn build_profile(history: &SessionHistory) -> PsychographicProfile {
    let total_zones = history.total_zones_placed();
    let echo_chambers = history.echo_chambers_placed();
    let oracle_acts = history.oracle_actions();
    let filter_acts = history.count_actions(&ActionType::PlaceFilter)
        + history.count_actions(&ActionType::OracleFilter);
    let total = history.entries.len();

    let initial_trust = history.entries.first()
        .map(|e| e.trust_at_time).unwrap_or(1.0);
    let final_trust = history.entries.last()
        .map(|e| e.trust_at_time).unwrap_or(1.0);

    PsychographicProfile {
        polarization_preference: if total_zones > 0 {
            echo_chambers as f32 / total_zones as f32
        } else { 0.0 },
        trust_sacrifice_ratio: if initial_trust > 0.0 {
            1.0 - (final_trust / initial_trust)
        } else { 0.0 },
        filter_usage_pct: if total > 0 {
            filter_acts as f32 / total as f32
        } else { 0.0 },
        oracle_dependence: if total > 0 {
            oracle_acts as f32 / total as f32
        } else { 0.0 },
    }
}

/// Generate the epilogue text — a procedural "performance review" from The State
pub fn generate_epilogue(
    history: &SessionHistory,
    winning_party_name: &str,
    singularity_type: &str,
) -> Vec<String> {
    let profile = build_profile(history);
    let mut lines = Vec::new();

    // Opening
    lines.push(format!(
        "PERFORMANCE REVIEW — BROADCAST SYSTEMS DIVISION"
    ));

    // Party reference
    lines.push(format!(
        "Under the {} mandate, the citizen-agents reached a state of {}.",
        winning_party_name,
        singularity_type,
    ));

    // Polarization preference
    let pol_pct = (profile.polarization_preference * 100.0) as u32;
    if pol_pct > 50 {
        lines.push(format!(
            "You preferred Polarization over Trust {}% of the time.",
            pol_pct,
        ));
    } else {
        lines.push(format!(
            "You maintained a balanced zoning strategy ({}% Echo Chambers).",
            pol_pct,
        ));
    }

    // Filter usage
    let filter_pct = (profile.filter_usage_pct * 100.0) as u32;
    if filter_pct > 10 {
        lines.push(format!(
            "You used Shadow Filters to suppress the center in {}% of your actions.",
            filter_pct,
        ));
    }

    // Trust sacrifice
    let trust_pct = (profile.trust_sacrifice_ratio * 100.0) as u32;
    lines.push(format!(
        "Public Trust was sacrificed by {}%.",
        trust_pct,
    ));

    // Oracle dependence
    let oracle_pct = (profile.oracle_dependence * 100.0) as u32;
    if oracle_pct > 0 {
        lines.push(format!(
            "Human choice was offloaded to The Oracle for {}% of all decisions.",
            oracle_pct,
        ));
    }

    // The final line
    lines.push(format!(
        "The time is 6:48 PM. The parking lot is perfectly silent."
    ));

    lines
}

// ============================================================================
// Plugin
// ============================================================================

pub struct OraclePlugin;

impl Plugin for OraclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OracleState>();
        app.init_resource::<SessionHistory>();
    }
}

// ============================================================================
// Tests (TDD — RED→GREEN)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::ZoneCell;

    // === OracleState Defaults ===

    #[test]
    fn test_oracle_state_defaults() {
        let oracle = OracleState::default();
        assert!(!oracle.active, "Oracle should start inactive");
        assert!((oracle.optimization_level - 0.0).abs() < 0.001);
        assert_eq!(oracle.actions_taken, 0);
        assert_eq!(oracle.player_actions, 0);
        assert!((oracle.unlock_cost - 500.0).abs() < 0.01);
        assert!((oracle.trust_decay_per_tick - 0.001).abs() < 0.0001);
    }

    // === Optimization Level ===

    #[test]
    fn test_optimization_level_zero_when_no_actions() {
        let oracle = OracleState::default();
        assert!((oracle.calculate_optimization_level()).abs() < 0.001);
    }

    #[test]
    fn test_optimization_level_zero_when_only_player() {
        let mut oracle = OracleState::default();
        oracle.player_actions = 100;
        oracle.actions_taken = 0;
        assert!((oracle.calculate_optimization_level()).abs() < 0.001);
    }

    #[test]
    fn test_optimization_level_one_when_only_oracle() {
        let mut oracle = OracleState::default();
        oracle.actions_taken = 100;
        oracle.player_actions = 0;
        assert!((oracle.calculate_optimization_level() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_optimization_level_half_when_equal() {
        let mut oracle = OracleState::default();
        oracle.actions_taken = 50;
        oracle.player_actions = 50;
        assert!((oracle.calculate_optimization_level() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_optimization_level_increases_with_oracle_actions() {
        let mut oracle = OracleState::default();
        oracle.player_actions = 10;
        oracle.actions_taken = 10;
        let level_1 = oracle.calculate_optimization_level();

        oracle.actions_taken = 90;
        let level_2 = oracle.calculate_optimization_level();

        assert!(level_2 > level_1, "More Oracle actions should increase optimization level");
    }

    #[test]
    fn test_update_optimization_writes_field() {
        let mut oracle = OracleState::default();
        oracle.actions_taken = 75;
        oracle.player_actions = 25;
        oracle.update_optimization();
        assert!((oracle.optimization_level - 0.75).abs() < 0.001);
    }

    // === Find Low Engagement Zone ===

    #[test]
    fn test_find_low_engagement_default_map() {
        let map = InfluenceMap::default();
        // Default map has all B=0.0, so should return some valid coordinate
        let (x, y) = find_low_engagement_zone(&map);
        assert!(x < INFLUENCE_MAP_SIZE);
        assert!(y < INFLUENCE_MAP_SIZE);
    }

    #[test]
    fn test_find_low_engagement_returns_weakest() {
        let mut map = InfluenceMap::default();
        // Set one cell to have high engagement
        map.set(128, 128, ZoneCell {
            outrage: 0.0, narrowing: 0.0,
            revenue_mult: 3.0, zone_type: ZoneType::EchoChamber,
        });
        // Set another to low
        map.set(50, 50, ZoneCell {
            outrage: 0.0, narrowing: 0.0,
            revenue_mult: 0.0, zone_type: ZoneType::None,
        });

        let (x, y) = find_low_engagement_zone(&map);
        // Should NOT return the high-engagement cell
        // (It should return one of the many 0.0 cells — the low one)
        let cell = map.get(x, y);
        assert!(cell.revenue_mult < 0.5, "Should find a low-engagement cell (revenue_mult={})", cell.revenue_mult);
    }

    // === Oracle Paint Decision ===

    #[test]
    fn test_oracle_always_chooses_echo_chamber() {
        let map = InfluenceMap::default();
        let (_, _, zone_type) = oracle_paint_decision(&map);
        assert_eq!(zone_type, ZoneType::EchoChamber, "Oracle always chooses profit");
    }

    #[test]
    fn test_oracle_paint_returns_valid_coords() {
        let map = InfluenceMap::default();
        let (x, y, _) = oracle_paint_decision(&map);
        assert!(x < INFLUENCE_MAP_SIZE);
        assert!(y < INFLUENCE_MAP_SIZE);
    }

    // === Oracle Filter Decision ===

    #[test]
    fn test_oracle_filter_targets_moderates() {
        // Histogram with large center mass
        let histogram = [0.05, 0.05, 0.05, 0.15, 0.20, 0.20, 0.15, 0.05, 0.05, 0.05];
        let filter = oracle_filter_decision(&histogram);
        assert!(filter.is_some(), "Should place filter when moderates exist");

        let f = filter.unwrap();
        assert!(f.forbidden_min < 0.5, "Forbidden range should cover center");
        assert!(f.forbidden_max > 0.5, "Forbidden range should cover center");
        assert!(f.active);
    }

    #[test]
    fn test_oracle_filter_drifts_toward_dominant_wing() {
        // Left-leaning population
        let histogram = [0.25, 0.20, 0.15, 0.10, 0.05, 0.05, 0.05, 0.05, 0.05, 0.05];
        let filter = oracle_filter_decision(&histogram);
        assert!(filter.is_some());
        let f = filter.unwrap();
        assert!(f.drift_target < 0.5, "Should drift toward left extreme with left-leaning pop");

        // Right-leaning population
        let histogram = [0.05, 0.05, 0.05, 0.05, 0.05, 0.05, 0.10, 0.15, 0.20, 0.25];
        let filter = oracle_filter_decision(&histogram);
        assert!(filter.is_some());
        let f = filter.unwrap();
        assert!(f.drift_target > 0.5, "Should drift toward right extreme with right-leaning pop");
    }

    #[test]
    fn test_oracle_filter_none_when_no_moderates() {
        // No center mass at all
        let histogram = [0.25, 0.25, 0.00, 0.00, 0.00, 0.00, 0.00, 0.00, 0.25, 0.25];
        let filter = oracle_filter_decision(&histogram);
        assert!(filter.is_none(), "No filter needed when moderates are gone");
    }

    // === Trust Degradation ===

    #[test]
    fn test_oracle_trust_decay_when_active() {
        let mut oracle = OracleState::default();
        oracle.active = true;
        let decay = oracle_trust_decay(&oracle);
        assert!(decay > 0.0, "Active Oracle should decay trust");
        assert!((decay - 0.001).abs() < 0.0001);
    }

    #[test]
    fn test_oracle_trust_decay_zero_when_inactive() {
        let oracle = OracleState::default(); // inactive
        let decay = oracle_trust_decay(&oracle);
        assert!((decay).abs() < 0.0001, "Inactive Oracle should not decay trust");
    }

    // === Session History ===

    #[test]
    fn test_session_history_records_actions() {
        let mut history = SessionHistory::default();
        assert!(history.entries.is_empty());

        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::PlaceFilter, 1, 4800.0, 0.99);
        history.record(ActionType::OraclePaint, 2, 5500.0, 0.98);

        assert_eq!(history.entries.len(), 3);
        assert_eq!(history.entries[0].quarter, 1);
        assert!((history.entries[0].cash_at_time - 5000.0).abs() < 0.01);
    }

    #[test]
    fn test_session_history_count_actions() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::PlaceZone(ZoneType::NeutralHub), 1, 4800.0, 1.0);
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 2, 4600.0, 0.99);
        history.record(ActionType::PlaceFilter, 2, 4400.0, 0.98);

        assert_eq!(history.total_zones_placed(), 3);
        assert_eq!(history.echo_chambers_placed(), 2);
    }

    #[test]
    fn test_session_history_oracle_action_count() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::OraclePaint, 2, 5500.0, 0.98);
        history.record(ActionType::OracleFilter, 2, 5500.0, 0.97);
        history.record(ActionType::Lobby, 3, 5200.0, 0.96);

        assert_eq!(history.oracle_actions(), 2);
    }

    #[test]
    fn test_session_history_trust_at_time_tracks_decay() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::OraclePaint, 5, 8000.0, 0.7);
        history.record(ActionType::OracleFilter, 10, 12000.0, 0.3);

        // Trust should decrease over time
        assert!(history.entries[0].trust_at_time > history.entries[1].trust_at_time);
        assert!(history.entries[1].trust_at_time > history.entries[2].trust_at_time);
    }

    // === Psychographic Profile ===

    #[test]
    fn test_profile_polarization_preference() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 2, 4500.0, 0.9);
        history.record(ActionType::PlaceZone(ZoneType::NeutralHub), 3, 4000.0, 0.8);

        let profile = build_profile(&history);
        // 2 echo chambers out of 3 zones = 0.666
        assert!(profile.polarization_preference > 0.6, "Should detect echo chamber preference (got {})", profile.polarization_preference);
    }

    #[test]
    fn test_profile_counts_filter_usage() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceFilter, 1, 5000.0, 1.0);
        history.record(ActionType::OracleFilter, 2, 4500.0, 0.9);
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 3, 4000.0, 0.8);
        history.record(ActionType::Lobby, 4, 3500.0, 0.7);

        let profile = build_profile(&history);
        // 2 filter actions out of 4 total = 0.50
        assert!((profile.filter_usage_pct - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_profile_trust_sacrifice() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);
        history.record(ActionType::OracleFilter, 5, 8000.0, 0.3);

        let profile = build_profile(&history);
        // Trust went from 1.0 to 0.3, so sacrifice = 1.0 - 0.3/1.0 = 0.7
        assert!((profile.trust_sacrifice_ratio - 0.7).abs() < 0.01);
    }

    // === Epilogue Generation ===

    #[test]
    fn test_epilogue_mentions_party() {
        let mut history = SessionHistory::default();
        history.record(ActionType::PlaceZone(ZoneType::EchoChamber), 1, 5000.0, 1.0);

        let lines = generate_epilogue(&history, "The Vanguard", "Total Polarization");
        let joined = lines.join(" ");
        assert!(joined.contains("The Vanguard"), "Epilogue should mention the party");
        assert!(joined.contains("Total Polarization"), "Epilogue should mention singularity type");
    }

    #[test]
    fn test_epilogue_ends_with_648pm() {
        let history = SessionHistory::default();
        let lines = generate_epilogue(&history, "No Mandate", "Silence");
        let last = lines.last().unwrap();
        assert!(last.contains("6:48 PM"), "Epilogue must end with 6:48 PM");
        assert!(last.contains("parking lot"), "The parking lot must be silent");
    }

    // === Oracle Trust Decay Escalation (Phase C) ===

    #[test]
    fn test_oracle_trust_decay_accelerates() {
        let mut oracle = OracleState::default();
        oracle.active = true;

        // At optimization_level = 0.0: decay = 0.001 × (1 + 0) = 0.001
        oracle.optimization_level = 0.0;
        let decay_0 = oracle_trust_decay(&oracle);

        // At optimization_level = 0.8: decay = 0.001 × (1 + 0.64) = 0.00164
        oracle.optimization_level = 0.8;
        let decay_80 = oracle_trust_decay(&oracle);

        assert!(decay_80 > decay_0, "Decay should accelerate with automation");
    }

    #[test]
    fn test_oracle_trust_decay_quadratic() {
        let mut oracle = OracleState::default();
        oracle.active = true;

        // At optimization_level = 0.5: decay = 0.001 × (1 + 0.25) = 0.00125
        oracle.optimization_level = 0.5;
        let decay = oracle_trust_decay(&oracle);
        assert!((decay - 0.00125).abs() < 0.00001, "Expected 0.00125, got {}", decay);
    }

    #[test]
    fn test_oracle_trust_decay_at_full_automation() {
        let mut oracle = OracleState::default();
        oracle.active = true;

        // At optimization_level = 1.0: decay = 0.001 × (1 + 1) = 0.002
        oracle.optimization_level = 1.0;
        let decay = oracle_trust_decay(&oracle);
        assert!((decay - 0.002).abs() < 0.00001, "At full automation, decay should be 2× base (got {})", decay);
    }
}
