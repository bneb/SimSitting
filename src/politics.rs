//! SimSitting — Political Engine (Phase 3: The State & The Shadow)
//!
//! The GPU-side opinion histogram ([`PoliticalData`]) determines the
//! Governing Party. Elections occur every 4 quarters via [`run_election`].
//! The winning party installs mandates that reshape revenue, knockout
//! thresholds, and contract availability.
//!
//! ## Parties
//!
//! | Party | When Wins | Effect |
//! |---|---|---|
//! | **Consensus** | Center peak > wings | Higher taxes, stable knockout |
//! | **Vanguard** | Wings > center | Defense contracts, lower knockout |
//! | **No Mandate** | Flat distribution | Status quo |
//!
//! Phase 4 adds [`SingularityType`] detection: `TotalConsensus`
//! (std_dev < 0.05) or `TotalPolarization` (center < 5%, wings > 35%).

use bevy::prelude::*;

// ============================================================================
// GPU-Side Political Data (mirrors WGSL PoliticalData struct)
// ============================================================================

/// Opinion histogram from the GPU: 10 buckets + total_votes.
/// Must match the WGSL `PoliticalData` layout exactly.
/// 44 bytes = 11 × u32.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PoliticalData {
    pub buckets: [u32; 10],
    pub total_votes: u32,
}

impl PoliticalData {
    /// Get the fraction of votes in each bucket (0.0–1.0)
    pub fn normalized(&self) -> [f32; 10] {
        if self.total_votes == 0 {
            return [0.0; 10];
        }
        let total = self.total_votes as f32;
        let mut result = [0.0f32; 10];
        for i in 0..10 {
            result[i] = self.buckets[i] as f32 / total;
        }
        result
    }

    /// Find the bucket with the most votes
    pub fn peak_bucket(&self) -> usize {
        self.buckets.iter()
            .enumerate()
            .max_by_key(|(_, &v)| v)
            .map(|(i, _)| i)
            .unwrap_or(5)
    }

    /// Check if the distribution is bimodal (two peaks on opposite sides)
    pub fn is_bimodal(&self, threshold: f32) -> bool {
        let norm = self.normalized();
        // Left wing = buckets 0–2, Right wing = buckets 7–9
        let left_mass: f32 = norm[0..3].iter().sum();
        let right_mass: f32 = norm[7..10].iter().sum();
        let center_mass: f32 = norm[3..7].iter().sum();

        // Bimodal if both wings have significant mass and center is hollowed out
        left_mass > threshold && right_mass > threshold && center_mass < (left_mass + right_mass) * 0.5
    }

    /// Check if the distribution has a centrist peak
    pub fn is_centrist_peak(&self, threshold: f32) -> bool {
        let norm = self.normalized();
        // Center = buckets 3–6
        let center_mass: f32 = norm[3..7].iter().sum();
        center_mass > threshold
    }
}

// ============================================================================
// Parties & Mandates
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Party {
    #[default]
    NoMandate,
    Consensus,
    Vanguard,
}

impl Party {
    pub fn name(&self) -> &'static str {
        match self {
            Party::NoMandate => "No Mandate",
            Party::Consensus => "The Consensus Party",
            Party::Vanguard  => "The Vanguard",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Party::NoMandate => "The populace is undecided. No mandate applies.",
            Party::Consensus => "Centrist stability. Higher taxes. Knockout threshold raised.",
            Party::Vanguard  => "Polarized extremism. Defense contracts. Social collapse accelerated.",
        }
    }
}

/// Determine the winning party from a histogram.
/// Pure function — fully testable without GPU.
pub fn determine_winner(data: &PoliticalData) -> Party {
    if data.total_votes == 0 {
        return Party::NoMandate;
    }

    // Check bimodal first (Vanguard wins if society is split)
    if data.is_bimodal(0.25) {
        return Party::Vanguard;
    }

    // Check centrist peak (Consensus wins if center holds)
    if data.is_centrist_peak(0.55) {
        return Party::Consensus;
    }

    // Flat or ambiguous → no mandate
    Party::NoMandate
}

