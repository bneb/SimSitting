//! SimSitting — HUMINT Profiler Engine
//!
//! Procedural "life data" generator that maps agent state to human behavior.
//! Makes each of the 100,000 dots feel like a person — right before the
//! system erases them.
//!
//! ## Architecture
//!
//! All generation is deterministic from `(id, opinion, engagement, zone, personhood)`.
//! No randomness needed — the procedural rhetori comes from the input space itself.
//!
//! When `personhood < 0.05`, the agent is "erased":
//! - Name becomes `ASSET_[hex_hash]`
//! - Queries become `[OPTIMIZED_CONTENT_STREAM]`
//! - Occupation becomes `[ROLE_DEPRECATED]`

use crate::zone::ZoneType;
use bevy::prelude::*;

// ============================================================================
// Data Structures
// ============================================================================

/// Procedural life data for display in the HUMINT profile window.
#[derive(Clone, Debug, PartialEq)]
pub struct SimLifeData {
    pub name: String,
    pub occupation: String,
    pub search_query: String,
    pub political_lean: String,
    pub personhood_pct: u32,
    pub is_erased: bool,
}

/// Up to 3 pinned sims for HUMINT tracking.
#[derive(Resource, Default, Clone, Debug)]
pub struct SimSelection {
    pub hovered: Option<Entity>,
    pub pinned: Vec<PinnedSim>,
}

/// A single pinned sim reference.
#[derive(Clone, Debug)]
pub struct PinnedSim {
    pub entity: Entity,
    pub agent_id: u32,
}

impl SimSelection {
    /// Pin a sim. Returns false if already at max (3) or already pinned.
    pub fn pin(&mut self, entity: Entity, agent_id: u32) -> bool {
        if self.pinned.len() >= 3 {
            return false;
        }
        if self.pinned.iter().any(|p| p.entity == entity) {
            return false;
        }
        self.pinned.push(PinnedSim { entity, agent_id });
        true
    }

    /// Unpin a sim by entity.
    pub fn unpin(&mut self, entity: Entity) {
        self.pinned.retain(|p| p.entity != entity);
    }

    /// Check if a sim is pinned.
    pub fn is_pinned(&self, entity: Entity) -> bool {
        self.pinned.iter().any(|p| p.entity == entity)
    }
}

// ============================================================================
// Procedural Generation Arrays
// ============================================================================

/// 20 occupations — keyed by `(id % 20)`
const OCCUPATIONS: [&str; 20] = [
    "Data Entry (Redundant)",
    "Content Moderator",
    "Gig-Driver",
    "Supply Chain Analyst",
    "Social Media Manager",
    "Call Center Operator",
    "Warehouse Picker",
    "Insurance Adjuster",
    "Copywriter (Automated)",
    "IT Help Desk",
    "Freelance Designer",
    "QA Tester",
    "Parking Enforcement",
    "Medical Billing",
    "Adjunct Professor",
    "Rideshare Safety Monitor",
    "News Aggregation Intern",
    "Claims Processor",
    "Substitute Teacher",
    "Customer Success Associate",
];

