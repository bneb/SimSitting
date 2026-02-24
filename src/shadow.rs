//! SimSitting — Shadow Infrastructure (Phase 3: The State & The Shadow)
//!
//! Shadow Filters scan agents passing through "Forbidden Ranges."
//! If an agent's opinion falls within the forbidden range, the filter
//! forces a drift toward the target narrative. This costs NC and reduces
//! [`PublicTrust`].
//!
//! The Shadow Layer is a toggle ([`ShadowMode`]) that reveals the invisible
//! pipes connecting zone nodes.

use bevy::prelude::*;

// ============================================================================
// Shadow Filter Types
// ============================================================================

/// A Shadow Filter placed on a feed pipeline.
/// Scans agents and forces opinion drift for "dangerous moderates."
#[derive(Component, Clone, Debug)]
pub struct ShadowFilter {
    /// Lower bound of the forbidden opinion range
    pub forbidden_min: f32,
    /// Upper bound of the forbidden opinion range
    pub forbidden_max: f32,
    /// Target opinion to drift toward
    pub drift_target: f32,
    /// Drift strength per second
    pub drift_rate: f32,
    /// NC cost per agent per second
    pub nc_cost_per_agent: f64,
    /// Cumulative public trust penalty
    pub trust_penalty: f32,
    /// Is this filter active?
    pub active: bool,
}

impl Default for ShadowFilter {
    fn default() -> Self {
        Self {
            forbidden_min: 0.4,
            forbidden_max: 0.6,
            drift_target: 0.1,
            drift_rate: 0.05,
            nc_cost_per_agent: 0.01,
            trust_penalty: 0.001,
            active: true,
        }
    }
}

impl ShadowFilter {
    /// Check if an agent's opinion falls within the forbidden range
    pub fn is_in_forbidden_range(&self, opinion: f32) -> bool {
        opinion >= self.forbidden_min && opinion <= self.forbidden_max
    }

    /// Calculate the opinion drift for an agent in the forbidden range.
    /// Returns the new opinion after drift is applied.
    pub fn apply_drift(&self, opinion: f32, dt: f32) -> f32 {
        if !self.is_in_forbidden_range(opinion) || !self.active {
            return opinion;
        }
        let direction = if self.drift_target > opinion { 1.0 } else { -1.0 };
        let drift_amount = self.drift_rate * dt * direction;
        (opinion + drift_amount).clamp(0.0, 1.0)
    }

    /// Calculate the NC cost for processing a batch of agents
    pub fn batch_nc_cost(&self, agents_in_range: u32, dt: f32) -> f64 {
        self.nc_cost_per_agent * agents_in_range as f64 * dt as f64
    }
}

/// A pipeline connecting two zones, visualized in Shadow Mode
#[derive(Component, Clone, Debug)]
pub struct FilterPipeline {
    /// Source zone position (world coords)
    pub source: Vec2,
    /// Target zone position (world coords)
    pub target: Vec2,
    /// Pipeline capacity (max agents processed per second)
    pub capacity: u32,
    /// Is this pipeline visible in shadow mode?
    pub visible: bool,
}

impl Default for FilterPipeline {
    fn default() -> Self {
        Self {
            source: Vec2::ZERO,
            target: Vec2::new(100.0, 0.0),
            capacity: 1000,
            visible: true,
        }
    }
}

impl FilterPipeline {
    /// Calculate the pipeline length
    pub fn length(&self) -> f32 {
        (self.target - self.source).length()
    }

    /// Check if a position is within range of this pipeline
    pub fn is_within_range(&self, pos: Vec2, range: f32) -> bool {
        // Point-to-line-segment distance
        let ab = self.target - self.source;
        let ap = pos - self.source;
        let t = ap.dot(ab) / ab.dot(ab);
        let t_clamped = t.clamp(0.0, 1.0);
        let closest = self.source + ab * t_clamped;
        (pos - closest).length() <= range
    }
}

/// Shadow Mode toggle — reveals invisible infrastructure
#[derive(Resource, Default)]
pub struct ShadowMode {
    pub enabled: bool,
}

/// Public Trust meter — damaged by shadow filters, game over at 0
#[derive(Resource)]
pub struct PublicTrust {
    pub value: f32,
}