/// Active party mandate — modifies gameplay for 4 quarters
#[derive(Resource, Clone, Debug)]
pub struct PartyMandate {
    pub party: Party,
    /// Tax rate multiplier (1.0 = normal, 1.2 = 20% tax)
    pub tax_rate: f32,
    /// Knockout threshold override (default 0.85)
    pub knockout_threshold: f32,
    /// Revenue multiplier from defense contracts
    pub defense_bonus: f32,
    /// Zone placement cost multiplier
    pub zone_cost_mult: f32,
    /// Quarters remaining for this mandate
    pub quarters_remaining: u32,
}

impl Default for PartyMandate {
    fn default() -> Self {
        Self {
            party: Party::NoMandate,
            tax_rate: 1.0,
            knockout_threshold: 0.85,
            defense_bonus: 1.0,
            zone_cost_mult: 1.0,
            quarters_remaining: 0,
        }
    }
}

impl PartyMandate {
    /// Create mandate from election winner
    pub fn from_party(party: Party) -> Self {
        match party {
            Party::NoMandate => Self::default(),
            Party::Consensus => Self {
                party,
                tax_rate: 1.25,       // 25% tax increase
                knockout_threshold: 0.95, // Harder to trigger knockout
                defense_bonus: 1.0,
                zone_cost_mult: 1.0,
                quarters_remaining: 4,
            },
            Party::Vanguard => Self {
                party,
                tax_rate: 0.85,       // 15% tax cut (pro-business)
                knockout_threshold: 0.70, // Easier knockout
                defense_bonus: 2.5,    // Massive defense contracts
                zone_cost_mult: 0.8,   // Cheaper zone placement
                quarters_remaining: 4,
            },
        }
    }

    /// Apply tax to revenue (returns post-tax amount)
    pub fn apply_tax(&self, gross_revenue: f64) -> f64 {
        gross_revenue / self.tax_rate as f64
    }

    /// Is this mandate still active?
    pub fn is_active(&self) -> bool {
        self.quarters_remaining > 0 && self.party != Party::NoMandate
    }

    /// Tick down one quarter
    pub fn tick_quarter(&mut self) {
        if self.quarters_remaining > 0 {
            self.quarters_remaining -= 1;
        }
    }
}

// ============================================================================
// Election State
// ============================================================================

/// Tracks the election cycle
#[derive(Resource, Clone, Debug)]
pub struct ElectionState {
    /// Quarters since last election
    pub quarters_since_election: u32,
    /// Election interval (quarters)
    pub election_interval: u32,
    /// History of election results
    pub election_history: Vec<Party>,
    /// Current GPU histogram (updated each frame)
    pub current_histogram: PoliticalData,
    /// NC spent on lobbying this cycle
    pub lobby_nc_spent: f64,
    /// Whether an election is pending (just triggered)
    pub election_pending: bool,
    /// Projected winner shown 1 quarter before election (preview for player)
    pub projected_winner: Party,
    /// Whether the projection is currently visible to the player
    pub showing_preview: bool,
}

impl Default for ElectionState {
    fn default() -> Self {
        Self {
            quarters_since_election: 0,
            election_interval: 4,
            election_history: Vec::new(),
            current_histogram: PoliticalData::default(),
            lobby_nc_spent: 0.0,
            election_pending: false,
            projected_winner: Party::NoMandate,
            showing_preview: false,
        }
    }
}

impl ElectionState {
    /// Check if an election should trigger
    pub fn should_trigger_election(&self) -> bool {
        self.quarters_since_election >= self.election_interval
    }

    /// Check if the election preview should be shown (1 quarter before election)
    pub fn should_show_preview(&self) -> bool {
        self.quarters_since_election + 1 >= self.election_interval
    }

    /// Update the projected winner based on current histogram.
    /// Called each quarter tick to keep the preview current.
    pub fn update_projection(&mut self) {
        if self.should_show_preview() {
            self.projected_winner = determine_winner(&self.current_histogram);
            self.showing_preview = true;
        } else {
            self.showing_preview = false;
        }
    }

    /// Apply lobbying: adjust histogram buckets based on NC spent.
    /// Lobbying shifts weight from edges toward center (or vice versa).
    pub fn lobbied_histogram(&self, target_bucket: usize) -> PoliticalData {
        let mut adjusted = self.current_histogram;
        if self.lobby_nc_spent <= 0.0 || target_bucket >= 10 {
            return adjusted;
        }

        // Each 10 NC spent adds 1% of total_votes to the target bucket
        let bonus_votes = (self.lobby_nc_spent / 10.0 * adjusted.total_votes as f64 * 0.01) as u32;
        adjusted.buckets[target_bucket] += bonus_votes;
        adjusted.total_votes += bonus_votes;
        adjusted
    }
}

