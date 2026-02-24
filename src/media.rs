//! SimSitting — Media Node System
//!
//! Players place media nodes to exert "cognitive gravity" on nearby agents.
//! Three node types model different media strategies:
//!
//! - **Echo Chamber**: Strong opinion push, fast attention decay, 3× revenue
//! - **Public Square**: Gentle center pull, attention recovery, 0.5× revenue
//! - **Data Refinery**: Increases susceptibility, mild push, 1.5× revenue
//!
//! Influence follows inverse-square falloff within the node's radius.

use bevy::prelude::*;
use crate::sim::SimAgent;

/// Types of media infrastructure the player can build
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeType {
    /// High-density opinion silo. High engagement revenue, increases polarization.
    EchoChamber,
    /// Diverse interaction zone. Low profit, maintains social cohesion.
    PublicSquare,
    /// Data processing. Unlocks psychographic targeting (increases susceptibility).
    DataRefinery,
}

impl NodeType {
    /// Returns the human-readable label for this node type.
    pub fn label(&self) -> &'static str {
        match self {
            NodeType::EchoChamber => "Echo Chamber",
            NodeType::PublicSquare => "Public Square",
            NodeType::DataRefinery => "Data Refinery",
        }
    }

    /// Returns the display color for this node type (RGBA).
    pub fn color(&self) -> Color {
        match self {
            NodeType::EchoChamber => Color::srgba(0.459, 0.231, 0.741, 0.6),  // Jazz Purple
            NodeType::PublicSquare => Color::srgba(0.0, 0.663, 0.616, 0.6),   // Jazz Teal
            NodeType::DataRefinery => Color::srgba(1.0, 0.549, 0.0, 0.6),     // Sunset Orange
        }
    }

    /// Returns the revenue multiplier applied to agents within this node's radius.
    pub fn revenue_multiplier(&self) -> f32 {
        match self {
            NodeType::EchoChamber => 3.0,
            NodeType::PublicSquare => 0.5,
            NodeType::DataRefinery => 1.5,
        }
    }

    /// Returns the cash cost to place this node type.
    pub fn cost(&self) -> f64 {
        match self {
            NodeType::EchoChamber => 500.0,
            NodeType::PublicSquare => 200.0,
            NodeType::DataRefinery => 1000.0,
        }
    }
}

/// A media broadcast node placed by the player
#[derive(Component)]
pub struct MediaNode {
    /// The opinion value this node pushes agents toward (0.0–1.0)
    pub narrative_target: f32,
    /// Broadcast power (0.0–1.0)
    pub intensity: f32,
    /// Spatial influence radius in world units
    pub radius: f32,
    /// Type of media infrastructure
    pub node_type: NodeType,
}

/// Visual marker for the media node's influence radius
#[derive(Component)]
pub struct MediaNodeVisual;

/// Resource tracking the player's node placement state
#[derive(Resource, Default)]
pub struct PlacementState {
    /// Whether the player is currently in placement mode
    pub active: bool,
    /// Selected node type
    pub node_type: NodeType,
    /// Narrative target for the next node
    pub narrative_target: f32,
    /// Intensity for the next node
    pub intensity: f32,
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::EchoChamber
    }
}

/// System: Media nodes influence nearby agents' opinions
pub fn media_influence(
    nodes: Query<(&MediaNode, &Transform)>,
    mut agents: Query<(&mut SimAgent, &Transform), Without<MediaNode>>,
) {
    for (node, node_transform) in nodes.iter() {
        let node_pos = node_transform.translation.truncate();
        let radius_sq = node.radius * node.radius;

        for (mut agent, agent_transform) in agents.iter_mut() {
            let agent_pos = agent_transform.translation.truncate();
            let dist_sq = node_pos.distance_squared(agent_pos);

            if dist_sq < radius_sq {
                // Influence falls off with distance (inverse square-ish)
                let dist_factor = 1.0 - (dist_sq / radius_sq).sqrt();
                let influence = node.intensity * agent.susceptibility * agent.attention * dist_factor * 0.005;

                // Apply special effects based on node type
                match node.node_type {
                    NodeType::EchoChamber => {
                        // Strong push toward narrative, but decays attention faster
                        agent.opinion += (node.narrative_target - agent.opinion) * influence * 2.0;
                        agent.attention = (agent.attention - 0.0003).max(0.0);
                    }
                    NodeType::PublicSquare => {
                        // Gentle push toward center (0.5), recovers attention
                        agent.opinion += (0.5 - agent.opinion) * influence * 0.5;
                        agent.attention = (agent.attention + 0.0001).min(1.0);
                    }
                    NodeType::DataRefinery => {
                        // Increases susceptibility, mild opinion push
                        agent.opinion += (node.narrative_target - agent.opinion) * influence;
                        agent.susceptibility = (agent.susceptibility + 0.0001).min(1.0);
                    }
                }

                agent.opinion = agent.opinion.clamp(0.0, 1.0);
            }
        }
    }
}

/// System: Handle mouse clicks to place media nodes
pub fn place_media_node(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut placement: ResMut<PlacementState>,
    mut stats: ResMut<crate::economy::GlobalStats>,
) {
    if !placement.active || !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_transform)) = camera_q.single() else { return };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor_pos) {
            let cost = placement.node_type.cost();
            if stats.cash < cost {
                return; // Not enough money
            }
            stats.cash -= cost;

            let radius = match placement.node_type {
                NodeType::EchoChamber => 120.0,
                NodeType::PublicSquare => 200.0,
                NodeType::DataRefinery => 80.0,
            };

            let node_color = placement.node_type.color();

            // Spawn the media node entity
            commands.spawn((
                MediaNode {
                    narrative_target: placement.narrative_target,
                    intensity: placement.intensity,
                    radius,
                    node_type: placement.node_type,
                },
                MediaNodeVisual,
                Sprite {
                    color: node_color,
                    custom_size: Some(Vec2::new(radius * 2.0, radius * 2.0)),
                    ..default()
                },
                Transform::from_xyz(world_pos.x, world_pos.y, 1.0),
                Visibility::default(),
            ));

            // Exit placement mode after placing
            placement.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_defaults() {
        let nt = NodeType::default();
        assert_eq!(nt, NodeType::EchoChamber);
    }

    #[test]
    fn test_node_costs() {
        assert!(NodeType::DataRefinery.cost() > NodeType::EchoChamber.cost());
        assert!(NodeType::EchoChamber.cost() > NodeType::PublicSquare.cost());
    }

    #[test]
    fn test_revenue_multipliers() {
        assert!(NodeType::EchoChamber.revenue_multiplier() > NodeType::PublicSquare.revenue_multiplier());
    }
}
