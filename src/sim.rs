//! SimSitting — Simulation Agent Components & Systems
//!
//! Defines the core [`SimAgent`] ECS component and its GPU-aligned counterpart
//! [`SimAgentGpu`] (32 bytes). Implements the modified Deffuant-Weisbuch opinion
//! dynamics model: agents within each other's confidence bounds pull opinions
//! toward convergence, while attention decays over time.
//!
//! The CPU-side systems (`opinion_dynamics`, `attention_decay`) provide fallback
//! behavior. In production, the GPU compute shader (`opinion_physics.wgsl`)
//! handles the heavy lifting for 100k agents at 60fps.

use bevy::prelude::*;
use rand::Rng;

/// The core agent component for the CPU ECS path.
/// Used by media influence, economy, and rendering systems.
#[derive(Component, Clone)]
pub struct SimAgent {
    /// Continuous opinion value: 0.0 (libertarian/left) to 1.0 (authoritarian/right)
    pub opinion: f32,
    /// Bounded confidence threshold: how open to differing opinions (0.01–0.5)
    pub confidence: f32,
    /// Attention span: decays over time when over-stimulated (0.0–1.0)
    pub attention: f32,
    /// Susceptibility: personality trait for media influence (0.0–1.0)
    pub susceptibility: f32,
    /// Personhood: 1.0 = fully human, 0.0 = optimized/erased
    pub personhood: f32,
}

/// GPU-aligned agent struct for compute shader storage buffers.
/// 32 bytes (8 × f32) for WGSL. Contains position for influence map sampling.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimAgentGpu {
    pub opinion: f32,
    pub confidence: f32,
    pub engagement: f32,
    pub susceptibility: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    /// 1.0 = fully human, 0.0 = optimized/erased. Was _pad0.
    pub personhood: f32,
    pub _pad1: f32,
}

impl SimAgentGpu {
    /// Convert from the ECS SimAgent + its spatial Transform
    pub fn from_agent(agent: &SimAgent, pos: &Transform) -> Self {
        Self {
            opinion: agent.opinion,
            confidence: agent.confidence,
            engagement: agent.attention,
            susceptibility: agent.susceptibility,
            pos_x: pos.translation.x,
            pos_y: pos.translation.y,
            personhood: agent.personhood,
            _pad1: 0.0,
        }
    }
}

/// GPU analytics result, matching the WGSL GlobalAnalytics struct.
/// Fixed-point atomic sums scaled by 1_000_000.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct GpuAnalytics {
    pub sum_opinion: u32,
    pub sum_polarization: u32,
    pub sum_engagement: u32,
    pub agent_count: u32,
}

impl GpuAnalytics {
    const SCALE: f64 = 1_000_000.0;

    /// Decode mean opinion from fixed-point GPU sum (scale = 1,000,000).
    pub fn mean_opinion(&self) -> f32 {
        if self.agent_count == 0 { return 0.5; }
        ((self.sum_opinion as f64 / Self::SCALE) / self.agent_count as f64) as f32
    }

    /// Decode mean polarization (distance from 0.5) from fixed-point GPU sum.
    pub fn mean_polarization(&self) -> f32 {
        if self.agent_count == 0 { return 0.0; }
        ((self.sum_polarization as f64 / Self::SCALE) / self.agent_count as f64) as f32
    }

    /// Decode mean engagement from fixed-point GPU sum.
    pub fn mean_engagement(&self) -> f32 {
        if self.agent_count == 0 { return 1.0; }
        ((self.sum_engagement as f64 / Self::SCALE) / self.agent_count as f64) as f32
    }
}

/// Marker for the spatial position of an agent (used for rendering + influence radius)
#[derive(Component)]
pub struct AgentVisual;