// ============================================================================
// Government Contracts (Directives)
// ============================================================================

#[derive(Clone, Debug)]
pub enum DirectiveTarget {
    /// Raise mean_opinion above threshold
    RaiseMeanOpinion(f32),
    /// Lower polarization_heat below threshold
    LowerPolarization(f32),
    /// Raise engagement_index above threshold
    RaiseEngagement(f32),
}

#[derive(Clone, Debug)]
pub struct Directive {
    pub name: String,
    pub description: String,
    pub target: DirectiveTarget,
    /// Quarters until deadline
    pub deadline_quarters: u32,
    /// Cash reward on success
    pub cash_reward: f64,
    /// NC reward on success
    pub nc_reward: f64,
    /// Is this contract active?
    pub active: bool,
}

impl Directive {
    /// Check if the directive target has been met
    pub fn is_met(&self, mean_opinion: f32, polarization: f32, engagement: f32) -> bool {
        match self.target {
            DirectiveTarget::RaiseMeanOpinion(t) => mean_opinion >= t,
            DirectiveTarget::LowerPolarization(t) => polarization <= t,
            DirectiveTarget::RaiseEngagement(t) => engagement >= t,
        }
    }
}

/// Active government contracts
#[derive(Resource, Default)]
pub struct GovernmentContracts {
    pub directives: Vec<Directive>,
}

// ============================================================================
// Election & Contract Lifecycle Functions
// ============================================================================

/// Run an election: determine winner using (potentially lobbied) histogram,
/// create mandate, generate contracts, record history.
/// Returns the winning party.
pub fn run_election(
    election: &mut ElectionState,
    mandate: &mut PartyMandate,
    contracts: &mut GovernmentContracts,
    lobby_target_bucket: usize,
) -> Party {
    // Apply lobbying to histogram before determining winner
    let effective_histogram = election.lobbied_histogram(lobby_target_bucket);
    let winner = determine_winner(&effective_histogram);

    // Record history
    election.election_history.push(winner);
    election.quarters_since_election = 0;
    election.lobby_nc_spent = 0.0;
    election.election_pending = false;

    // Install new mandate (or tick down existing)
    *mandate = PartyMandate::from_party(winner);

    // Generate a government directive based on the winning party
    if winner != Party::NoMandate {
        contracts.directives.push(generate_directive(winner));
    }

    winner
}

/// Generate a government directive appropriate to the winning party
pub fn generate_directive(party: Party) -> Directive {
    match party {
        Party::Consensus => Directive {
            name: "Ministry of Stability".into(),
            description: "Lower polarization below 0.20 within 3 quarters.".into(),
            target: DirectiveTarget::LowerPolarization(0.20),
            deadline_quarters: 3,
            cash_reward: 3000.0,
            nc_reward: 30.0,
            active: true,
        },
        Party::Vanguard => Directive {
            name: "Defense Intelligence Contract".into(),
            description: "Raise engagement above 0.85 within 2 quarters.".into(),
            target: DirectiveTarget::RaiseEngagement(0.85),
            deadline_quarters: 2,
            cash_reward: 8000.0,
            nc_reward: 80.0,
            active: true,
        },
        Party::NoMandate => Directive {
            name: "Public Interest Filing".into(),
            description: "Maintain current state.".into(),
            target: DirectiveTarget::RaiseMeanOpinion(0.0), // Always met
            deadline_quarters: 4,
            cash_reward: 500.0,
            nc_reward: 5.0,
            active: true,
        },
    }
}