impl Default for PublicTrust {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

/// Result of applying shadow filters to a batch of agents
#[derive(Debug)]
pub struct ShadowResult {
    pub agents_filtered: u32,
    pub nc_spent: f64,
    pub trust_lost: f32,
}

/// Apply a shadow filter to a batch of agent opinions.
/// Returns the modified opinions, cost, and trust penalty.
pub fn apply_filter_to_batch(
    filter: &ShadowFilter,
    opinions: &mut [f32],
    dt: f32,
) -> ShadowResult {
    let mut filtered = 0u32;

    for opinion in opinions.iter_mut() {
        if filter.is_in_forbidden_range(*opinion) {
            *opinion = filter.apply_drift(*opinion, dt);
            filtered += 1;
        }
    }

    ShadowResult {
        agents_filtered: filtered,
        nc_spent: filter.batch_nc_cost(filtered, dt),
        trust_lost: filter.trust_penalty * filtered as f32,
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct ShadowPlugin;

impl Plugin for ShadowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShadowMode>();
        app.init_resource::<PublicTrust>();
    }
}

// ============================================================================
// Tests (TDD — RED first)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === Shadow Filter Defaults ===

    #[test]
    fn test_shadow_filter_defaults() {
        let filter = ShadowFilter::default();
        assert!((filter.forbidden_min - 0.4).abs() < 0.001);
        assert!((filter.forbidden_max - 0.6).abs() < 0.001);
        assert!((filter.drift_target - 0.1).abs() < 0.001);
        assert!(filter.active);
    }

    // === Forbidden Range Detection ===

    #[test]
    fn test_forbidden_range_center_is_forbidden() {
        let filter = ShadowFilter::default(); // 0.4–0.6
        assert!(filter.is_in_forbidden_range(0.5), "Center opinion should be forbidden");
    }

    #[test]
    fn test_forbidden_range_edge_is_forbidden() {
        let filter = ShadowFilter::default();
        assert!(filter.is_in_forbidden_range(0.4), "Lower edge is inclusive");
        assert!(filter.is_in_forbidden_range(0.6), "Upper edge is inclusive");
    }

    #[test]
    fn test_forbidden_range_outside_is_safe() {
        let filter = ShadowFilter::default();
        assert!(!filter.is_in_forbidden_range(0.1), "Extreme left is safe");
        assert!(!filter.is_in_forbidden_range(0.9), "Extreme right is safe");
        assert!(!filter.is_in_forbidden_range(0.39), "Just below min is safe");
        assert!(!filter.is_in_forbidden_range(0.61), "Just above max is safe");
    }

    // === Drift Mechanics ===

    #[test]
    fn test_drift_moves_toward_target() {
        let filter = ShadowFilter {
            drift_target: 0.1, // Drift left
            drift_rate: 0.05,
            ..ShadowFilter::default()
        };
        let original = 0.5;
        let drifted = filter.apply_drift(original, 1.0);
        assert!(drifted < original, "Should drift left toward 0.1 (got {})", drifted);
    }

    #[test]
    fn test_drift_moves_right_toward_target() {
        let filter = ShadowFilter {
            drift_target: 0.9, // Drift right
            drift_rate: 0.05,
            ..ShadowFilter::default()
        };
        let original = 0.5;
        let drifted = filter.apply_drift(original, 1.0);
        assert!(drifted > original, "Should drift right toward 0.9 (got {})", drifted);
    }

    #[test]
    fn test_drift_is_proportional_to_dt() {
        let filter = ShadowFilter::default();
        let short_drift = filter.apply_drift(0.5, 0.1);
        let long_drift = filter.apply_drift(0.5, 1.0);
        // Longer dt = more drift
        assert!((0.5 - long_drift).abs() > (0.5 - short_drift).abs(),
            "Longer dt should produce more drift");
    }

    #[test]
    fn test_drift_does_not_affect_outside_range() {
        let filter = ShadowFilter::default();
        let opinion = 0.1; // Outside forbidden range
        let drifted = filter.apply_drift(opinion, 1.0);
        assert!((drifted - opinion).abs() < 0.001, "Should not drift outside range");
    }