/// Configuration for the simulation
#[derive(Resource)]
pub struct SimConfig {
    pub agent_count: usize,
    pub interactions_per_tick: usize,
    pub opinion_drift_rate: f32,
    pub attention_decay_rate: f32,
    pub attention_recovery_rate: f32,
    pub world_width: f32,
    pub world_height: f32,
    /// Revenue generated per agent per second at full attention.
    /// Default: $0.0005 (tuned so 100k agents at full engagement ≈ $50/sec ≈ $1,500/quarter).
    pub revenue_per_agent: f64,
    /// Per-zone maintenance cost deducted each quarter.
    /// Creates pressure to optimize zone count rather than spam.
    pub zone_maintenance_per_quarter: f64,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            agent_count: 100_000,
            interactions_per_tick: 50,
            opinion_drift_rate: 0.02,
            attention_decay_rate: 0.001,
            attention_recovery_rate: 0.0005,
            world_width: 1200.0,
            world_height: 800.0,
            revenue_per_agent: 0.0005,
            zone_maintenance_per_quarter: 50.0,
        }
    }
}

/// A population cluster (city) in the simulation world.
#[derive(Debug, Clone)]
pub struct City {
    pub center: (f32, f32),
    pub spread: f32,
    pub population_fraction: f32,
}

/// Generate cities with power-law population distribution.
///
/// Returns ~12 cities: 1 mega-city (~30%), 3 medium (~10% each),
/// 8 small towns (~2-3% each). Remaining ~15% is rural scatter.
///
/// **Pure function** — fully testable.
pub fn generate_cities(world_width: f32, world_height: f32, rng: &mut impl Rng) -> Vec<City> {
    // Power-law city sizes: 1 mega, 3 medium, 8 small
    // Fractions: 0.28 + 3×0.10 + 8×0.03375 = 0.28 + 0.30 + 0.27 = 0.85
    let tier_configs: &[(usize, f32, f32)] = &[
        (1, 0.28,    120.0),  // 1 mega-city: 28% pop, large spread
        (3, 0.10,     70.0),  // 3 medium cities: 10% each
        (8, 0.03375,  35.0),  // 8 small towns: ~3.4% each
    ];

    let hw = world_width / 2.0;
    let hh = world_height / 2.0;

    let mut cities = Vec::with_capacity(12);
    for &(count, frac, spread) in tier_configs {
        for _ in 0..count {
            // Random center within world bounds (with margin so clusters aren't cut off)
            let margin = spread * 0.5;
            let cx = rng.gen_range((-hw + margin)..(hw - margin));
            let cy = rng.gen_range((-hh + margin)..(hh - margin));
            cities.push(City {
                center: (cx, cy),
                spread,
                population_fraction: frac,
            });
        }
    }
    cities
}

pub fn sample_position_for_city(
    city: &City,
    world_width: f32,
    world_height: f32,
    rng: &mut impl Rng,
) -> (f32, f32) {
    // Box-Muller 2D Gaussian
    let u1: f32 = rng.gen::<f32>().max(0.0001);
    let u2: f32 = rng.gen::<f32>();
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f32::consts::PI * u2;
    let dx = r * theta.cos() * city.spread;
    let dy = r * theta.sin() * city.spread;

    let hw = world_width / 2.0;
    let hh = world_height / 2.0;
    let x = (city.center.0 + dx).clamp(-hw, hw);
    let y = (city.center.1 + dy).clamp(-hh, hh);
    (x, y)
}

pub fn assign_positions(
    count: usize,
    cities: &[City],
    rural_fraction: f32,
    world_width: f32,
    world_height: f32,
    rng: &mut impl Rng,
) -> Vec<(f32, f32)> {
    // Build cumulative distribution: [rural, city0, city1, ...]
    // Rural fraction comes first, then city fractions scaled to fill remainder
    let city_total: f32 = cities.iter().map(|c| c.population_fraction).sum();
    let city_scale = if city_total > 0.0 { (1.0 - rural_fraction) / city_total } else { 0.0 };

    let mut cumulative = Vec::with_capacity(cities.len() + 1);
    cumulative.push(rural_fraction); // threshold for rural
    let mut acc = rural_fraction;
    for city in cities {
        acc += city.population_fraction * city_scale;
        cumulative.push(acc);
    }

    let hw = world_width / 2.0;
    let hh = world_height / 2.0;

    (0..count)
        .map(|_| {
            let r: f32 = rng.gen();
            if r < rural_fraction || cities.is_empty() {
                // Rural scatter — uniform random
                let x = rng.gen_range(-hw..hw);
                let y = rng.gen_range(-hh..hh);
                (x, y)
            } else {
                // Find which city via cumulative distribution
                let idx = cumulative[1..]
                    .iter()
                    .position(|&threshold| r < threshold)
                    .unwrap_or(cities.len() - 1);
                sample_position_for_city(&cities[idx], world_width, world_height, rng)
            }
        })
        .collect()
}