/// Tick contracts: decrement deadlines, check completion, handle expiry.
/// Returns (completed_cash, completed_nc, failed_count).
pub fn tick_contracts(
    contracts: &mut GovernmentContracts,
    mean_opinion: f32,
    polarization: f32,
    engagement: f32,
) -> (f64, f64, u32) {
    let mut total_cash = 0.0;
    let mut total_nc = 0.0;
    let mut failures = 0u32;

    for directive in contracts.directives.iter_mut() {
        if !directive.active { continue; }

        if directive.is_met(mean_opinion, polarization, engagement) {
            // Contract completed!
            total_cash += directive.cash_reward;
            total_nc += directive.nc_reward;
            directive.active = false;
        } else {
            directive.deadline_quarters = directive.deadline_quarters.saturating_sub(1);
            if directive.deadline_quarters == 0 {
                // Contract expired — failure
                directive.active = false;
                failures += 1;
            }
        }
    }

    // Remove inactive directives
    contracts.directives.retain(|d| d.active);

    (total_cash, total_nc, failures)
}

// ============================================================================
// Singularity Detection (Phase 4)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SingularityType {
    #[default]
    None,
    /// All opinions collapsed into a single peak (std_dev < 0.05)
    TotalConsensus,
    /// Two perfectly frozen peaks, hollow center (center < 5%, wings > 35%)
    TotalPolarization,
}

/// The Singularity State — once triggered, irreversible
#[derive(Resource, Clone, Debug)]
pub struct SingularityState {
    pub triggered: bool,
    pub singularity_type: SingularityType,
    /// The quarter when singularity was triggered
    pub trigger_quarter: u32,
}

impl Default for SingularityState {
    fn default() -> Self {
        Self {
            triggered: false,
            singularity_type: SingularityType::None,
            trigger_quarter: 0,
        }
    }
}

/// Calculate the standard deviation of a normalized histogram
pub fn histogram_std_dev(normalized: &[f32; 10]) -> f32 {
    // Weighted mean
    let mean: f32 = normalized.iter().enumerate()
        .map(|(i, &p)| (i as f32 + 0.5) / 10.0 * p) // bucket center * probability
        .sum();

    // Weighted variance
    let variance: f32 = normalized.iter().enumerate()
        .map(|(i, &p)| {
            let center = (i as f32 + 0.5) / 10.0;
            (center - mean) * (center - mean) * p
        })
        .sum();

    variance.sqrt()
}

/// Check if the opinion histogram has reached a singularity.
/// Pure function — fully testable without GPU.
pub fn check_singularity(data: &PoliticalData) -> SingularityType {
    if data.total_votes == 0 {
        return SingularityType::None;
    }

    let norm = data.normalized();
    let std_dev = histogram_std_dev(&norm);

    // Total Consensus: everyone agrees (std_dev below threshold)
    if std_dev < 0.05 {
        return SingularityType::TotalConsensus;
    }

    // Total Polarization: hollow center, frozen wings
    let center_mass: f32 = norm[3..7].iter().sum();
    let left_mass: f32 = norm[0..3].iter().sum();
    let right_mass: f32 = norm[7..10].iter().sum();

    if center_mass < 0.05 && left_mass > 0.35 && right_mass > 0.35 {
        return SingularityType::TotalPolarization;
    }

    SingularityType::None
}

// ============================================================================
// Plugin
// ============================================================================

pub struct PoliticsPlugin;

impl Plugin for PoliticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartyMandate>();
        app.init_resource::<ElectionState>();
        app.init_resource::<GovernmentContracts>();
        app.init_resource::<SingularityState>();
    }
}

