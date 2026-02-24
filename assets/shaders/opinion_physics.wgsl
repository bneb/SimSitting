// SimSitting — Opinion Physics Compute Shader
// Deffuant-Weisbuch opinion dynamics for 100k agents on the GPU
// Phase 3: Spatial Influence Map + Political Histogram

struct SimAgentRaw {
    opinion: f32,
    confidence: f32,
    engagement: f32,
    susceptibility: f32,
    pos_x: f32,
    pos_y: f32,
    personhood: f32,
    _pad1: f32,
};

struct GlobalAnalytics {
    sum_opinion: atomic<u32>,
    sum_polarization: atomic<u32>,
    sum_engagement: atomic<u32>,
    agent_count: u32,
};

@group(0) @binding(0) var<storage, read_write> sims: array<SimAgentRaw>;
@group(0) @binding(1) var<storage, read_write> stats: GlobalAnalytics;

// Phase 2: Influence map texture (256×256 RGBA)
// R = outrage intensity, G = confidence narrowing, B = revenue multiplier (normalized), A = zone type
@group(1) @binding(0) var influence_map: texture_2d<f32>;
@group(1) @binding(1) var map_sampler: sampler;

const SCALE: f32 = 1000.0; // Fixed-point scale for atomic sums (1000 for 100k agents)
const WORLD_WIDTH: f32 = 1000.0;
const WORLD_HEIGHT: f32 = 1000.0;

// Phase 3: Political histogram (10 opinion buckets + total vote count)
struct PoliticalData {
    buckets: array<atomic<u32>, 10>,
    total_votes: atomic<u32>,
};

@group(2) @binding(0) var<storage, read_write> politics: PoliticalData;

// Simple PRNG
fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state * 2654435769u;
    state = state ^ (state >> 16u);
    state = state ^ (state >> 16u);
    return state;
}

fn rand_float(state: ptr<function, u32>) -> f32 {
    *state = hash(*state);
    return f32(*state) / 4294967295.0; // 0.0 to 1.0
}

@compute @workgroup_size(64)
fn update_opinions(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= stats.agent_count) { return; }

    var agent = sims[index];

    var seed = index + u32(agent.opinion * 100000.0);

    // --- ZONE INFLUENCE ---
    // Convert world position to UV for texture sampling
    let uv = vec2<f32>(
        (agent.pos_x + WORLD_WIDTH * 0.5) / WORLD_WIDTH,
        (agent.pos_y + WORLD_HEIGHT * 0.5) / WORLD_HEIGHT
    );
    let zone_data = textureSampleLevel(influence_map, map_sampler, uv, 0.0);

    // Zone effects:
    // G channel = confidence narrowing (Echo Chambers shrink the Overton window)
    agent.confidence = max(agent.confidence - zone_data.g * 0.01, 0.01);

    // B channel = engagement multiplier (normalized 0-1, represents 0-3x)
    agent.engagement = clamp(agent.engagement * (1.0 + zone_data.b * 0.1), 0.0, 1.0);

    // R channel = outrage amplification (pushes opinions away from center)
    if (zone_data.r > 0.0) {
        let center_dist = agent.opinion - 0.5;
        agent.opinion = clamp(
            agent.opinion + center_dist * zone_data.r * 0.005 * agent.susceptibility,
            0.0, 1.0
        );
    }

    // --- DEFFUANT-WEISBUCH OPINION DYNAMICS ---
    let target_idx = u32(rand_float(&seed) * f32(stats.agent_count));

    if target_idx < stats.agent_count && target_idx != index {
        let them = sims[target_idx];

        let diff = abs(agent.opinion - them.opinion);
        let mutual_confidence = (agent.confidence + them.confidence) * 0.5;

        if diff < mutual_confidence {
            let drift = 0.02 * agent.susceptibility;
            agent.opinion = clamp(agent.opinion + (them.opinion - agent.opinion) * drift, 0.0, 1.0);
        }
    }

    // --- ANALYTICS COLLECTION ---
    let polarization = abs(agent.opinion - 0.5) * 2.0;

    let u_opinion = u32(agent.opinion * SCALE);
    let u_polarization = u32(polarization * SCALE);
    let u_engagement = u32(agent.engagement * SCALE);

    atomicAdd(&stats.sum_opinion, u_opinion);
    atomicAdd(&stats.sum_polarization, u_polarization);
    atomicAdd(&stats.sum_engagement, u_engagement);

    // --- POLITICAL HISTOGRAM ---
    // Bucket each agent's opinion into one of 10 bins for the election system
    let bucket_idx = u32(clamp(agent.opinion * 10.0, 0.0, 9.0));
    atomicAdd(&politics.buckets[bucket_idx], 1u);
    atomicAdd(&politics.total_votes, 1u);

    sims[index] = agent;
}
