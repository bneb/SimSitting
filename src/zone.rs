//! SimSitting — Zone System (Phase 2: The Attention Economy)
//!
//! Zones are the "geography of control." Players paint zones onto a 256×256
//! [`InfluenceMap`] that the GPU compute shader samples per-agent.
//!
//! ## Zone Types
//!
//! | Zone | Outrage | Narrowing | Revenue | Cost |
//! |---|---|---|---|---|
//! | **Echo Chamber** | 0.6 | 0.8 | 2.0× | $200 |
//! | **Neutral Hub** | 0.0 | 0.0 | 0.3× | $150 |
//! | **Data Refinery** | 0.2 | 0.3 | 0.5× | $500 |

use bevy::prelude::*;

// ============================================================================
// Zone Types
// ============================================================================

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZoneType {
    #[default]
    None,
    EchoChamber,
    NeutralHub,
    DataRefinery,
}

impl ZoneType {
    /// Encode zone type as the Alpha channel value (0.0–1.0)
    pub fn to_alpha(&self) -> f32 {
        match self {
            ZoneType::None        => 0.0,
            ZoneType::EchoChamber => 0.25,
            ZoneType::NeutralHub  => 0.50,
            ZoneType::DataRefinery => 0.75,
        }
    }

    /// Decode zone type from Alpha channel value
    pub fn from_alpha(a: f32) -> Self {
        if a < 0.125 { ZoneType::None }
        else if a < 0.375 { ZoneType::EchoChamber }
        else if a < 0.625 { ZoneType::NeutralHub }
        else { ZoneType::DataRefinery }
    }

    /// Cash cost to place this zone
    pub fn cost(&self) -> f64 {
        match self {
            ZoneType::None => 0.0,
            ZoneType::EchoChamber => 200.0,
            ZoneType::NeutralHub => 150.0,
            ZoneType::DataRefinery => 500.0,
        }
    }
}

// ============================================================================
// Zone Cell (single pixel in the influence map)
// ============================================================================

/// A single cell in the influence map. Encodes as RGBA for GPU texture.
#[derive(Clone, Copy, Debug, Default)]
pub struct ZoneCell {
    /// Outrage intensity: how much this zone amplifies extreme opinions (0.0–1.0)
    pub outrage: f32,
    /// Confidence narrowing: how much this zone reduces agent confidence bounds (0.0–1.0)
    pub narrowing: f32,
    /// Revenue multiplier: bonus revenue for agents in this zone (0.0–3.0)
    pub revenue_mult: f32,
    /// Zone type encoded as alpha
    pub zone_type: ZoneType,
}

impl ZoneCell {
    /// Create a cell for a specific zone type with default intensities
    pub fn from_zone(zone: ZoneType) -> Self {
        match zone {
            ZoneType::None => Self::default(),
            ZoneType::EchoChamber => Self {
                outrage: 0.6,
                narrowing: 0.8,
                revenue_mult: 2.0,
                zone_type: ZoneType::EchoChamber,
            },
            ZoneType::NeutralHub => Self {
                outrage: 0.0,
                narrowing: 0.0,
                revenue_mult: 0.3,
                zone_type: ZoneType::NeutralHub,
            },
            ZoneType::DataRefinery => Self {
                outrage: 0.2,
                narrowing: 0.3,
                revenue_mult: 0.5,
                zone_type: ZoneType::DataRefinery,
            },
        }
    }

    /// Pack into [R, G, B, A] for GPU texture upload
    pub fn to_rgba(&self) -> [f32; 4] {
        [
            self.outrage,
            self.narrowing,
            self.revenue_mult / 3.0, // Normalize to 0–1 range for texture
            self.zone_type.to_alpha(),
        ]
    }

    /// Unpack from [R, G, B, A] GPU texture data
    pub fn from_rgba(rgba: [f32; 4]) -> Self {
        Self {
            outrage: rgba[0],
            narrowing: rgba[1],
            revenue_mult: rgba[2] * 3.0,
            zone_type: ZoneType::from_alpha(rgba[3]),
        }
    }
}

// ============================================================================
// Influence Map (256×256 grid)
// ============================================================================