/// 60 search queries distributed across opinion spectrum and zone type.
/// Indexed by `(zone_index * 20 + opinion_bucket)` where opinion_bucket = (opinion * 19) as usize
const QUERIES: [&str; 60] = [
    // Echo Chamber (0..20): confirmation bias queries
    "how to prove they are wrong",
    "my side of the story explained",
    "why everyone else is wrong about",
    "echo chamber or common sense",
    "alternative facts 2024",
    "people who agree with me near me",
    "is it okay to unfriend family",
    "confirmation bias test (just to be sure)",
    "my opinion backed by science",
    "people who disagree are mentally ill",
    "what do the experts REALLY say",
    "mainstream media is lying proof",
    "how to win any argument",
    "am i the only one who sees this",
    "top 10 reasons they are wrong",
    "filter bubble benefits",
    "information warfare for beginners",
    "my truth vs their truth",
    "echo chamber definition (just checking)",
    "how to report misinformation (theirs)",

    // Neutral Hub (20..40): mundane queries reflecting apathy
    "top 10 office supplies 1996",
    "why is the sunset purple",
    "cheaper rent near echo chamber",
    "how to stop feeling apathetic",
    "weather tomorrow",
    "best white noise for sleeping",
    "is anyone else tired all the time",
    "recipe for something i already have",
    "what day is it",
    "nearest park that is quiet",
    "how to care about things again",
    "ambient music for working late",
    "why do i keep refreshing",
    "digital detox weekend ideas",
    "meaning of life reddit thread",
    "is doomscrolling harmful",
    "free meditation app 2024",
    "when did news become entertainment",
    "6:47 PM sunset meaning",
    "why cant i stop watching",

    // Data Refinery (40..60): surveillance/paranoia queries
    "is someone watching me",
    "how to tell if phone is tapped",
    "vpn that actually works",
    "data broker removal service",
    "how much does google know about me",
    "dark web monitoring free",
    "employer monitoring software detect",
    "facial recognition opt out",
    "burner phone best practices 2024",
    "who owns my data",
    "terms of service too long to read",
    "is my smart tv listening",
    "ring doorbell police access",
    "digital footprint eraser",
    "edward snowden was right",
    "end-to-end encryption for dummies",
    "how many cameras watch me daily",
    "right to be forgotten where",
    "algorithmic harm examples",
    "am i a product or a customer",
];

/// Political lean descriptions — keyed by opinion value
const POLITICAL_LEANS: [&str; 5] = [
    "Progressive (Hopeful)",
    "Left-Leaning (Anxious)",
    "Center (Numb)",
    "Right-Leaning (Frustrated)",
    "Authoritarian (Certain)",
];

/// Erased-state occupation
const ERASED_OCCUPATION: &str = "[ROLE_DEPRECATED]";
/// Erased-state query
const ERASED_QUERY: &str = "[OPTIMIZED_CONTENT_STREAM]";

// ============================================================================
// Generation
// ============================================================================

/// Input for life data generation — decoupled from ECS for testability.
#[derive(Clone, Debug)]
pub struct HumintInput {
    pub agent_id: u32,
    pub opinion: f32,
    pub engagement: f32,
    pub zone: ZoneType,
    pub personhood: f32,
}

/// Generate procedural life data from agent state.
///
/// **Pure function** — deterministic, no randomness, no ECS.
pub fn generate_life_data(input: &HumintInput) -> SimLifeData {
    let is_erased = input.personhood < 0.05;

    // Name
    let name = if is_erased {
        // Hash the ID to make it feel algorithmic
        let hash = simple_hash(input.agent_id);
        format!("ASSET_{:08X}", hash)
    } else {
        let suffix = ((input.opinion * 100.0) as u32) % 100;
        format!("Subject #{:04}-{}", input.agent_id, (b'A' + (suffix % 26) as u8) as char)
    };

    // Occupation
    let occupation = if is_erased {
        ERASED_OCCUPATION.to_string()
    } else {
        OCCUPATIONS[(input.agent_id as usize) % OCCUPATIONS.len()].to_string()
    };

    // Search query
    let search_query = if is_erased {
        ERASED_QUERY.to_string()
    } else {
        let zone_offset = match input.zone {
            ZoneType::EchoChamber => 0,
            ZoneType::NeutralHub => 20,
            ZoneType::DataRefinery => 40,
            _ => 20, // Default to neutral
        };
        let opinion_bucket = ((input.opinion * 19.0) as usize).min(19);
        QUERIES[zone_offset + opinion_bucket].to_string()
    };

    // Political lean
    let political_lean = if is_erased {
        "[CLASSIFICATION_UNNECESSARY]".to_string()
    } else {
        let lean_idx = ((input.opinion * 4.0) as usize).min(4);
        POLITICAL_LEANS[lean_idx].to_string()
    };

    SimLifeData {
        name,
        occupation,
        search_query,
        political_lean,
        personhood_pct: (input.personhood * 100.0).round().clamp(0.0, 100.0) as u32,
        is_erased,
    }
}