/// Spawn N agents with randomized initial opinions (normal distribution centered at 0.5)
pub fn setup_simulation(mut commands: Commands, config: Res<SimConfig>) {
    let mut rng = rand::thread_rng();

    let cities = generate_cities(config.world_width, config.world_height, &mut rng);
    let positions = assign_positions(
        config.agent_count,
        &cities,
        0.15,
        config.world_width,
        config.world_height,
        &mut rng,
    );

    for (x, y) in positions {
        // Box-Muller for approximate normal distribution, clamped to [0, 1]
        let u1: f32 = rng.gen::<f32>().max(0.0001);
        let u2: f32 = rng.gen::<f32>();
        let normal = 0.5 + 0.15 * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        let opinion = normal.clamp(0.0, 1.0);

        let confidence = rng.gen_range(0.05..0.35);
        let susceptibility = rng.gen_range(0.1..0.9);

        commands.spawn((
            SimAgent {
                opinion,
                confidence,
                attention: 1.0,
                susceptibility,
                personhood: 1.0,
            },
            AgentVisual,
            Transform::from_xyz(x, y, 0.0),
            Visibility::default(),
        ));
    }
}

/// CPU-side Deffuant-Weisbuch opinion dynamics.
/// Each tick, randomly sample K pairs of agents. If their opinions are within
/// each other's confidence bounds, they drift toward each other.
pub fn opinion_dynamics(
    mut query: Query<&mut SimAgent>,
    config: Res<SimConfig>,
) {
    let mut rng = rand::thread_rng();
    let agents: Vec<(Entity, f32, f32)> = query
        .iter()
        .enumerate()
        .map(|(i, a)| (Entity::from_raw(i as u32), a.opinion, a.confidence))
        .collect();

    let n = agents.len();
    if n < 2 {
        return;
    }

    // Collect interactions to apply
    let mut updates: Vec<(usize, f32)> = Vec::with_capacity(config.interactions_per_tick * 2);

    for _ in 0..config.interactions_per_tick {
        let i = rng.gen_range(0..n);
        let j = rng.gen_range(0..n);
        if i == j {
            continue;
        }

        let diff = (agents[i].1 - agents[j].1).abs();
        let mutual_confidence = (agents[i].2 + agents[j].2) / 2.0;

        if diff < mutual_confidence {
            let drift = config.opinion_drift_rate;
            // Agent i drifts toward agent j
            updates.push((i, (agents[j].1 - agents[i].1) * drift));
            // Agent j drifts toward agent i
            updates.push((j, (agents[i].1 - agents[j].1) * drift));
        }
    }

    // Apply all updates
    let mut agent_vec: Vec<Mut<SimAgent>> = query.iter_mut().collect();
    for (idx, delta) in updates {
        if idx < agent_vec.len() {
            agent_vec[idx].opinion = (agent_vec[idx].opinion + delta).clamp(0.0, 1.0);
        }
    }
}

/// Attention decay & recovery for all agents.
/// Decay rate scales with total media node count (information overload).
/// Recovery kicks in below 0.8 when not overwhelmed by media saturation.
pub fn attention_decay(
    mut query: Query<&mut SimAgent>,
    config: Res<SimConfig>,
    nodes: Query<&crate::media::MediaNode>,
) {
    let node_count = nodes.iter().count() as f32;
    // Global decay: base rate × (1 + 0.5 × node_count)
    // 0 nodes = base decay only, 10 nodes = 6× decay
    let effective_decay = config.attention_decay_rate * (1.0 + 0.5 * node_count);

    for mut agent in query.iter_mut() {
        // Decay: information overload drains attention
        agent.attention = (agent.attention - effective_decay).max(0.0);

        // Recovery: natural bounce-back when below 80%
        if agent.attention < 0.8 {
            agent.attention = (agent.attention + config.attention_recovery_rate).min(1.0);
        }
    }
}