    #[test]
    fn test_drift_inactive_filter_no_effect() {
        let filter = ShadowFilter {
            active: false,
            ..ShadowFilter::default()
        };
        let drifted = filter.apply_drift(0.5, 1.0);
        assert!((drifted - 0.5).abs() < 0.001, "Inactive filter should not drift");
    }

    #[test]
    fn test_drift_clamps_to_bounds() {
        let filter = ShadowFilter {
            forbidden_min: 0.0,
            forbidden_max: 1.0,
            drift_target: 0.0,
            drift_rate: 10.0, // Very aggressive
            ..ShadowFilter::default()
        };
        let drifted = filter.apply_drift(0.05, 1.0);
        assert!(drifted >= 0.0, "Opinion should never go below 0.0 (got {})", drifted);
    }

    // === Batch Processing ===

    #[test]
    fn test_batch_nc_cost() {
        let filter = ShadowFilter::default();
        let cost = filter.batch_nc_cost(100, 1.0);
        // 0.01 NC/agent/sec × 100 agents × 1.0s = 1.0 NC
        assert!((cost - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_apply_filter_to_batch() {
        let filter = ShadowFilter::default();
        let mut opinions = vec![0.1, 0.3, 0.5, 0.5, 0.7, 0.9];
        let result = apply_filter_to_batch(&filter, &mut opinions, 1.0);

        // Only opinions in 0.4–0.6 should be affected (two at 0.5)
        assert_eq!(result.agents_filtered, 2, "Only 2 agents in forbidden range");
        assert!(result.nc_spent > 0.0);
        assert!(result.trust_lost > 0.0);

        // The filtered agents should have drifted
        assert!(opinions[2] < 0.5, "Agent at 0.5 should drift left toward 0.1");
        assert!(opinions[3] < 0.5, "Agent at 0.5 should drift left toward 0.1");

        // Unaffected agents should not change
        assert!((opinions[0] - 0.1).abs() < 0.001);
        assert!((opinions[4] - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_apply_filter_no_agents_in_range() {
        let filter = ShadowFilter::default(); // 0.4–0.6
        let mut opinions = vec![0.1, 0.2, 0.8, 0.9];
        let result = apply_filter_to_batch(&filter, &mut opinions, 1.0);
        assert_eq!(result.agents_filtered, 0);
        assert!((result.nc_spent).abs() < 0.001);
    }

    // === Pipeline ===

    #[test]
    fn test_pipeline_length() {
        let pipe = FilterPipeline {
            source: Vec2::ZERO,
            target: Vec2::new(100.0, 0.0),
            ..FilterPipeline::default()
        };
        assert!((pipe.length() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_pipeline_within_range() {
        let pipe = FilterPipeline {
            source: Vec2::ZERO,
            target: Vec2::new(100.0, 0.0),
            ..FilterPipeline::default()
        };
        // Point on the line
        assert!(pipe.is_within_range(Vec2::new(50.0, 0.0), 5.0));
        // Point near the line
        assert!(pipe.is_within_range(Vec2::new(50.0, 3.0), 5.0));
        // Point far from line
        assert!(!pipe.is_within_range(Vec2::new(50.0, 20.0), 5.0));
    }

    #[test]
    fn test_pipeline_within_range_endpoint() {
        let pipe = FilterPipeline::default();
        // At source
        assert!(pipe.is_within_range(Vec2::ZERO, 5.0));
        // At target
        assert!(pipe.is_within_range(Vec2::new(100.0, 0.0), 5.0));
    }

    // === Public Trust ===

    #[test]
    fn test_public_trust_default() {
        let trust = PublicTrust::default();
        assert!((trust.value - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_shadow_mode_default_off() {
        let mode = ShadowMode::default();
        assert!(!mode.enabled);
    }

    // === Shadow Result ===

    #[test]
    fn test_shadow_result_trust_lost_proportional() {
        let filter = ShadowFilter::default();
        let mut opinions_few = vec![0.5; 10];
        let mut opinions_many = vec![0.5; 100];

        let result_few = apply_filter_to_batch(&filter, &mut opinions_few, 1.0);
        let result_many = apply_filter_to_batch(&filter, &mut opinions_many, 1.0);

        assert!(result_many.trust_lost > result_few.trust_lost,
            "More filtered agents should cause more trust loss");
    }
}