pub const INFLUENCE_MAP_SIZE: usize = 256;

/// The influence map: a 256×256 grid of ZoneCells. Lives in Main World.
/// Uploaded to a GPU texture each frame for the compute shader to sample.
#[derive(Resource)]
pub struct InfluenceMap {
    pub cells: Vec<ZoneCell>,
    pub width: usize,
    pub height: usize,
    pub dirty: bool, // Set when cells change, cleared after GPU upload
}

impl Default for InfluenceMap {
    fn default() -> Self {
        Self {
            cells: vec![ZoneCell::default(); INFLUENCE_MAP_SIZE * INFLUENCE_MAP_SIZE],
            width: INFLUENCE_MAP_SIZE,
            height: INFLUENCE_MAP_SIZE,
            dirty: false,
        }
    }
}

impl InfluenceMap {
    /// Get a cell at grid coordinates
    pub fn get(&self, x: usize, y: usize) -> &ZoneCell {
        &self.cells[y * self.width + x]
    }

    /// Set a cell at grid coordinates, marking map dirty
    pub fn set(&mut self, x: usize, y: usize, cell: ZoneCell) {
        self.cells[y * self.width + x] = cell;
        self.dirty = true;
    }

    /// Convert world coordinates to grid coordinates
    pub fn world_to_grid(&self, world_x: f32, world_y: f32, world_w: f32, world_h: f32) -> (usize, usize) {
        // World space: -w/2..w/2, -h/2..h/2 → grid: 0..255
        let nx = ((world_x + world_w / 2.0) / world_w).clamp(0.0, 0.9999);
        let ny = ((world_y + world_h / 2.0) / world_h).clamp(0.0, 0.9999);
        (
            (nx * self.width as f32) as usize,
            (ny * self.height as f32) as usize,
        )
    }

    /// Convert world coordinates to UV (0.0–1.0) for GPU texture sampling
    pub fn world_to_uv(world_x: f32, world_y: f32, world_w: f32, world_h: f32) -> (f32, f32) {
        (
            ((world_x + world_w / 2.0) / world_w).clamp(0.0, 1.0),
            ((world_y + world_h / 2.0) / world_h).clamp(0.0, 1.0),
        )
    }

    /// Paint a zone with a circular brush at world coordinates.
    /// Returns the number of cells painted.
    pub fn paint_circle(
        &mut self,
        center_gx: usize,
        center_gy: usize,
        radius: usize,
        cell: ZoneCell,
    ) -> usize {
        let mut count = 0;
        let r2 = (radius * radius) as i64;

        let min_x = center_gx.saturating_sub(radius);
        let max_x = (center_gx + radius).min(self.width - 1);
        let min_y = center_gy.saturating_sub(radius);
        let max_y = (center_gy + radius).min(self.height - 1);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as i64 - center_gx as i64;
                let dy = y as i64 - center_gy as i64;
                if dx * dx + dy * dy <= r2 {
                    self.set(x, y, cell);
                    count += 1;
                }
            }
        }
        count
    }

    /// Flatten to RGBA f32 array for GPU texture upload (width * height * 4 floats)
    pub fn to_rgba_buffer(&self) -> Vec<f32> {
        let mut buf = Vec::with_capacity(self.width * self.height * 4);
        for cell in &self.cells {
            let rgba = cell.to_rgba();
            buf.extend_from_slice(&rgba);
        }
        buf
    }
}

// ============================================================================
// Zone Brush (player tool state)
// ============================================================================

#[derive(Resource)]
pub struct ZoneBrush {
    pub active_zone: ZoneType,
    pub radius: usize, // Grid cells
    pub is_painting: bool,
}

impl Default for ZoneBrush {
    fn default() -> Self {
        Self {
            active_zone: ZoneType::EchoChamber,
            radius: 8,
            is_painting: false,
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct ZonePlugin;

impl Plugin for ZonePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InfluenceMap>();
        app.init_resource::<ZoneBrush>();
    }
}

// ============================================================================
// Tests (TDD — written BEFORE implementation verified in game)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === Zone Type Tests ===

