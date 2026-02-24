// Most items in SimSitting are used by tests, GPU shaders, or Bevy ECS runtime
// systems that the compiler cannot statically trace. Suppress false positives.
#![allow(dead_code)]

//! # SimSitting — A Media Company Simulator
//!
//! Interactive art exploring mechanisms of control, media manipulation,
//! and the attention economy. Built with Rust + Bevy + WebGPU.
//!
//! 100,000 opinion-bearing agents processed on the GPU at 60fps.
//! A Windows 95 brutalist UI that bleaches itself as optimization increases.
//! The clock is stuck at 6:47 PM until the population reaches Total Consensus.
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`sim`] | Agent components, Deffuant-Weisbuch opinion dynamics |
//! | [`compute`] | GPU compute pipeline, buffer management, analytics readback |
//! | [`render`] | Sprite rendering, camera shake, knockout visuals |
//! | [`media`] | Media node placement, cognitive gravity |
//! | [`economy`] | Revenue model, Narrative Capital, quarterly reports |
//! | [`ui`] | BROADCAST_OS dashboard, zone toolbar, election banners |
//! | [`audio`] | Procedural synth, Dorian/Lydian/Phrygian modes |
//! | [`zone`] | 256×256 influence map, zone painting |
//! | [`politics`] | Elections, mandates, government contracts, singularity |
//! | [`shadow`] | Shadow Filters, forbidden range drift, Public Trust |
//! | [`oracle`] | Greedy autopilot, session history, epilogue |
//! | [`telemetry`] | Diegetic telemetry: oscilloscope, vitality, thermal |
//! | [`humint`] | HUMINT profiler: procedural life data, personhood |
//! | [`interop`] | Fourth wall: browser bridge, real-world context |

mod sim;
mod media;
mod economy;
mod render;
mod ui;
mod compute;
mod audio;
mod zone;
mod politics;
mod shadow;
mod oracle;
mod telemetry;
mod humint;
mod interop;

use bevy::prelude::*;

/// Entry point: builds the Bevy app with all plugins, resources, and systems.
///
/// System scheduling is split into two `Update` groups to stay under
/// Bevy's tuple size limit for system registration.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "SimSitting — Media Company Simulator".into(),
                resolution: bevy::window::WindowResolution::new(1600.0, 900.0),
                canvas: Some("#bevy-canvas".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_egui::EguiPlugin { enable_multipass_for_primary_context: false })
        // Resources
        .init_resource::<sim::SimConfig>()
        .init_resource::<economy::GlobalStats>()
        .init_resource::<media::PlacementState>()
        .init_resource::<render::CameraState>()
        .init_resource::<render::CameraShake>()
        .init_resource::<ui::UiState>()
        .init_resource::<economy::CurrentPhase>()
        // Startup systems
        .add_systems(Startup, (
            render::setup_camera,
            sim::setup_simulation,
        ))
        .add_plugins(compute::ComputePlugin)
        .add_plugins(audio::AudioPlugin)
        .add_plugins(zone::ZonePlugin)
        .add_plugins(politics::PoliticsPlugin)
        .add_plugins(shadow::ShadowPlugin)
        .add_plugins(oracle::OraclePlugin)
        .add_plugins(telemetry::TelemetryPlugin)
        .add_plugins(humint::HumintPlugin)
        .add_plugins(interop::InteropPlugin)
        .add_systems(Startup, render::setup_agent_sprites.after(sim::setup_simulation))
        .add_systems(Startup, compute::setup_gpu_buffers.after(sim::setup_simulation))
        // Update systems — split into groups to stay under the tuple limit
        .add_systems(Update, (
            sim::opinion_dynamics,
            sim::attention_decay,
            media::media_influence,
            economy::calculate_revenue,
            economy::update_stats,
            economy::quarterly_report,
        ))
        .add_systems(Update, (
            sim::update_agent_visuals,
            render::camera_zoom,
            render::camera_pan,
            render::update_camera_shake,
            render::knockout_agent_scatter,
            media::place_media_node,
            ui::mike_tyson_check_system,
            ui::dashboard_ui,
        ))
        .add_systems(Update, ui::humint_profiler_ui)
        .add_systems(Update, ui::singularity_epilogue_ui)
        .add_systems(Update, ui::game_overlay_ui)
        .run();
}