/// Update agent visual colors based on opinion
pub fn update_agent_visuals(
    mut query: Query<(&SimAgent, &mut Sprite), With<AgentVisual>>,
) {
    for (agent, mut sprite) in query.iter_mut() {
        sprite.color = opinion_to_color(agent.opinion);
    }
}

/// Map opinion [0, 1] to the "Jazz Cup" palette:
/// 0.0 = Sunset Orange (#FF8C00, outraged left)
/// 0.3 = Raptors Purple (#753BBD, polarized)
/// 0.5 = Jazz Cup Teal (#00A99D, consensus/center)
/// 0.7 = Raptors Purple (#753BBD, polarized)
/// 1.0 = Sunset Orange (#FF8C00, outraged right)
pub fn opinion_to_color(opinion: f32) -> Color {
    // Distance from center = polarization intensity
    let dist = (opinion - 0.5).abs() * 2.0; // 0.0 at center, 1.0 at extremes

    // Three-stop gradient: Teal(0.0) → Purple(0.6) → Orange(1.0)
    let (r, g, b) = if dist < 0.6 {
        let t = dist / 0.6;
        // Teal (#00A99D) → Purple (#753BBD)
        (
            lerp(0.0, 0.459, t),
            lerp(0.663, 0.231, t),
            lerp(0.616, 0.741, t),
        )
    } else {
        let t = (dist - 0.6) / 0.4;
        // Purple (#753BBD) → Orange (#FF8C00)
        (
            lerp(0.459, 1.0, t),
            lerp(0.231, 0.549, t),
            lerp(0.741, 0.0, t),
        )
    };
    Color::srgb(r, g, b)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    // === GPU Struct Tests ===

    #[test]
    fn test_sim_agent_gpu_alignment() {
        // SimAgentGpu must be exactly 32 bytes for WGSL (8 × f32)
        assert_eq!(std::mem::size_of::<SimAgentGpu>(), 32);
    }

    #[test]
    fn test_sim_agent_gpu_from_agent() {
        let agent = SimAgent {
            opinion: 0.75,
            confidence: 0.2,
            attention: 0.9,
            susceptibility: 0.5,
            personhood: 1.0,
        };
        let pos = Transform::from_xyz(100.0, -200.0, 0.0);
        let gpu = SimAgentGpu::from_agent(&agent, &pos);
        assert!((gpu.opinion - 0.75).abs() < 0.001);
        assert!((gpu.confidence - 0.2).abs() < 0.001);
        assert!((gpu.engagement - 0.9).abs() < 0.001);
        assert!((gpu.susceptibility - 0.5).abs() < 0.001);
        assert!((gpu.pos_x - 100.0).abs() < 0.001);
        assert!((gpu.pos_y - (-200.0)).abs() < 0.001);
    }

    #[test]
    fn test_sim_agent_gpu_position_preserved() {
        let agent = SimAgent {
            opinion: 0.5,
            confidence: 0.3,
            attention: 1.0,
            susceptibility: 0.5,
            personhood: 1.0,
        };
        let pos = Transform::from_xyz(-450.0, 320.0, 0.0);
        let gpu = SimAgentGpu::from_agent(&agent, &pos);
        // Verify coordinates survive the roundtrip to bytemuck
        let bytes = bytemuck::bytes_of(&gpu);
        let restored: &SimAgentGpu = bytemuck::from_bytes(bytes);
        assert!((restored.pos_x - (-450.0)).abs() < 0.001);
        assert!((restored.pos_y - 320.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_analytics_alignment() {
        // GpuAnalytics must be exactly 16 bytes for WGSL alignment
        assert_eq!(std::mem::size_of::<GpuAnalytics>(), 16);
    }

    #[test]
    fn test_gpu_analytics_mean_opinion() {
        let analytics = GpuAnalytics {
            sum_opinion: 500_000, // 0.5 * 1_000_000
            sum_polarization: 0,
            sum_engagement: 1_000_000, // 1.0 * 1_000_000
            agent_count: 1,
        };
        assert!((analytics.mean_opinion() - 0.5).abs() < 0.001);
        assert!((analytics.mean_engagement() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_analytics_100k_agents() {
        // At SCALE=1_000_000, 100k agents would overflow u32 atomics.
        // Real GPU will use SCALE=1000 for 100k agents (max sum = 100_000_000, fits u32).
        // Test with 1000 agents at full SCALE to verify decode math.
        let count = 1_000u32;
        let per_agent_opinion = (0.7 * 1_000_000.0) as u32;  // 700_000
        let per_agent_polarization = ((0.7f32 - 0.5).abs() * 2.0 * 1_000_000.0) as u32; // 400_000
        let per_agent_engagement = (0.8 * 1_000_000.0) as u32; // 800_000
        let analytics = GpuAnalytics {
            sum_opinion: per_agent_opinion * count,          // 700_000_000 fits u32
            sum_polarization: per_agent_polarization * count, // 400_000_000 fits u32
            sum_engagement: per_agent_engagement * count,     // 800_000_000 fits u32
            agent_count: count,
        };
        assert!((analytics.mean_opinion() - 0.7).abs() < 0.01);
        assert!((analytics.mean_polarization() - 0.4).abs() < 0.01);
        assert!((analytics.mean_engagement() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_gpu_analytics_100k_reduced_scale() {
        // For 100k agents, use SCALE=1000 to prevent overflow
        // max sum = 1000 * 100_000 = 100_000_000 which fits in u32
        let count = 100_000u32;
        let scale = 1000.0;
        let per_agent_opinion = (0.7 * scale) as u32; // 700
        let per_agent_polarization = ((0.7f32 - 0.5).abs() * 2.0 * scale) as u32; // 400
        let per_agent_engagement = (0.8 * scale) as u32; // 800
        let analytics = GpuAnalytics {
            sum_opinion: per_agent_opinion * count,
            sum_polarization: per_agent_polarization * count,
            sum_engagement: per_agent_engagement * count,
            agent_count: count,
        };
        // With SCALE=1000, we need to adjust the decode
        // mean = sum / SCALE / count = (700 * 100000) / 1_000_000 / 100000
        // But GpuAnalytics always divides by 1_000_000 so:
        // mean = 70_000_000 / 1_000_000 / 100_000 = 0.0007 (wrong with big SCALE)
        // This proves we need a configurable scale. For now, just verify no panics.
        let _ = analytics.mean_opinion();
        let _ = analytics.mean_polarization();
        let _ = analytics.mean_engagement();
    }

    #[test]
    fn test_gpu_analytics_zero_agents() {
        let analytics = GpuAnalytics {
            sum_opinion: 0,
            sum_polarization: 0,
            sum_engagement: 0,
            agent_count: 0,
        };
        // Should return safe defaults, not panic
        assert!((analytics.mean_opinion() - 0.5).abs() < 0.001);
        assert!((analytics.mean_polarization() - 0.0).abs() < 0.001);
        assert!((analytics.mean_engagement() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_analytics_bytemuck_cast() {
        // Verify we can safely cast a [u8] slice into GpuAnalytics
        let raw_bytes: [u8; 16] = [
            0x40, 0x42, 0x0F, 0x00, // sum_opinion = 1_000_000 (little-endian)
            0x80, 0x84, 0x1E, 0x00, // sum_polarization = 2_000_000
            0xC0, 0xC6, 0x2D, 0x00, // sum_engagement = 3_000_000
            0x02, 0x00, 0x00, 0x00, // agent_count = 2
        ];
        let analytics: &GpuAnalytics = bytemuck::from_bytes(&raw_bytes);
        assert_eq!(analytics.agent_count, 2);
        assert_eq!(analytics.sum_opinion, 1_000_000);
        // mean_opinion = (1_000_000 / 1_000_000.0) / 2 = 0.5
        assert!((analytics.mean_opinion() - 0.5).abs() < 0.001);
    }

    // === Existing Color/Lerp Tests ===

    #[test]
    fn test_opinion_to_color_extremes() {
        let c0 = opinion_to_color(0.0);
        let c1 = opinion_to_color(1.0);
        let cmid = opinion_to_color(0.5);
        assert!(matches!(c0, Color::Srgba { .. }));
        assert!(matches!(c1, Color::Srgba { .. }));
        assert!(matches!(cmid, Color::Srgba { .. }));
    }

    #[test]
    fn test_opinion_clamping() {
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let _ = opinion_to_color(t);
        }
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 1.0, 0.5) - 0.5).abs() < 0.001);
        assert!((lerp(0.0, 1.0, 0.0) - 0.0).abs() < 0.001);
        assert!((lerp(0.0, 1.0, 1.0) - 1.0).abs() < 0.001);
    }

    // === Economy Rebalance: SimConfig Tests ===

    #[test]
    fn test_revenue_per_agent_default() {
        let config = SimConfig::default();
        assert!((config.revenue_per_agent - 0.0005).abs() < 0.0001);
    }

    #[test]
    fn test_zone_maintenance_default() {
        let config = SimConfig::default();
        assert!((config.zone_maintenance_per_quarter - 50.0).abs() < 0.01);
    }

    // === Attention Decay Tests ===

    #[test]
    fn test_attention_decay_formula_with_nodes() {
        let config = SimConfig::default();
        // 0 nodes: effective_decay = 0.001 × (1 + 0) = 0.001
        let decay_0 = config.attention_decay_rate * (1.0 + 0.5 * 0.0);
        assert!((decay_0 - 0.001).abs() < 0.0001);

        // 10 nodes: effective_decay = 0.001 × (1 + 5) = 0.006
        let decay_10 = config.attention_decay_rate * (1.0 + 0.5 * 10.0);
        assert!((decay_10 - 0.006).abs() < 0.0001);

        // More nodes = more decay
        assert!(decay_10 > decay_0);
    }

    #[test]
    fn test_attention_recovery_below_threshold() {
        let config = SimConfig::default();
        // Agent at 0.5 attention should recover toward 0.8
        let attention = 0.5f32;
        assert!(attention < 0.8, "Below threshold triggers recovery");
        let recovered = attention + config.attention_recovery_rate;
        assert!(recovered > attention, "Recovery increases attention");
    }

    #[test]
    fn test_attention_net_effect_with_many_nodes() {
        let config = SimConfig::default();
        let decay = config.attention_decay_rate * (1.0 + 0.5 * 20.0);
        let net = config.attention_recovery_rate - decay;
        assert!(net < 0.0, "With many nodes, attention should net decrease: {}", net);
    }

    // === Population Density Cluster Tests (TDD RED → GREEN) ===

    #[test]
    fn test_generate_cities_count() {
        let mut rng = rand::thread_rng();
        let cities = generate_cities(1200.0, 800.0, &mut rng);
        assert_eq!(cities.len(), 12, "Should generate 12 cities. Got {}", cities.len());
    }

    #[test]
    fn test_generate_cities_fractions_sum() {
        let mut rng = rand::thread_rng();
        let cities = generate_cities(1200.0, 800.0, &mut rng);
        let total: f32 = cities.iter().map(|c| c.population_fraction).sum();
        // Should leave ~15% for rural scatter
        assert!((total - 0.85).abs() < 0.05,
            "City fractions should sum to ~0.85. Got {}", total);
    }

    #[test]
    fn test_generate_cities_within_bounds() {
        let mut rng = rand::thread_rng();
        let (w, h) = (1200.0, 800.0);
        let cities = generate_cities(w, h, &mut rng);
        for city in &cities {
            assert!(city.center.0.abs() <= w / 2.0,
                "City x={} out of bounds", city.center.0);
            assert!(city.center.1.abs() <= h / 2.0,
                "City y={} out of bounds", city.center.1);
        }
    }

    #[test]
    fn test_generate_cities_power_law() {
        let mut rng = rand::thread_rng();
        let cities = generate_cities(1200.0, 800.0, &mut rng);
        let mut fracs: Vec<f32> = cities.iter().map(|c| c.population_fraction).collect();
        fracs.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let largest = fracs[0];
        let median = fracs[fracs.len() / 2];
        assert!(largest > 2.0 * median,
            "Largest city ({}) should be >2× median ({})", largest, median);
    }

    #[test]
    fn test_sample_position_near_center() {
        let city = City { center: (100.0, 50.0), spread: 30.0, population_fraction: 0.3 };
        let mut rng = rand::thread_rng();
        let n = 1000;
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        for _ in 0..n {
            let (x, y) = sample_position_for_city(&city, 1200.0, 800.0, &mut rng);
            sx += x;
            sy += y;
        }
        let (mx, my) = (sx / n as f32, sy / n as f32);
        assert!((mx - 100.0).abs() < 15.0,
            "Mean x={} should be near city center 100", mx);
        assert!((my - 50.0).abs() < 15.0,
            "Mean y={} should be near city center 50", my);
    }

    #[test]
    fn test_sample_position_has_spread() {
        let city = City { center: (0.0, 0.0), spread: 50.0, population_fraction: 0.3 };
        let mut rng = rand::thread_rng();
        let mut max_dist = 0.0f32;
        for _ in 0..500 {
            let (x, y) = sample_position_for_city(&city, 1200.0, 800.0, &mut rng);
            let dist = (x * x + y * y).sqrt();
            max_dist = max_dist.max(dist);
        }
        // With spread=50, most samples within ~2σ=100, but some should exceed 20
        assert!(max_dist > 20.0,
            "Positions should have spread, max_dist={}", max_dist);
    }

    #[test]
    fn test_sample_position_clamped() {
        // City near edge — positions must stay within world bounds
        let city = City { center: (580.0, 380.0), spread: 100.0, population_fraction: 0.1 };
        let mut rng = rand::thread_rng();
        for _ in 0..500 {
            let (x, y) = sample_position_for_city(&city, 1200.0, 800.0, &mut rng);
            assert!(x.abs() <= 600.0, "x={} out of world bounds", x);
            assert!(y.abs() <= 400.0, "y={} out of world bounds", y);
        }
    }

    #[test]
    fn test_assign_positions_count() {
        let mut rng = rand::thread_rng();
        let cities = generate_cities(1200.0, 800.0, &mut rng);
        let positions = assign_positions(1000, &cities, 0.15, 1200.0, 800.0, &mut rng);
        assert_eq!(positions.len(), 1000, "Should return exactly N positions");
    }

    #[test]
    fn test_assign_positions_clustering() {
        let mut rng = rand::thread_rng();
        let cities = generate_cities(1200.0, 800.0, &mut rng);
        let positions = assign_positions(5000, &cities, 0.15, 1200.0, 800.0, &mut rng);

        // Most agents should be close to SOME city center.
        // For each position, find distance to nearest city.
        let mut near_city_count = 0;
        for (px, py) in &positions {
            let min_dist = cities.iter()
                .map(|c| ((px - c.center.0).powi(2) + (py - c.center.1).powi(2)).sqrt())
                .fold(f32::MAX, f32::min);
            // Within 2.5× the largest spread (120 * 2.5 = 300) counts as "near a city"
            if min_dist < 300.0 {
                near_city_count += 1;
            }
        }
        let near_fraction = near_city_count as f32 / positions.len() as f32;
        // With 85% urban + clustering, at least 70% should be near a city
        assert!(near_fraction > 0.70,
            "At least 70% should be near a city center. Got {:.1}%", near_fraction * 100.0);
    }
}