    #[test]
    fn test_zone_type_alpha_roundtrip() {
        for zone in [ZoneType::None, ZoneType::EchoChamber, ZoneType::NeutralHub, ZoneType::DataRefinery] {
            let alpha = zone.to_alpha();
            let decoded = ZoneType::from_alpha(alpha);
            assert_eq!(zone, decoded, "Roundtrip failed for {:?} (alpha={})", zone, alpha);
        }
    }

    #[test]
    fn test_zone_type_alpha_values() {
        assert!((ZoneType::None.to_alpha() - 0.0).abs() < 0.001);
        assert!((ZoneType::EchoChamber.to_alpha() - 0.25).abs() < 0.001);
        assert!((ZoneType::NeutralHub.to_alpha() - 0.50).abs() < 0.001);
        assert!((ZoneType::DataRefinery.to_alpha() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_zone_costs() {
        assert_eq!(ZoneType::None.cost(), 0.0);
        assert_eq!(ZoneType::EchoChamber.cost(), 200.0);
        assert_eq!(ZoneType::NeutralHub.cost(), 150.0);
        assert_eq!(ZoneType::DataRefinery.cost(), 500.0);
    }

    // === Zone Cell Tests ===

    #[test]
    fn test_zone_cell_rgba_roundtrip() {
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        let rgba = cell.to_rgba();
        let decoded = ZoneCell::from_rgba(rgba);

        assert!((cell.outrage - decoded.outrage).abs() < 0.01);
        assert!((cell.narrowing - decoded.narrowing).abs() < 0.01);
        assert!((cell.revenue_mult - decoded.revenue_mult).abs() < 0.1);
        assert_eq!(cell.zone_type, decoded.zone_type);
    }

    #[test]
    fn test_echo_chamber_defaults() {
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        assert!(cell.outrage > 0.5, "Echo chambers should have high outrage");
        assert!(cell.narrowing > 0.5, "Echo chambers should narrow confidence");
        assert!(cell.revenue_mult > 1.0, "Echo chambers should boost revenue");
    }

    #[test]
    fn test_neutral_hub_defaults() {
        let cell = ZoneCell::from_zone(ZoneType::NeutralHub);
        assert!((cell.outrage).abs() < 0.001, "Neutral hubs should have zero outrage");
        assert!((cell.narrowing).abs() < 0.001, "Neutral hubs should not narrow confidence");
        assert!(cell.revenue_mult < 1.0, "Neutral hubs should reduce revenue");
    }

    #[test]
    fn test_data_refinery_defaults() {
        let cell = ZoneCell::from_zone(ZoneType::DataRefinery);
        assert!(cell.outrage < 0.5, "Refineries have low outrage");
        assert!(cell.revenue_mult < 1.0, "Refineries have low revenue");
    }

    #[test]
    fn test_none_cell_is_zero() {
        let cell = ZoneCell::default();
        assert!((cell.outrage).abs() < 0.001);
        assert!((cell.narrowing).abs() < 0.001);
        assert!((cell.revenue_mult).abs() < 0.001);
    }

    // === Influence Map Tests ===

    #[test]
    fn test_influence_map_dimensions() {
        let map = InfluenceMap::default();
        assert_eq!(map.width, 256);
        assert_eq!(map.height, 256);
        assert_eq!(map.cells.len(), 256 * 256);
    }

    #[test]
    fn test_influence_map_byte_size() {
        let map = InfluenceMap::default();
        let rgba_buf = map.to_rgba_buffer();
        // 256 × 256 × 4 floats = 262,144 floats
        assert_eq!(rgba_buf.len(), 256 * 256 * 4);
        // In bytes: 262,144 × 4 = 1,048,576 bytes = 1 MB
        assert_eq!(rgba_buf.len() * std::mem::size_of::<f32>(), 1_048_576);
    }

    #[test]
    fn test_influence_map_set_get() {
        let mut map = InfluenceMap::default();
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        map.set(100, 100, cell);
        let retrieved = map.get(100, 100);
        assert_eq!(retrieved.zone_type, ZoneType::EchoChamber);
        assert!(map.dirty);
    }

    #[test]
    fn test_world_to_grid_center() {
        let map = InfluenceMap::default();
        let (gx, gy) = map.world_to_grid(0.0, 0.0, 1000.0, 1000.0);
        assert_eq!(gx, 128);
        assert_eq!(gy, 128);
    }

    #[test]
    fn test_world_to_grid_corners() {
        let map = InfluenceMap::default();
        // Bottom-left corner
        let (gx, gy) = map.world_to_grid(-500.0, -500.0, 1000.0, 1000.0);
        assert_eq!(gx, 0);
        assert_eq!(gy, 0);
        // Near top-right
        let (gx, gy) = map.world_to_grid(499.0, 499.0, 1000.0, 1000.0);
        assert_eq!(gx, 255);
        assert_eq!(gy, 255);
    }

    #[test]
    fn test_world_to_uv_center() {
        let (u, v) = InfluenceMap::world_to_uv(0.0, 0.0, 1000.0, 1000.0);
        assert!((u - 0.5).abs() < 0.001);
        assert!((v - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_world_to_uv_edges_clamped() {
        let (u, v) = InfluenceMap::world_to_uv(-1000.0, -1000.0, 1000.0, 1000.0);
        assert!((u - 0.0).abs() < 0.001);
        assert!((v - 0.0).abs() < 0.001);

        let (u, v) = InfluenceMap::world_to_uv(1000.0, 1000.0, 1000.0, 1000.0);
        assert!((u - 1.0).abs() < 0.001);
        assert!((v - 1.0).abs() < 0.001);
    }

    // === Paint Brush Tests ===

    #[test]
    fn test_paint_circle_center() {
        let mut map = InfluenceMap::default();
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        let painted = map.paint_circle(128, 128, 5, cell);
        // Circle of radius 5: roughly π*5² ≈ 78 cells
        assert!(painted > 60 && painted < 100, "Expected ~78 cells, got {}", painted);
        assert_eq!(map.get(128, 128).zone_type, ZoneType::EchoChamber);
    }

    #[test]
    fn test_paint_circle_edge_clamping() {
        let mut map = InfluenceMap::default();
        let cell = ZoneCell::from_zone(ZoneType::NeutralHub);
        // Paint at corner — should not panic
        let painted = map.paint_circle(0, 0, 3, cell);
        assert!(painted > 0);
        assert_eq!(map.get(0, 0).zone_type, ZoneType::NeutralHub);
    }

    #[test]
    fn test_paint_does_not_affect_outside_radius() {
        let mut map = InfluenceMap::default();
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        map.paint_circle(128, 128, 3, cell);
        // Cell far from center should be unaffected
        assert_eq!(map.get(200, 200).zone_type, ZoneType::None);
    }

    // === Zone Effect Tests (what the compute shader will do) ===

    #[test]
    fn test_echo_chamber_narrows_confidence() {
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        let mut confidence = 0.3f32;
        // Simulate what the WGSL shader does: confidence -= zone.g * 0.01
        confidence -= cell.narrowing * 0.01;
        assert!(confidence < 0.3, "Confidence should decrease in echo chamber");
        assert!(confidence > 0.0, "Confidence should remain positive");
    }

    #[test]
    fn test_neutral_hub_no_confidence_change() {
        let cell = ZoneCell::from_zone(ZoneType::NeutralHub);
        let original = 0.3f32;
        let mut confidence = original;
        confidence -= cell.narrowing * 0.01;
        assert!((confidence - original).abs() < 0.001, "Neutral hub should not change confidence");
    }

    #[test]
    fn test_echo_chamber_boosts_engagement() {
        let cell = ZoneCell::from_zone(ZoneType::EchoChamber);
        let mut engagement = 0.5f32;
        // Simulate: engagement *= (1.0 + zone.b * 0.1)
        let zone_b = cell.revenue_mult / 3.0; // Normalized to 0–1
        engagement *= 1.0 + zone_b * 0.1;
        assert!(engagement > 0.5, "Engagement should increase in echo chamber");
    }

    // === Zone Brush Tests ===

    #[test]
    fn test_zone_brush_defaults() {
        let brush = ZoneBrush::default();
        assert_eq!(brush.active_zone, ZoneType::EchoChamber);
        assert_eq!(brush.radius, 8);
        assert!(!brush.is_painting);
    }
}