/// Simple deterministic hash for agent IDs.
fn simple_hash(id: u32) -> u32 {
    let mut h = id;
    h = h.wrapping_mul(2654435769);
    h ^= h >> 16;
    h = h.wrapping_mul(2246822519);
    h ^= h >> 13;
    h
}

// ============================================================================
// Personhood Decay
// ============================================================================

/// Input for decay computation — decoupled from ECS.
#[derive(Clone, Debug)]
pub struct PersonhoodDecayInput {
    pub current_personhood: f32,
    pub in_echo_chamber: bool,
    pub under_shadow_filter: bool,
    pub oracle_active: bool,
    pub optimization_level: f32,
    pub dt: f32,
}

/// Compute new personhood value after one tick of decay/recovery.
///
/// **Pure function** — deterministic, no side effects.
///
/// Decay rates per second:
/// - Echo Chamber: -0.002
/// - Shadow Filter: -0.003
/// - Oracle (global): -0.005 × (1 + optimization_level)
/// - Recovery (outside all zones): +0.001 (capped at 0.95, never fully heals)
pub fn compute_personhood_decay(input: &PersonhoodDecayInput) -> f32 {
    let mut delta: f32 = 0.0;

    // Decay sources (cumulative)
    if input.in_echo_chamber {
        delta -= 0.002;
    }
    if input.under_shadow_filter {
        delta -= 0.003;
    }
    if input.oracle_active {
        delta -= 0.005 * (1.0 + input.optimization_level);
    }

    // Recovery: only if no decay sources are active
    if delta == 0.0 && input.current_personhood < 0.95 {
        delta = 0.001;
    }

    let new = input.current_personhood + delta * input.dt;
    new.clamp(0.0, 0.95) // Never fully recovers. The damage is done.
}

// ============================================================================
// Plugin
// ============================================================================

pub struct HumintPlugin;