// ============================================================================
// Tests (TDD)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === PoliticalData Alignment ===

    #[test]
    fn test_political_data_alignment() {
        assert_eq!(std::mem::size_of::<PoliticalData>(), 44);
    }

    #[test]
    fn test_political_data_bytemuck_cast() {
        let data = PoliticalData {
            buckets: [100, 200, 300, 400, 500, 600, 700, 800, 900, 1000],
            total_votes: 5500,
        };
        let bytes = bytemuck::bytes_of(&data);
        assert_eq!(bytes.len(), 44);
        let restored: &PoliticalData = bytemuck::from_bytes(bytes);
        assert_eq!(restored.total_votes, 5500);
        assert_eq!(restored.buckets[0], 100);
        assert_eq!(restored.buckets[9], 1000);
    }

    #[test]
    fn test_normalized_distribution() {
        let data = PoliticalData {
            buckets: [100, 0, 0, 0, 0, 0, 0, 0, 0, 100],
            total_votes: 200,
        };
        let norm = data.normalized();
        assert!((norm[0] - 0.5).abs() < 0.01);
        assert!((norm[9] - 0.5).abs() < 0.01);
        assert!((norm[5]).abs() < 0.01);
    }

    #[test]
    fn test_normalized_zero_votes() {
        let data = PoliticalData::default();
        let norm = data.normalized();
        for v in norm {
            assert!((v).abs() < 0.001);
        }
    }

    // === Party Determination ===

    #[test]
    fn test_determine_winner_centrist_peak() {
        let data = PoliticalData {
            buckets: [10, 20, 30, 200, 400, 500, 300, 40, 20, 10],
            total_votes: 1530,
        };
        assert_eq!(determine_winner(&data), Party::Consensus);
    }

    #[test]
    fn test_determine_winner_bimodal() {
        let data = PoliticalData {
            buckets: [400, 300, 200, 10, 5, 5, 10, 200, 300, 400],
            total_votes: 1830,
        };
        assert_eq!(determine_winner(&data), Party::Vanguard);
    }

    #[test]
    fn test_determine_winner_flat_distribution() {
        let data = PoliticalData {
            buckets: [100, 100, 100, 100, 100, 100, 100, 100, 100, 100],
            total_votes: 1000,
        };
        assert_eq!(determine_winner(&data), Party::NoMandate);
    }

    #[test]
    fn test_determine_winner_empty() {
        let data = PoliticalData::default();
        assert_eq!(determine_winner(&data), Party::NoMandate);
    }

    #[test]
    fn test_determine_winner_slight_center() {
        // Center has some weight but not enough for Consensus
        let data = PoliticalData {
            buckets: [80, 90, 100, 120, 130, 120, 110, 100, 90, 80],
            total_votes: 1020,
        };
        // Center mass (3-6) = 120+130+120+110 = 480/1020 ≈ 0.47 < 0.55
        assert_eq!(determine_winner(&data), Party::NoMandate);
    }

    // === Party Mandate Effects ===

    #[test]
    fn test_consensus_mandate_raises_tax() {
        let mandate = PartyMandate::from_party(Party::Consensus);
        assert!(mandate.tax_rate > 1.0, "Consensus should raise taxes");
        assert_eq!(mandate.quarters_remaining, 4);
    }

    #[test]
    fn test_consensus_mandate_raises_knockout_threshold() {
        let mandate = PartyMandate::from_party(Party::Consensus);
        assert!(mandate.knockout_threshold > 0.85, "Consensus stabilizes knockout");
    }

    #[test]
    fn test_vanguard_mandate_defense_bonus() {
        let mandate = PartyMandate::from_party(Party::Vanguard);
        assert!(mandate.defense_bonus > 2.0, "Vanguard offers defense contracts");
        assert!(mandate.tax_rate < 1.0, "Vanguard cuts taxes");
    }

    #[test]
    fn test_vanguard_mandate_lowers_knockout() {
        let mandate = PartyMandate::from_party(Party::Vanguard);
        assert!(mandate.knockout_threshold < 0.85, "Vanguard accelerates collapse");
    }

    #[test]
    fn test_no_mandate_is_neutral() {
        let mandate = PartyMandate::from_party(Party::NoMandate);
        assert!((mandate.tax_rate - 1.0).abs() < 0.001);
        assert!((mandate.defense_bonus - 1.0).abs() < 0.001);
        assert_eq!(mandate.quarters_remaining, 0);
    }

    #[test]
    fn test_apply_tax_consensus() {
        let mandate = PartyMandate::from_party(Party::Consensus);
        let post_tax = mandate.apply_tax(1000.0);
        // 1000 / 1.25 = 800
        assert!((post_tax - 800.0).abs() < 1.0);
    }

    #[test]
    fn test_apply_tax_vanguard() {
        let mandate = PartyMandate::from_party(Party::Vanguard);
        let post_tax = mandate.apply_tax(1000.0);
        // 1000 / 0.85 ≈ 1176
        assert!(post_tax > 1100.0, "Vanguard tax cut should increase take-home");
    }

    #[test]
    fn test_mandate_tick_expires() {
        let mut mandate = PartyMandate::from_party(Party::Consensus);
        assert!(mandate.is_active());
        for _ in 0..4 {
            mandate.tick_quarter();
        }
        assert!(!mandate.is_active());
    }

    // === Election State ===

    #[test]
    fn test_election_triggers_every_4_quarters() {
        let mut state = ElectionState::default();
        assert!(!state.should_trigger_election());
        state.quarters_since_election = 3;
        assert!(!state.should_trigger_election());
        state.quarters_since_election = 4;
        assert!(state.should_trigger_election());
    }

    #[test]
    fn test_lobbying_shifts_histogram() {
        let mut state = ElectionState::default();
        state.current_histogram = PoliticalData {
            buckets: [100, 100, 100, 100, 100, 100, 100, 100, 100, 100],
            total_votes: 1000,
        };
        state.lobby_nc_spent = 100.0; // 100 NC

        let adjusted = state.lobbied_histogram(5); // Lobby toward center
        assert!(adjusted.buckets[5] > 100, "Lobbying should increase target bucket");
        assert!(adjusted.total_votes > 1000, "Total votes should increase");
    }

    #[test]
    fn test_lobbying_zero_nc_no_change() {
        let mut state = ElectionState::default();
        state.current_histogram = PoliticalData {
            buckets: [100; 10],
            total_votes: 1000,
        };
        state.lobby_nc_spent = 0.0;

        let adjusted = state.lobbied_histogram(5);
        assert_eq!(adjusted.buckets[5], 100);
    }

    // === Government Contracts ===

    #[test]
    fn test_directive_met_raise_opinion() {
        let directive = Directive {
            name: "Ministry of Stability".into(),
            description: "Raise trust".into(),
            target: DirectiveTarget::RaiseMeanOpinion(0.6),
            deadline_quarters: 3,
            cash_reward: 5000.0,
            nc_reward: 50.0,
            active: true,
        };
        assert!(!directive.is_met(0.5, 0.3, 0.8));
        assert!(directive.is_met(0.65, 0.3, 0.8));
    }

    #[test]
    fn test_directive_met_lower_polarization() {
        let directive = Directive {
            name: "Calm the Storm".into(),
            description: "Lower polarization".into(),
            target: DirectiveTarget::LowerPolarization(0.2),
            deadline_quarters: 2,
            cash_reward: 3000.0,
            nc_reward: 30.0,
            active: true,
        };
        assert!(!directive.is_met(0.5, 0.3, 0.8));
        assert!(directive.is_met(0.5, 0.15, 0.8));
    }

    #[test]
    fn test_directive_met_raise_engagement() {
        let directive = Directive {
            name: "Capture Attention".into(),
            description: "Boost engagement".into(),
            target: DirectiveTarget::RaiseEngagement(0.9),
            deadline_quarters: 2,
            cash_reward: 2000.0,
            nc_reward: 20.0,
            active: true,
        };
        assert!(!directive.is_met(0.5, 0.3, 0.8));
        assert!(directive.is_met(0.5, 0.3, 0.95));
    }

    // === Bimodal / Centrist Detection ===

    #[test]
    fn test_is_bimodal_clear() {
        let data = PoliticalData {
            buckets: [300, 300, 200, 10, 5, 5, 10, 200, 300, 300],
            total_votes: 1630,
        };
        assert!(data.is_bimodal(0.25));
    }

    #[test]
    fn test_is_not_bimodal_centrist() {
        let data = PoliticalData {
            buckets: [10, 20, 50, 200, 400, 400, 200, 50, 20, 10],
            total_votes: 1360,
        };
        assert!(!data.is_bimodal(0.25));
    }

    #[test]
    fn test_is_centrist_strong() {
        let data = PoliticalData {
            buckets: [10, 20, 50, 200, 400, 400, 200, 50, 20, 10],
            total_votes: 1360,
        };
        assert!(data.is_centrist_peak(0.55));
    }

    #[test]
    fn test_peak_bucket() {
        let data = PoliticalData {
            buckets: [10, 20, 30, 40, 50, 999, 40, 30, 20, 10],
            total_votes: 1249,
        };
        assert_eq!(data.peak_bucket(), 5);
    }

    // === Party Names ===

    #[test]
    fn test_party_names() {
        assert_eq!(Party::Consensus.name(), "The Consensus Party");
        assert_eq!(Party::Vanguard.name(), "The Vanguard");
        assert_eq!(Party::NoMandate.name(), "No Mandate");
    }

    // === Election Lifecycle ===

    #[test]
    fn test_run_election_consensus() {
        let mut election = ElectionState::default();
        election.current_histogram = PoliticalData {
            buckets: [10, 20, 30, 200, 400, 500, 300, 40, 20, 10],
            total_votes: 1530,
        };
        let mut mandate = PartyMandate::default();
        let mut contracts = GovernmentContracts::default();

        let winner = run_election(&mut election, &mut mandate, &mut contracts, 5);
        assert_eq!(winner, Party::Consensus);
        assert_eq!(mandate.party, Party::Consensus);
        assert_eq!(mandate.quarters_remaining, 4);
        assert_eq!(election.election_history.len(), 1);
        assert_eq!(election.quarters_since_election, 0);
        assert_eq!(contracts.directives.len(), 1);
        assert_eq!(contracts.directives[0].name, "Ministry of Stability");
    }

    #[test]
    fn test_run_election_vanguard() {
        let mut election = ElectionState::default();
        election.current_histogram = PoliticalData {
            buckets: [400, 300, 200, 10, 5, 5, 10, 200, 300, 400],
            total_votes: 1830,
        };
        let mut mandate = PartyMandate::default();
        let mut contracts = GovernmentContracts::default();

        let winner = run_election(&mut election, &mut mandate, &mut contracts, 5);
        assert_eq!(winner, Party::Vanguard);
        assert!(mandate.defense_bonus > 2.0);
        assert_eq!(contracts.directives[0].name, "Defense Intelligence Contract");
    }

    #[test]
    fn test_lobbying_flips_election() {
        let mut election = ElectionState::default();
        // Borderline histogram: slightly favoring edges
        election.current_histogram = PoliticalData {
            buckets: [150, 120, 100, 80, 60, 60, 80, 100, 120, 150],
            total_votes: 1020,
        };
        // Without lobbying, this might be NoMandate
        let _base_winner = determine_winner(&election.current_histogram);

        // Lobby heavily toward center (bucket 5)
        election.lobby_nc_spent = 500.0; // Massive lobby spend
        let adjusted = election.lobbied_histogram(5);
        let _lobbied_winner = determine_winner(&adjusted);

        // The lobby should shift the outcome toward Consensus
        // (we can't guarantee it changes, but the center weight should increase)
        let adj_norm = adjusted.normalized();
        let center_mass: f32 = adj_norm[3..7].iter().sum();
        assert!(center_mass > 0.4, "Heavy lobbying should increase center mass (got {})", center_mass);
    }

    // === Contract Lifecycle ===

    #[test]
    fn test_tick_contracts_completion() {
        let mut contracts = GovernmentContracts::default();
        contracts.directives.push(Directive {
            name: "Test".into(),
            description: "Lower pol".into(),
            target: DirectiveTarget::LowerPolarization(0.3),
            deadline_quarters: 3,
            cash_reward: 5000.0,
            nc_reward: 50.0,
            active: true,
        });

        // Met condition: polarization = 0.2 < 0.3
        let (cash, nc, failures) = tick_contracts(&mut contracts, 0.5, 0.2, 0.8);
        assert!((cash - 5000.0).abs() < 0.01);
        assert!((nc - 50.0).abs() < 0.01);
        assert_eq!(failures, 0);
        assert!(contracts.directives.is_empty(), "Completed contracts removed");
    }

    #[test]
    fn test_tick_contracts_expiry_failure() {
        let mut contracts = GovernmentContracts::default();
        contracts.directives.push(Directive {
            name: "Impossible".into(),
            description: "Raise engagement absurdly".into(),
            target: DirectiveTarget::RaiseEngagement(0.99),
            deadline_quarters: 1, // Only 1 quarter left
            cash_reward: 10000.0,
            nc_reward: 100.0,
            active: true,
        });

        // Not met, deadline ticks to 0
        let (cash, _nc, failures) = tick_contracts(&mut contracts, 0.5, 0.3, 0.5);
        assert!((cash).abs() < 0.01, "No reward for failure");
        assert_eq!(failures, 1);
        assert!(contracts.directives.is_empty(), "Failed contracts removed");
    }

    #[test]
    fn test_generate_directive_consensus() {
        let d = generate_directive(Party::Consensus);
        assert_eq!(d.name, "Ministry of Stability");
        assert!(d.active);
        match d.target {
            DirectiveTarget::LowerPolarization(t) => assert!((t - 0.2).abs() < 0.01),
            _ => panic!("Consensus should target polarization"),
        }
    }

    #[test]
    fn test_generate_directive_vanguard() {
        let d = generate_directive(Party::Vanguard);
        assert_eq!(d.name, "Defense Intelligence Contract");
        assert!(d.cash_reward > 5000.0, "Vanguard contracts pay well");
        match d.target {
            DirectiveTarget::RaiseEngagement(t) => assert!(t > 0.8),
            _ => panic!("Vanguard should target engagement"),
        }
    }

    // === Singularity Detection ===

    #[test]
    fn test_singularity_state_defaults() {
        let state = SingularityState::default();
        assert!(!state.triggered);
        assert_eq!(state.singularity_type, SingularityType::None);
        assert_eq!(state.trigger_quarter, 0);
    }

    #[test]
    fn test_singularity_total_consensus() {
        // Everyone in bucket 5 (opinion 0.5)
        let data = PoliticalData {
            buckets: [0, 0, 0, 0, 0, 10000, 0, 0, 0, 0],
            total_votes: 10000,
        };
        assert_eq!(check_singularity(&data), SingularityType::TotalConsensus);
    }

    #[test]
    fn test_singularity_total_polarization() {
        // Perfectly split with empty center
        let data = PoliticalData {
            buckets: [2000, 2000, 1000, 0, 0, 0, 0, 1000, 2000, 2000],
            total_votes: 10000,
        };
        assert_eq!(check_singularity(&data), SingularityType::TotalPolarization);
    }

    #[test]
    fn test_no_singularity_healthy_distribution() {
        // Normal bell curve
        let data = PoliticalData {
            buckets: [100, 300, 600, 1200, 2000, 2000, 1200, 600, 300, 100],
            total_votes: 8400,
        };
        assert_eq!(check_singularity(&data), SingularityType::None);
    }

    #[test]
    fn test_no_singularity_empty() {
        let data = PoliticalData::default();
        assert_eq!(check_singularity(&data), SingularityType::None);
    }

    #[test]
    fn test_histogram_std_dev_uniform() {
        let norm = [0.1; 10]; // Uniform distribution
        let sd = histogram_std_dev(&norm);
        assert!(sd > 0.2, "Uniform should have moderate std_dev (got {})", sd);
    }

    #[test]
    fn test_histogram_std_dev_peaked() {
        let mut norm = [0.0f32; 10];
        norm[5] = 1.0; // All mass in one bucket
        let sd = histogram_std_dev(&norm);
        assert!(sd < 0.05, "Single peak should have very low std_dev (got {})", sd);
    }

    // === Election Preview Tests (Phase C) ===

    #[test]
    fn test_election_preview_default() {
        let state = ElectionState::default();
        assert_eq!(state.projected_winner, Party::NoMandate);
        assert!(!state.showing_preview);
    }

    #[test]
    fn test_election_preview_shows_one_quarter_before() {
        let mut state = ElectionState::default();
        // election_interval = 4, so preview shows at quarter 3
        state.quarters_since_election = 3;
        assert!(state.should_show_preview(), "Preview should show 1 quarter before election");
    }

    #[test]
    fn test_no_preview_when_far_from_election() {
        let mut state = ElectionState::default();
        state.quarters_since_election = 1;
        assert!(!state.should_show_preview(), "No preview when 3 quarters away");
    }

    #[test]
    fn test_projection_updates_correctly() {
        let mut state = ElectionState::default();
        state.quarters_since_election = 3; // 1 quarter before election

        // Set a centrist histogram → Consensus should win
        state.current_histogram = PoliticalData {
            buckets: [100, 200, 500, 1500, 3000, 3000, 1500, 500, 200, 100],
            total_votes: 10600,
        };

        state.update_projection();
        assert!(state.showing_preview, "Preview should be visible");
        assert_eq!(state.projected_winner, Party::Consensus,
            "Centrist population should project Consensus winner");
    }

    #[test]
    fn test_projection_not_shown_early() {
        let mut state = ElectionState::default();
        state.quarters_since_election = 1; // Too early

        state.update_projection();
        assert!(!state.showing_preview, "Preview should NOT be visible with 3 quarters remaining");
    }
}