impl Plugin for HumintPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimSelection>();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn default_input() -> HumintInput {
        HumintInput {
            agent_id: 8291,
            opinion: 0.5,
            engagement: 0.7,
            zone: ZoneType::NeutralHub,
            personhood: 1.0,
        }
    }

    fn default_decay_input() -> PersonhoodDecayInput {
        PersonhoodDecayInput {
            current_personhood: 1.0,
            in_echo_chamber: false,
            under_shadow_filter: false,
            oracle_active: false,
            optimization_level: 0.0,
            dt: 1.0,
        }
    }

    // =========================================================================
    // Name Generation
    // =========================================================================

    #[test]
    fn test_name_has_subject_prefix() {
        let data = generate_life_data(&default_input());
        assert!(data.name.starts_with("Subject #"), "Got: {}", data.name);
    }

    #[test]
    fn test_name_includes_agent_id() {
        let data = generate_life_data(&default_input());
        assert!(data.name.contains("8291"), "Name should include agent ID: {}", data.name);
    }

    #[test]
    fn test_name_erased_is_hash() {
        let mut input = default_input();
        input.personhood = 0.0;
        let data = generate_life_data(&input);
        assert!(data.name.starts_with("ASSET_"), "Erased name should be hash: {}", data.name);
        assert!(!data.name.contains("Subject"), "Erased name should not contain Subject");
    }

    #[test]
    fn test_different_ids_different_names() {
        let mut input1 = default_input();
        let mut input2 = default_input();
        input2.agent_id = 1234;
        let d1 = generate_life_data(&input1);
        let d2 = generate_life_data(&input2);
        assert_ne!(d1.name, d2.name, "Different IDs should give different names");
    }

    // =========================================================================
    // Occupation
    // =========================================================================

    #[test]
    fn test_occupation_is_nonempty() {
        let data = generate_life_data(&default_input());
        assert!(!data.occupation.is_empty());
    }

    #[test]
    fn test_occupation_erased() {
        let mut input = default_input();
        input.personhood = 0.0;
        let data = generate_life_data(&input);
        assert_eq!(data.occupation, "[ROLE_DEPRECATED]");
    }

    #[test]
    fn test_occupation_varies_by_id() {
        let mut input1 = default_input();
        input1.agent_id = 0;
        let mut input2 = default_input();
        input2.agent_id = 1;
        let d1 = generate_life_data(&input1);
        let d2 = generate_life_data(&input2);
        // IDs 0 and 1 should map to different occupations (index 0 and 1)
        assert_ne!(d1.occupation, d2.occupation);
    }

    // =========================================================================
    // Search Queries
    // =========================================================================

    #[test]
    fn test_query_echo_chamber() {
        let mut input = default_input();
        input.zone = ZoneType::EchoChamber;
        let data = generate_life_data(&input);
        // Echo chamber queries are indices 0..20
        assert!(QUERIES[0..20].contains(&data.search_query.as_str()),
            "Echo chamber query not in expected set: {}", data.search_query);
    }

    #[test]
    fn test_query_neutral_hub() {
        let input = default_input(); // zone = NeutralHub
        let data = generate_life_data(&input);
        assert!(QUERIES[20..40].contains(&data.search_query.as_str()),
            "Neutral hub query not in expected set: {}", data.search_query);
    }

    #[test]
    fn test_query_data_refinery() {
        let mut input = default_input();
        input.zone = ZoneType::DataRefinery;
        let data = generate_life_data(&input);
        assert!(QUERIES[40..60].contains(&data.search_query.as_str()),
            "Data refinery query not in expected set: {}", data.search_query);
    }

    #[test]
    fn test_query_erased() {
        let mut input = default_input();
        input.personhood = 0.0;
        let data = generate_life_data(&input);
        assert_eq!(data.search_query, "[OPTIMIZED_CONTENT_STREAM]");
    }

    #[test]
    fn test_query_varies_by_opinion() {
        let mut low_opinion = default_input();
        low_opinion.opinion = 0.0;
        let mut high_opinion = default_input();
        high_opinion.opinion = 1.0;
        let d_low = generate_life_data(&low_opinion);
        let d_high = generate_life_data(&high_opinion);
        assert_ne!(d_low.search_query, d_high.search_query,
            "Different opinions should give different queries");
    }

    #[test]
    fn test_50_unique_queries() {
        // We should get at least 50 unique queries across the array
        let unique: std::collections::HashSet<&str> = QUERIES.iter().copied().collect();
        assert!(unique.len() >= 50,
            "Expected 50+ unique queries, got {}", unique.len());
    }

    #[test]
    fn test_all_60_queries_distinct() {
        let unique: std::collections::HashSet<&str> = QUERIES.iter().copied().collect();
        assert_eq!(unique.len(), 60, "All 60 queries should be unique");
    }

    // =========================================================================
    // Political Lean
    // =========================================================================

    #[test]
    fn test_political_lean_low_opinion() {
        let mut input = default_input();
        input.opinion = 0.0;
        let data = generate_life_data(&input);
        assert_eq!(data.political_lean, "Progressive (Hopeful)");
    }

    #[test]
    fn test_political_lean_high_opinion() {
        let mut input = default_input();
        input.opinion = 1.0;
        let data = generate_life_data(&input);
        assert_eq!(data.political_lean, "Authoritarian (Certain)");
    }

    #[test]
    fn test_political_lean_center() {
        let mut input = default_input();
        input.opinion = 0.5;
        let data = generate_life_data(&input);
        assert_eq!(data.political_lean, "Center (Numb)");
    }

    #[test]
    fn test_political_lean_erased() {
        let mut input = default_input();
        input.personhood = 0.0;
        let data = generate_life_data(&input);
        assert_eq!(data.political_lean, "[CLASSIFICATION_UNNECESSARY]");
    }

    // =========================================================================
    // Personhood & Erasure
    // =========================================================================

    #[test]
    fn test_personhood_pct_full() {
        let data = generate_life_data(&default_input());
        assert_eq!(data.personhood_pct, 100);
        assert!(!data.is_erased);
    }

    #[test]
    fn test_personhood_pct_half() {
        let mut input = default_input();
        input.personhood = 0.5;
        let data = generate_life_data(&input);
        assert_eq!(data.personhood_pct, 50);
        assert!(!data.is_erased);
    }

    #[test]
    fn test_erasure_threshold() {
        // Just above threshold
        let mut input = default_input();
        input.personhood = 0.05;
        let data = generate_life_data(&input);
        assert!(!data.is_erased, "At 0.05 should not be erased");

        // Just below threshold
        input.personhood = 0.049;
        let data = generate_life_data(&input);
        assert!(data.is_erased, "At 0.049 should be erased");
    }

    #[test]
    fn test_erased_agent_complete_state() {
        let mut input = default_input();
        input.personhood = 0.0;
        let data = generate_life_data(&input);
        assert!(data.is_erased);
        assert!(data.name.starts_with("ASSET_"));
        assert_eq!(data.occupation, "[ROLE_DEPRECATED]");
        assert_eq!(data.search_query, "[OPTIMIZED_CONTENT_STREAM]");
        assert_eq!(data.political_lean, "[CLASSIFICATION_UNNECESSARY]");
        assert_eq!(data.personhood_pct, 0);
    }

    // =========================================================================
    // SimSelection / Pinning
    // =========================================================================

    #[test]
    fn test_pin_adds_entity() {
        let mut sel = SimSelection::default();
        let e = Entity::from_raw(42);
        assert!(sel.pin(e, 42));
        assert_eq!(sel.pinned.len(), 1);
        assert!(sel.is_pinned(e));
    }

    #[test]
    fn test_pin_max_three() {
        let mut sel = SimSelection::default();
        assert!(sel.pin(Entity::from_raw(1), 1));
        assert!(sel.pin(Entity::from_raw(2), 2));
        assert!(sel.pin(Entity::from_raw(3), 3));
        assert!(!sel.pin(Entity::from_raw(4), 4)); // Fourth fails
        assert_eq!(sel.pinned.len(), 3);
    }

    #[test]
    fn test_pin_no_duplicates() {
        let mut sel = SimSelection::default();
        let e = Entity::from_raw(42);
        assert!(sel.pin(e, 42));
        assert!(!sel.pin(e, 42)); // Duplicate fails
        assert_eq!(sel.pinned.len(), 1);
    }

    #[test]
    fn test_unpin() {
        let mut sel = SimSelection::default();
        let e = Entity::from_raw(42);
        sel.pin(e, 42);
        sel.unpin(e);
        assert!(!sel.is_pinned(e));
        assert_eq!(sel.pinned.len(), 0);
    }

    #[test]
    fn test_unpin_nonexistent_noop() {
        let mut sel = SimSelection::default();
        sel.unpin(Entity::from_raw(999)); // Should not panic
        assert_eq!(sel.pinned.len(), 0);
    }

    // =========================================================================
    // Personhood Decay (Pure Function)
    // =========================================================================

    #[test]
    fn test_decay_no_sources_recovery() {
        let input = default_decay_input();
        let new = compute_personhood_decay(&input);
        // No decay sources, but personhood = 1.0 which is > 0.95, so no recovery
        // Should stay at 0.95 cap
        assert!(new <= 0.95, "Should cap at 0.95: {}", new);
    }

    #[test]
    fn test_decay_recovery_when_damaged() {
        let mut input = default_decay_input();
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Recovery: +0.001 × 1.0 = 0.501
        assert!((new - 0.501).abs() < 0.001, "Expected ~0.501, got {}", new);
    }

    #[test]
    fn test_decay_echo_chamber() {
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        let new = compute_personhood_decay(&input);
        // Decay: -0.002 × 1.0 = 0.998 → capped at 0.95
        assert!(new <= 0.95, "Should cap at 0.95: {}", new);
    }

    #[test]
    fn test_decay_echo_chamber_from_low() {
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Decay: 0.5 - 0.002 = 0.498
        assert!((new - 0.498).abs() < 0.001, "Expected ~0.498, got {}", new);
    }

    #[test]
    fn test_decay_shadow_filter() {
        let mut input = default_decay_input();
        input.under_shadow_filter = true;
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Decay: 0.5 - 0.003 = 0.497
        assert!((new - 0.497).abs() < 0.001, "Expected ~0.497, got {}", new);
    }

    #[test]
    fn test_decay_oracle_active() {
        let mut input = default_decay_input();
        input.oracle_active = true;
        input.optimization_level = 0.0;
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Decay: -0.005 × (1 + 0) × 1.0 = 0.495
        assert!((new - 0.495).abs() < 0.001, "Expected ~0.495, got {}", new);
    }

    #[test]
    fn test_decay_oracle_scales_with_optimization() {
        let mut input = default_decay_input();
        input.oracle_active = true;
        input.optimization_level = 1.0;
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Decay: -0.005 × (1 + 1) × 1.0 = -0.01 → 0.49
        assert!((new - 0.49).abs() < 0.001, "Expected ~0.49, got {}", new);
    }

    #[test]
    fn test_decay_cumulative_all_sources() {
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        input.under_shadow_filter = true;
        input.oracle_active = true;
        input.optimization_level = 0.5;
        input.current_personhood = 0.5;
        let new = compute_personhood_decay(&input);
        // Echo: -0.002, Shadow: -0.003, Oracle: -0.005 × 1.5 = -0.0075
        // Total: -0.0125 × 1.0 dt = 0.4875
        assert!((new - 0.4875).abs() < 0.001, "Expected ~0.4875, got {}", new);
    }

    #[test]
    fn test_decay_floors_at_zero() {
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        input.under_shadow_filter = true;
        input.oracle_active = true;
        input.optimization_level = 1.0;
        input.current_personhood = 0.001; // Almost gone
        let new = compute_personhood_decay(&input);
        assert!(new >= 0.0, "Personhood should never go negative: {}", new);
    }

    #[test]
    fn test_recovery_never_exceeds_95() {
        let mut input = default_decay_input();
        input.current_personhood = 0.94;
        let new = compute_personhood_decay(&input);
        // Recovery: 0.94 + 0.001 = 0.941, but capped at 0.95
        assert!(new <= 0.95, "Should cap at 0.95: {}", new);
        assert!(new > 0.94, "Should recover from 0.94: {}", new);
    }

    #[test]
    fn test_no_recovery_when_decaying() {
        // If any decay source is active, no recovery
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        input.current_personhood = 0.3;
        let new = compute_personhood_decay(&input);
        assert!(new < 0.3, "Should decay, not recover: {}", new);
    }

    #[test]
    fn test_dt_scaling() {
        let mut input = default_decay_input();
        input.in_echo_chamber = true;
        input.current_personhood = 0.5;

        let dt_1 = {
            let mut i = input.clone();
            i.dt = 1.0;
            compute_personhood_decay(&i)
        };
        let dt_2 = {
            let mut i = input.clone();
            i.dt = 2.0;
            compute_personhood_decay(&i)
        };
        // dt=2 should decay twice as much
        let delta_1 = 0.5 - dt_1;
        let delta_2 = 0.5 - dt_2;
        assert!((delta_2 - 2.0 * delta_1).abs() < 0.0001,
            "dt scaling: delta_1={}, delta_2={}", delta_1, delta_2);
    }
}
