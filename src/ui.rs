//! SimSitting — UI Dashboard (`bevy_egui`)
//!
//! The "BROADCAST_OS" workstation interface. Windows 95 Brutalist aesthetic
//! crossed with the Solo Jazz Cup teal-and-purple palette and 90s NBA vibes.
//!
//! ## UI Sections
//!
//! - **Left Panel**: Revenue ticker, stats, zone toolbar, election results,
//!   government contracts, shadow filter controls, Oracle unlock, lobbying
//! - **Right Panel**: Opinion histogram, agent scatter plot
//! - **Overlays**: Mike Tyson Knockout glitch, Regulatory Audit warning,
//!   Singularity full-screen (6:48 PM, sterile white, no interaction)
//!
//! As [`OracleState::optimization_level`] increases, the UI colors morph
//! from dark brutalist to sterile clinical white.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::economy::{GlobalStats, LICENSE_FEE, GamePhase};
use crate::media::{PlacementState, NodeType};
use crate::zone::{ZoneBrush, ZoneType, InfluenceMap};
use crate::politics::{PartyMandate, ElectionState, GovernmentContracts, Party, SingularityState, SingularityType};
use crate::shadow::{ShadowMode, PublicTrust};
use crate::oracle::OracleState;
use crate::telemetry::UiTelemetry;
use crate::humint::{SimSelection, generate_life_data, HumintInput};
use crate::sim::SimAgent;
use rand::Rng;

#[derive(Resource)]
pub struct UiState {
    pub is_glitching: bool,
    pub corporate_message: String,
    pub clock_time: f32, // The ticking time in seconds from 00:00
    /// Whether the [?] game legend panel is open
    pub show_legend: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            is_glitching: false,
            corporate_message: String::new(),
            // 6:47 PM = 18 * 3600 + 47 * 60 = 67620
            clock_time: 67620.0,
            show_legend: false,
        }
    }
}

/// System: checks if "Mike Tyson Knockout" UI glitch should trigger.
/// Activates when polarization_heat > 0.85 and apathy_rate < 0.2.
pub fn mike_tyson_check_system(
    stats: Res<GlobalStats>,
    mut ui_state: ResMut<UiState>,
    time: Res<Time>,
) {
    if stats.polarization_heat > 0.85 && stats.apathy_rate < 0.2 {
        ui_state.is_glitching = true;
        ui_state.corporate_message = "REGULATORY_INTERFERENCE: CEASE AND DESIST".to_string();
    }

    if !ui_state.is_glitching {
        // Tick up slower than real time to stay around 6:47 PM for a while
        ui_state.clock_time += time.delta_secs() * 0.1; 
    } else {
        // Glitch the clock backwards and forwards
        let mut rng = rand::thread_rng();
        ui_state.clock_time += rng.gen_range(-50.0..50.0);
    }
}

// Jazz Cup Palette
const JAZZ_TEAL: egui::Color32 = egui::Color32::from_rgb(0, 169, 157);      // #00A99D
const JAZZ_PURPLE: egui::Color32 = egui::Color32::from_rgb(117, 59, 189);    // #753BBD
const RHETORIC_ORANGE: egui::Color32 = egui::Color32::from_rgb(255, 140, 0); // #FF8C00
const WIN_GRAY: egui::Color32 = egui::Color32::from_rgb(192, 192, 192);      // #C0C0C0
const WIN_DARK: egui::Color32 = egui::Color32::from_rgb(64, 64, 64);         // #404040
const TERMINAL_GREEN: egui::Color32 = egui::Color32::from_rgb(0, 255, 0);    // #00FF00
const BG_BLACK: egui::Color32 = egui::Color32::from_rgb(10, 10, 12);         // #0A0A0C

/// System: Render the main UI dashboard — "BROADCAST_OS v1.04"
pub fn dashboard_ui(
    mut contexts: EguiContexts,
    stats: Res<GlobalStats>,
    mut placement: ResMut<PlacementState>,
    mut ui_state: ResMut<UiState>,
    mut zone_brush: ResMut<ZoneBrush>,
    _influence_map: ResMut<InfluenceMap>,
    mandate: Res<PartyMandate>,
    mut election: ResMut<ElectionState>,
    contracts: Res<GovernmentContracts>,
    mut shadow_mode: ResMut<ShadowMode>,
    public_trust: Res<PublicTrust>,
    mut oracle_state: ResMut<OracleState>,
    singularity: Res<SingularityState>,
    time: Res<Time>,
    telemetry: Res<UiTelemetry>,
) {
    // === SINGULARITY SCREEN (full overlay, no interaction) ===
    if singularity.triggered {
        let ctx = contexts.ctx_mut();
        egui::Area::new(egui::Id::new("singularity_overlay"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen_rect = ui.ctx().screen_rect();
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(240, 240, 245, 250),
                );
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(
                    egui::Rect::from_center_size(
                        screen_rect.center(),
                        egui::vec2(500.0, 300.0),
                    )),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            let title = match singularity.singularity_type {
                                SingularityType::TotalConsensus => "TOTAL CONSENSUS ACHIEVED",
                                SingularityType::TotalPolarization => "TOTAL POLARIZATION ACHIEVED",
                                _ => "SINGULARITY",
                            };
                            ui.label(
                                egui::RichText::new(title)
                                    .color(egui::Color32::from_rgb(40, 40, 40))
                                    .size(28.0)
                                    .strong()
                            );
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("6:48 PM")
                                    .color(egui::Color32::from_rgb(100, 100, 100))
                                    .size(48.0)
                                    .monospace()
                            );
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new(
                                    "The simulation no longer requires human input.\n\
                                     The parking lot is perfectly silent."
                                )
                                    .color(egui::Color32::from_rgb(120, 120, 130))
                                    .size(14.0)
                                    .italics()
                            );
                        });
                    },
                );
            });
        return; // No further UI rendering
    }

    let ctx = contexts.ctx_mut();
    
    // Simulate UI Glitch (Mike Tyson Knockout)
    let _glitch_offset = if ui_state.is_glitching {
        let mut rng = rand::thread_rng();
        egui::vec2(rng.gen_range(-5.0..5.0), rng.gen_range(-5.0..5.0))
    } else {
        egui::vec2(0.0, 0.0)
    };
    
    // Create an overall frame to apply glitch translation if needed
    // (egui doesn't have an easy global offset without a central panel, so we will color tint instead)

    let mut win_dark = WIN_DARK;
    let mut jazz_teal = JAZZ_TEAL;
    if ui_state.is_glitching {
        let flash = (time.elapsed_secs() * 20.0).sin();
        if flash > 0.0 {
            win_dark = egui::Color32::from_rgb(150, 0, 0);
            jazz_teal = egui::Color32::from_rgb(255, 0, 0);
        }
    }

    // -- Win95 Brutalist theme --
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = win_dark;
    style.visuals.panel_fill = win_dark;
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(26, 26, 26);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 40, 50);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 50, 80);
    style.visuals.widgets.active.bg_fill = JAZZ_PURPLE;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, WIN_GRAY);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, WIN_GRAY);
    ctx.set_style(style);

    // ========================================
    // TOP: Jazz Cup Header Bar
    // ========================================
    egui::TopBottomPanel::top("jazz_header")
        .frame(egui::Frame::NONE.fill(jazz_teal).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("SIMSITTING // BROADCAST_OS")
                        .color(egui::Color32::WHITE)
                        .size(18.0)
                        .strong()
                        .italics()
                );

                ui.separator();

                // Cash display — desaturates when engagement is gated
                let cash_color = if telemetry.engagement_gated {
                    egui::Color32::from_rgb(120, 120, 120) // Ghostly gray
                } else {
                    egui::Color32::BLACK
                };
                ui.label(
                    egui::RichText::new(format!("${:.0}", stats.cash))
                        .color(cash_color)
                        .size(16.0)
                        .strong()
                        .monospace()
                );

                ui.separator();

                // Narrative Capital display
                ui.label(
                    egui::RichText::new(format!("NC: {:.0}", stats.narrative_capital))
                        .color(egui::Color32::from_rgb(200, 160, 255))
                        .size(12.0)
                        .strong()
                        .monospace()
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // System stability indicator
                    let stability = (stats.social_cohesion * 100.0) as u32;
                    ui.label(
                        egui::RichText::new(format!("SYSTEM_STABILITY: {:.1}%", stability))
                            .color(egui::Color32::BLACK)
                            .size(10.0)
                            .strong()
                            .monospace()
                    );

                    if placement.active {
                        ui.label(
                            egui::RichText::new("⊕ PLACEMENT_MODE_ACTIVE")
                                .color(RHETORIC_ORANGE)
                                .size(10.0)
                                .strong()
                        );
                    }

                    // [?] Legend toggle button
                    let legend_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new(if ui_state.show_legend { "[×]" } else { "[?]" })
                                .color(egui::Color32::BLACK)
                                .size(12.0)
                                .strong()
                                .monospace()
                        )
                        .fill(if ui_state.show_legend {
                            egui::Color32::from_rgb(200, 200, 200)
                        } else {
                            JAZZ_TEAL
                        })
                    );
                    if legend_btn.clicked() {
                        ui_state.show_legend = !ui_state.show_legend;
                    }
                });
            });
        });

    // ========================================
    // BOTTOM: Windows 95 Taskbar
    // ========================================
    egui::TopBottomPanel::bottom("taskbar")
        .frame(egui::Frame::NONE.fill(WIN_GRAY).inner_margin(egui::Margin::symmetric(4, 2)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("▸ Start")
                        .color(egui::Color32::BLACK)
                        .size(11.0)
                        .strong()
                );
                ui.separator();

                // === BANDWIDTH OSCILLOSCOPE ===
                // CRT-style SNR visualization in the taskbar
                let osc_width = 120.0;
                let osc_height = 16.0;
                let (osc_rect, _) = ui.allocate_exact_size(
                    egui::vec2(osc_width, osc_height), egui::Sense::hover()
                );
                // Black background
                ui.painter().rect_filled(osc_rect, 0.0, BG_BLACK);

                // Draw waveform: 30 segments with jitter based on SNR
                let jitter_amp = (1.0 - telemetry.snr_ratio) * (osc_height * 0.4);
                let wave_color = if telemetry.snr_warning { JAZZ_TEAL } else { TERMINAL_GREEN };
                let mut rng = rand::thread_rng();
                let segments = 30;
                let dx = osc_width / segments as f32;
                let mid_y = osc_rect.center().y;

                for i in 0..segments {
                    let x0 = osc_rect.min.x + i as f32 * dx;
                    let x1 = x0 + dx;
                    // Base sine wave + random jitter proportional to noise
                    let phase = time.elapsed_secs() * 3.0 + i as f32 * 0.5;
                    let base_y = phase.sin() * 2.0;
                    let noise = oscilloscope_jitter(&mut rng, jitter_amp);
                    let y0 = mid_y + base_y + noise;
                    let noise2 = oscilloscope_jitter(&mut rng, jitter_amp);
                    let y1 = mid_y + (phase + 0.5).sin() * 2.0 + noise2;
                    ui.painter().line_segment(
                        [egui::pos2(x0, y0), egui::pos2(x1, y1)],
                        egui::Stroke::new(1.0, wave_color),
                    );
                }

                // SNR text overlay
                let snr_pct = (telemetry.snr_ratio * 100.0) as u32;
                ui.painter().text(
                    egui::pos2(osc_rect.max.x - 2.0, osc_rect.min.y + 1.0),
                    egui::Align2::RIGHT_TOP,
                    format!("SNR:{}%", snr_pct),
                    egui::FontId::monospace(7.0),
                    wave_color,
                );

                ui.separator();

                // SNR warning status
                if telemetry.snr_warning {
                    let flash = (time.elapsed_secs() * 4.0).sin() > 0.0;
                    if flash {
                        ui.label(
                            egui::RichText::new(
                                format!("SENSORY_SATIATION: -{}%", telemetry.efficiency_loss_pct as u32)
                            )
                                .color(JAZZ_TEAL)
                                .size(8.0)
                                .monospace()
                                .strong()
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Opinion_Drift.exe")
                            .color(egui::Color32::BLACK)
                            .size(9.0)
                            .monospace()
                    );
                }

                // Engagement gate warning
                if telemetry.engagement_gated {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("REVENUE_GATED")
                            .color(egui::Color32::from_rgb(200, 50, 50))
                            .size(8.0)
                            .monospace()
                            .strong()
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let total_seconds = ui_state.clock_time as u32;
                    let hours = (total_seconds / 3600) % 24;
                    let minutes = (total_seconds / 60) % 60;
                    let am_pm = if hours >= 12 { "PM" } else { "AM" };
                    let display_hours = if hours % 12 == 0 { 12 } else { hours % 12 };
                    let clock_str = format!("{}:{:02} {}", display_hours, minutes, am_pm);

                    ui.label(
                        egui::RichText::new(clock_str)
                            .color(egui::Color32::BLACK)
                            .size(10.0)
                            .strong()
                            .monospace()
                    );
                });
            });
        });

    // ========================================
    // LEFT PANEL: Attention Zoning Controls
    // ========================================
    egui::SidePanel::left("controls_panel")
        .default_width(260.0)
        .frame(egui::Frame::NONE.fill(WIN_GRAY).inner_margin(egui::Margin::same(8)))
        .show(ctx, |ui| {
            // Terminal header
            ui.add_space(4.0);
            egui::Frame::NONE
                .fill(BG_BLACK)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("> NARRATIVE_ENGINE_ONLINE")
                            .color(TERMINAL_GREEN)
                            .size(10.0)
                            .monospace()
                    );
                });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("ATTENTION ZONING")
                    .color(egui::Color32::BLACK)
                    .size(10.0)
                    .strong()
            );
            ui.add_space(4.0);

            // Node type buttons — Win95 style
            for node_type in &[NodeType::EchoChamber, NodeType::PublicSquare, NodeType::DataRefinery] {
                let selected = placement.node_type == *node_type;
                let (icon_color, label) = match node_type {
                    NodeType::EchoChamber => (JAZZ_PURPLE, "Echo Chamber"),
                    NodeType::PublicSquare => (JAZZ_TEAL, "Public Square (Low ROI)"),
                    NodeType::DataRefinery => (RHETORIC_ORANGE, "Rage-Bait Node"),
                };

                let btn_fill = if selected {
                    egui::Color32::from_rgb(180, 180, 180)
                } else {
                    WIN_GRAY
                };

                egui::Frame::NONE
                    .fill(btn_fill)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        if ui.horizontal(|ui| {
                            // Colored square indicator
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0.0, icon_color);

                            ui.label(
                                egui::RichText::new(label)
                                    .color(egui::Color32::BLACK)
                                    .size(11.0)
                                    .strong()
                            );
                        }).response.interact(egui::Sense::click()).clicked() {
                            placement.node_type = *node_type;
                        }
                    });

                ui.add_space(2.0);
            }

            ui.add_space(12.0);

            // ========================================
            // Phase 2: Zone Painting Toolbar
            // ========================================
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("ZONE PAINTING")
                    .color(egui::Color32::BLACK)
                    .size(10.0)
                    .strong()
            );
            ui.add_space(4.0);

            // Zone type buttons
            for zone_type in &[ZoneType::EchoChamber, ZoneType::NeutralHub, ZoneType::DataRefinery] {
                let selected = zone_brush.active_zone == *zone_type;
                let (icon, label, color, cost) = match zone_type {
                    ZoneType::EchoChamber  => ("🏠", "Echo Chamber",  JAZZ_PURPLE, 200),
                    ZoneType::NeutralHub   => ("🏢", "Neutral Hub",   JAZZ_TEAL, 150),
                    ZoneType::DataRefinery => ("🏭", "Data Refinery", RHETORIC_ORANGE, 500),
                    _ => unreachable!(),
                };

                let btn_fill = if selected {
                    egui::Color32::from_rgb(200, 200, 200)
                } else {
                    egui::Color32::from_rgb(180, 180, 180)
                };

                egui::Frame::NONE
                    .fill(btn_fill)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        if ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0.0, color);
                            ui.label(
                                egui::RichText::new(format!("{} {} (${})" , icon, label, cost))
                                    .color(egui::Color32::BLACK)
                                    .size(10.0)
                                    .strong()
                            );
                        }).response.interact(egui::Sense::click()).clicked() {
                            zone_brush.active_zone = *zone_type;
                            zone_brush.is_painting = true;
                        }
                    });
                ui.add_space(1.0);
            }

            ui.add_space(8.0);

            // Brush radius slider
            ui.label(
                egui::RichText::new("BRUSH RADIUS")
                    .color(egui::Color32::BLACK)
                    .size(10.0)
                    .strong()
            );
            let mut radius_f32 = zone_brush.radius as f32;
            ui.add(egui::Slider::new(&mut radius_f32, 2.0..=24.0).integer());
            zone_brush.radius = radius_f32 as usize;

            ui.add_space(4.0);

            // Paint toggle
            let paint_text = if zone_brush.is_painting { "🎨 PAINTING ACTIVE" } else { "⊕ START PAINTING" };
            if ui.button(
                egui::RichText::new(paint_text)
                    .color(if zone_brush.is_painting { RHETORIC_ORANGE } else { egui::Color32::BLACK })
                    .size(11.0)
                    .strong()
            ).clicked() {
                zone_brush.is_painting = !zone_brush.is_painting;
            }

            // Consensus Trap audit warning
            if stats.is_under_audit {
                ui.add_space(8.0);
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(200, 0, 0))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("⚠ REGULATORY AUDIT")
                                .color(egui::Color32::WHITE)
                                .size(12.0)
                                .strong()
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Revenue below ${:.0} for {} quarters.\nBuild Echo Chambers or face dissolution.",
                                LICENSE_FEE, stats.audit_quarters
                            ))
                                .color(egui::Color32::from_rgb(255, 200, 200))
                                .size(9.0)
                        );
                    });
            }

            // ========================================
            // Phase 3: Election & Contract Panel
            // ========================================
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Active Party Mandate Banner
            if mandate.is_active() {
                let (seal_color, seal_label) = match mandate.party {
                    Party::Consensus => (JAZZ_TEAL, "⚖ CONSENSUS MANDATE"),
                    Party::Vanguard  => (egui::Color32::from_rgb(200, 50, 50), "⚔ VANGUARD MANDATE"),
                    _ => (WIN_GRAY, "NO MANDATE"),
                };
                egui::Frame::NONE
                    .fill(seal_color)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(seal_label)
                                .color(egui::Color32::WHITE)
                                .size(11.0)
                                .strong()
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{}\n{} quarters remaining",
                                mandate.party.description(),
                                mandate.quarters_remaining
                            ))
                                .color(egui::Color32::from_rgb(220, 220, 255))
                                .size(8.0)
                                .italics()
                        );
                    });
                ui.add_space(4.0);
            }

            // Active Government Contracts
            if !contracts.directives.is_empty() {
                ui.label(
                    egui::RichText::new("GOV CONTRACTS")
                        .color(egui::Color32::BLACK)
                        .size(10.0)
                        .strong()
                );
                for directive in &contracts.directives {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(30, 30, 40))
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&directive.name)
                                    .color(RHETORIC_ORANGE)
                                    .size(9.0)
                                    .strong()
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}\nDeadline: {} Qs | Reward: ${:.0}",
                                    directive.description,
                                    directive.deadline_quarters,
                                    directive.cash_reward
                                ))
                                    .color(egui::Color32::from_rgb(180, 180, 180))
                                    .size(8.0)
                                    .monospace()
                            );
                        });
                    ui.add_space(2.0);
                }
            }

            // Lobbying Button
            if stats.narrative_capital > 10.0 {
                ui.add_space(4.0);
                if ui.button(
                    egui::RichText::new(format!("🏛 LOBBY (10 NC → shift vote)"))
                        .color(egui::Color32::from_rgb(200, 160, 255))
                        .size(10.0)
                        .strong()
                ).clicked() {
                    election.lobby_nc_spent += 10.0;
                    // NC deduction happens in the economy system
                }
            }

            // ========================================
            // Shadow Infrastructure
            // ========================================
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Shadow Mode toggle
            let shadow_text = if shadow_mode.enabled { "👁 SHADOW MODE: ON" } else { "👁 SHADOW MODE: OFF" };
            let shadow_color = if shadow_mode.enabled {
                egui::Color32::from_rgb(180, 50, 255)
            } else {
                egui::Color32::BLACK
            };
            if ui.button(
                egui::RichText::new(shadow_text)
                    .color(shadow_color)
                    .size(11.0)
                    .strong()
            ).clicked() {
                shadow_mode.enabled = !shadow_mode.enabled;
            }

            // Public Trust meter
            ui.add_space(4.0);
            let trust_pct = (public_trust.value * 100.0) as u32;
            let trust_color = if public_trust.value > 0.6 {
                TERMINAL_GREEN
            } else if public_trust.value > 0.3 {
                RHETORIC_ORANGE
            } else {
                egui::Color32::from_rgb(255, 50, 50)
            };
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(20, 20, 20))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("PUBLIC TRUST: {}%", trust_pct))
                            .color(trust_color)
                            .size(10.0)
                            .strong()
                            .monospace()
                    );
                    // Trust bar
                    let bar_width = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_width, 6.0), egui::Sense::hover()
                    );
                    ui.painter().rect_filled(
                        rect, 0.0, egui::Color32::from_rgb(40, 40, 40)
                    );
                    let fill_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(bar_width * public_trust.value, 6.0)
                    );
                    ui.painter().rect_filled(fill_rect, 0.0, trust_color);
                });

            // ========================================
            // The Oracle
            // ========================================
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            if !oracle_state.active {
                if stats.narrative_capital >= oracle_state.unlock_cost {
                    ui.horizontal(|ui| {
                        if ui.button(
                            egui::RichText::new(format!("\u{1f52e} UNLOCK THE ORACLE ({:.0} NC)", oracle_state.unlock_cost))
                                .color(egui::Color32::from_rgb(255, 200, 50))
                                .size(11.0)
                                .strong()
                        ).clicked() {
                            oracle_state.active = true;
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new(format!("\u{1f512} THE ORACLE (requires {:.0} NC)", oracle_state.unlock_cost))
                            .color(egui::Color32::from_rgb(100, 100, 100))
                            .size(10.0)
                    );
                }
            } else {
                // Oracle is active — show optimization level + THERMAL MONITOR
                ui.horizontal(|ui| {
                    // === ORACLE THERMAL MONITOR ===
                    let therm_width = 16.0;
                    let therm_height = 60.0;
                    let (therm_rect, _) = ui.allocate_exact_size(
                        egui::vec2(therm_width, therm_height), egui::Sense::hover()
                    );
                    // Thermometer background
                    ui.painter().rect_filled(therm_rect, 2.0, egui::Color32::from_rgb(20, 20, 20));
                    // Mercury fill (from bottom)
                    let fill_height = therm_height * telemetry.oracle_heat;
                    let fill_rect = egui::Rect::from_min_size(
                        egui::pos2(therm_rect.min.x, therm_rect.max.y - fill_height),
                        egui::vec2(therm_width, fill_height),
                    );
                    ui.painter().rect_filled(fill_rect, 0.0, RHETORIC_ORANGE);
                    // Thermometer border
                    ui.painter().rect_stroke(therm_rect, 2.0, egui::Stroke::new(1.0, WIN_GRAY), egui::StrokeKind::Middle);

                    // Labels
                    ui.vertical(|ui| {
                        let opt = oracle_state.optimization_level;
                        let opt_color = egui::Color32::from_rgb(
                            (200.0 + 55.0 * opt) as u8,
                            (200.0 + 55.0 * opt) as u8,
                            (210.0 + 45.0 * opt) as u8,
                        );
                        ui.label(
                            egui::RichText::new("\u{1f52e} ORACLE ACTIVE")
                                .color(opt_color)
                                .size(11.0)
                                .strong()
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Optimization: {:.0}%\nThermal: {:.0}%",
                                opt * 100.0,
                                telemetry.oracle_heat * 100.0,
                            ))
                                .color(opt_color)
                                .size(8.0)
                                .monospace()
                        );

                        // Smoke particles (randomized chars when oracle_smoking)
                        if telemetry.oracle_smoking {
                            let mut rng = rand::thread_rng();
                            let smoke_chars: String = (0..12)
                                .map(|_| {
                                    let chars = ['.', '·', '°', '•', '░', '▒', '▓'];
                                    chars[rng.gen_range(0..chars.len())]
                                })
                                .collect();
                            ui.label(
                                egui::RichText::new(smoke_chars)
                                    .color(egui::Color32::from_rgba_premultiplied(255, 140, 0, 120))
                                    .size(8.0)
                                    .monospace()
                            );
                        }

                        // Friction warning
                        if telemetry.oracle_friction_warning {
                            ui.label(
                                egui::RichText::new("SYSTEMIC_FRICTION")
                                    .color(egui::Color32::from_rgb(255, 50, 50))
                                    .size(8.0)
                                    .monospace()
                                    .strong()
                            );
                        }
                    });
                });
            }

            ui.add_space(12.0);

            // Narrative target slider
            ui.label(
                egui::RichText::new("NARRATIVE FREQUENCY")
                    .color(egui::Color32::BLACK)
                    .size(10.0)
                    .strong()
            );
            ui.add(
                egui::Slider::new(&mut placement.narrative_target, 0.0..=1.0)
                    .custom_formatter(|v, _| {
                        if v < 0.3 { format!("{:.2} PROGRESSIVE", v) }
                        else if v > 0.7 { format!("{:.2} CONSERVATIVE", v) }
                        else { format!("{:.2} CENTRIST", v) }
                    })
            );

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("BROADCAST INTENSITY")
                    .color(egui::Color32::BLACK)
                    .size(10.0)
                    .strong()
            );
            ui.add(egui::Slider::new(&mut placement.intensity, 0.1..=1.0));

            ui.add_space(12.0);

            // Place button
            let can_afford = stats.cash >= placement.node_type.cost();
            let button_text = if placement.active {
                "✕ CANCEL"
            } else if can_afford {
                "⊕ DEPLOY NODE"
            } else {
                "⊘ INSUFFICIENT_FUNDS"
            };

            if ui.add_enabled(
                can_afford || placement.active,
                egui::Button::new(
                    egui::RichText::new(button_text)
                        .color(if placement.active { RHETORIC_ORANGE } else { egui::Color32::BLACK })
                        .size(12.0)
                        .strong()
                )
            ).clicked() {
                placement.active = !placement.active;
            }

            ui.add_space(16.0);

            let mandate_text = if ui_state.is_glitching {
                &ui_state.corporate_message
            } else {
                generate_mandate(&stats)
            };

            // Active mandate box (social commentary)
            egui::Frame::NONE
                .fill(if ui_state.is_glitching { egui::Color32::from_rgb(255, 0, 0) } else { JAZZ_PURPLE })
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("URGENT CONTRACT")
                            .color(egui::Color32::WHITE)
                            .size(10.0)
                            .strong()
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(mandate_text)
                            .color(egui::Color32::from_rgb(220, 200, 255))
                            .size(10.0)
                            .italics()
                    );
                });

            // Bottom tagline
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(210, 210, 210))
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("COMMODIFIED_SOUPS v.96\nPROFIT IS THE ONLY METRIC.")
                                .color(egui::Color32::BLACK)
                                .size(8.0)
                                .monospace()
                        );
                    });
                ui.label(
                    egui::RichText::new("Right-click drag: Pan | Scroll: Zoom")
                        .color(egui::Color32::from_rgb(80, 80, 80))
                        .size(8.0)
                );
            });
        });

    // ========================================
    // BOTTOM PANEL: Stats Dashboard (Win95 inset boxes)
    // ========================================
    egui::TopBottomPanel::bottom("stats_dashboard")
        .frame(egui::Frame::NONE.fill(WIN_GRAY).inner_margin(egui::Margin::same(6)))
        .show(ctx, |ui| {
            ui.columns(5, |cols| {
                // Quarterly Yield
                stat_inset(&mut cols[0], "QUARTERLY YIELD",
                    &format!("${:.0}", stats.quarterly_revenue),
                    TERMINAL_GREEN,
                    &format!("Q{} | GROWTH: {}",
                        stats.quarter,
                        if stats.prev_quarterly_revenue > 0.0 {
                            let g = ((stats.quarterly_revenue - stats.prev_quarterly_revenue) / stats.prev_quarterly_revenue * 100.0) as i32;
                            format!("{:+}%", g)
                        } else { "N/A".to_string() }
                    ),
                );

                // Polarization Heat
                let pol_label = if stats.polarization_heat > 0.35 { "CRITICAL" }
                    else if stats.polarization_heat > 0.2 { "ELEVATED" }
                    else { "NOMINAL" };
                stat_inset(&mut cols[1], "POLARIZATION HEAT",
                    pol_label, RHETORIC_ORANGE,
                    &format!("SOCIAL_BOND: FRAX: {:.2}", stats.social_cohesion),
                );

                // === VITALITY METER (replaces Public Apathy) ===
                let vit_ui = &mut cols[2];
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(26, 26, 26))
                    .inner_margin(egui::Margin::same(8))
                    .show(vit_ui, |ui| {
                        let status_text = if telemetry.engagement_gated {
                            "INSUFFICIENT_COGNITIVE_RESONANCE"
                        } else if telemetry.vitality_segments <= 5 {
                            "DEGRADED"
                        } else {
                            "NOMINAL"
                        };
                        let status_color = if telemetry.engagement_gated {
                            egui::Color32::from_rgb(255, 50, 50)
                        } else if telemetry.vitality_segments <= 5 {
                            RHETORIC_ORANGE
                        } else {
                            TERMINAL_GREEN
                        };
                        ui.label(
                            egui::RichText::new("VITALITY")
                                .color(egui::Color32::from_rgb(128, 128, 128))
                                .size(9.0)
                                .strong()
                        );
                        ui.label(
                            egui::RichText::new(status_text)
                                .color(status_color)
                                .size(12.0)
                                .strong()
                        );
                        // Segmented bar: 10 segments
                        let bar_width = ui.available_width();
                        let seg_width = bar_width / 10.0;
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_width, 8.0), egui::Sense::hover()
                        );
                        for i in 0..10u32 {
                            let x = bar_rect.min.x + i as f32 * seg_width;
                            let seg_rect = egui::Rect::from_min_size(
                                egui::pos2(x + 1.0, bar_rect.min.y),
                                egui::vec2(seg_width - 2.0, 8.0),
                            );
                            let fill = if i < telemetry.vitality_segments {
                                status_color
                            } else {
                                egui::Color32::from_rgb(40, 40, 40)
                            };
                            ui.painter().rect_filled(seg_rect, 0.0, fill);
                        }
                        ui.label(
                            egui::RichText::new(
                                format!("ENG: {:.0}%", stats.engagement_index * 100.0)
                            )
                                .color(egui::Color32::from_rgb(100, 100, 100))
                                .size(8.0)
                                .monospace()
                        );
                    });

                // Consensus Status
                let consensus = if stats.polarization_heat > 0.4 { "NARRATIVE FAILURE" }
                    else if stats.mean_opinion > 0.6 || stats.mean_opinion < 0.4 { "DRIFTING" }
                    else { "CONTROLLED" };
                let consensus_note = if stats.polarization_heat > 0.4 {
                    "UNAUTHORIZED_THOUGHTS_DETECTED"
                } else {
                    "WITHIN_PARAMETERS"
                };
                stat_inset(&mut cols[3], "CONSENSUS STATUS",
                    consensus, egui::Color32::WHITE,
                    consensus_note,
                );

                // Narrative Capital
                stat_inset(&mut cols[4], "NARRATIVE CAPITAL",
                    &format!("{:.0} NC", stats.narrative_capital),
                    egui::Color32::from_rgb(200, 160, 255),
                    if stats.is_under_audit { "AUDIT_IN_PROGRESS" }
                    else if stats.narrative_capital > 100.0 { "UPGRADES_AVAILABLE" }
                    else { "HARVESTING_DATA" },
                );
            });
        });

    // ========================================
    // UPDATE WIZARD OVERLAY (GamePhase transitions)
    // Win95 "installer" modal with progress bar
    // ========================================
    if telemetry.wizard_active {
        let module_name = crate::telemetry::wizard_module_name(telemetry.wizard_target_phase);
        let status_text = crate::telemetry::wizard_status_text(telemetry.wizard_target_phase);

        egui::Window::new("BROADCAST_OS Update Wizard")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_size(egui::vec2(380.0, 180.0))
            .frame(egui::Frame::window(&ctx.style())
                .fill(WIN_GRAY)
                .inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("Installing: {}.exe", module_name))
                        .color(egui::Color32::BLACK)
                        .size(13.0)
                        .strong()
                );
                ui.add_space(8.0);

                // Status text
                ui.label(
                    egui::RichText::new(status_text)
                        .color(egui::Color32::from_rgb(40, 40, 60))
                        .size(10.0)
                        .monospace()
                );
                ui.add_space(12.0);

                // Progress bar (Win95 style — segmented blue blocks)
                let bar_width = ui.available_width();
                let bar_height = 20.0;
                let (bar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_width, bar_height), egui::Sense::hover()
                );
                // Background (sunken)
                ui.painter().rect_filled(bar_rect, 0.0, egui::Color32::WHITE);
                ui.painter().rect_stroke(bar_rect, 0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(128, 128, 128)),
                    egui::StrokeKind::Middle,
                );

                // Blue progress blocks
                let filled_width = bar_width * telemetry.wizard_progress;
                let block_size = 12.0;
                let blocks = (filled_width / (block_size + 2.0)) as u32;
                for i in 0..blocks {
                    let x = bar_rect.min.x + 2.0 + i as f32 * (block_size + 2.0);
                    let block_rect = egui::Rect::from_min_size(
                        egui::pos2(x, bar_rect.min.y + 2.0),
                        egui::vec2(block_size, bar_height - 4.0),
                    );
                    ui.painter().rect_filled(block_rect, 0.0,
                        egui::Color32::from_rgb(0, 0, 128) // Win95 blue
                    );
                }

                ui.add_space(8.0);

                // Percentage
                let pct = (telemetry.wizard_progress * 100.0) as u32;
                ui.label(
                    egui::RichText::new(format!("{}% Complete", pct))
                        .color(egui::Color32::BLACK)
                        .size(10.0)
                        .monospace()
                );
            });
    }

    // === ORACLE FRICTION WARNING MODAL ===
    if telemetry.oracle_friction_warning && oracle_state.active {
        egui::Window::new("⚠ SYSTEM NOTICE")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .default_size(egui::vec2(360.0, 100.0))
            .frame(egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgb(255, 250, 230))
                .inner_margin(egui::Margin::same(12)))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "NOTICE: UNATTENDED OPTIMIZATION IS CAUSING\n\
                         SYSTEMIC FRICTION. INTERVENE OR ACCEPT DATA LOSS."
                    )
                        .color(egui::Color32::from_rgb(180, 60, 0))
                        .size(11.0)
                        .strong()
                        .monospace()
                );
            });
    }
}

/// Separate system for HUMINT profiler windows (keeps dashboard_ui under Bevy's tuple limit).
pub fn humint_profiler_ui(
    mut contexts: EguiContexts,
    mut sim_selection: ResMut<SimSelection>,
    agents: Query<(Entity, &SimAgent, &Transform)>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();

    // ========================================
    // HUMINT PROFILER WINDOWS (Surveillance Aesthetic)
    // Up to 3 pinned agent profiles
    // ========================================
    let pinned_snapshot: Vec<_> = sim_selection.pinned.clone();
    let mut to_unpin = Vec::new();

    for (idx, pinned) in pinned_snapshot.iter().enumerate() {
        // Look up the agent
        let agent_data = agents.iter().find(|(e, _, _)| *e == pinned.entity);
        let Some((_entity, agent, transform)) = agent_data else {
            to_unpin.push(pinned.entity);
            continue;
        };

        // Generate life data
        let life = generate_life_data(&HumintInput {
            agent_id: pinned.agent_id,
            opinion: agent.opinion,
            engagement: agent.attention,
            zone: ZoneType::NeutralHub, // TODO: look up actual zone from influence map
            personhood: agent.personhood,
        });

        let window_offset = egui::vec2(20.0 + idx as f32 * 30.0, 60.0 + idx as f32 * 30.0);
        let win_id = format!("humint_profile_{}", pinned.agent_id);

        // Surveillance green
        let surv_green = egui::Color32::from_rgb(0, 200, 65);
        let surv_dim = egui::Color32::from_rgb(0, 120, 40);
        let surv_bg = egui::Color32::from_rgb(8, 12, 8);

        let mut is_open = true;

        egui::Window::new(format!("HUMINT_PROFILE_ASSET_{:04}", pinned.agent_id))
            .id(egui::Id::new(&win_id))
            .open(&mut is_open)
            .collapsible(true)
            .resizable(false)
            .default_pos(egui::pos2(400.0 + window_offset.x, 100.0 + window_offset.y))
            .default_size(egui::vec2(280.0, 240.0))
            .frame(egui::Frame::window(&ctx.style())
                .fill(surv_bg)
                .inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                // === Header ===
                ui.label(
                    egui::RichText::new(format!("╔══ {} ══╗", life.name))
                        .color(if life.is_erased { egui::Color32::RED } else { surv_green })
                        .size(12.0)
                        .monospace()
                        .strong()
                );
                ui.add_space(4.0);

                // === Fields ===
                let field_color = if life.is_erased { egui::Color32::from_rgb(180, 0, 0) } else { surv_dim };
                let value_color = if life.is_erased {
                    let mut rng = rand::thread_rng();
                    let g: u8 = rng.gen_range(100..255);
                    egui::Color32::from_rgb(g, 0, 0)
                } else {
                    surv_green
                };

                for (label, value) in [
                    ("OCCUPATION", life.occupation.as_str()),
                    ("LAST_SEARCH", life.search_query.as_str()),
                    ("POLITICAL",   life.political_lean.as_str()),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}: ", label))
                                .color(field_color)
                                .size(9.0)
                                .monospace()
                        );
                        ui.label(
                            egui::RichText::new(value)
                                .color(value_color)
                                .size(9.0)
                                .monospace()
                        );
                    });
                }

                ui.add_space(6.0);

                // === Personhood Bar ===
                ui.label(
                    egui::RichText::new(format!("PERSONHOOD: {}%", life.personhood_pct))
                        .color(if life.is_erased { egui::Color32::RED } else { surv_green })
                        .size(10.0)
                        .monospace()
                        .strong()
                );
                let bar_w = ui.available_width();
                let bar_h = 6.0;
                let (bar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(bar_w, bar_h), egui::Sense::hover()
                );
                ui.painter().rect_filled(bar_rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
                let fill_pct = agent.personhood.clamp(0.0, 1.0);
                let fill_color = if fill_pct < 0.2 {
                    egui::Color32::RED
                } else if fill_pct < 0.5 {
                    RHETORIC_ORANGE
                } else {
                    surv_green
                };
                let fill_rect = egui::Rect::from_min_size(
                    bar_rect.min,
                    egui::vec2(bar_w * fill_pct, bar_h),
                );
                ui.painter().rect_filled(fill_rect, 0.0, fill_color);

                ui.add_space(4.0);

                // === Heartbeat Line ===
                let hb_w = ui.available_width();
                let hb_h = 20.0;
                let (hb_rect, _) = ui.allocate_exact_size(
                    egui::vec2(hb_w, hb_h), egui::Sense::hover()
                );
                ui.painter().rect_filled(hb_rect, 0.0, egui::Color32::from_rgb(5, 8, 5));

                let segments = 40;
                let dx = hb_w / segments as f32;
                let mid_y = hb_rect.center().y;

                if life.is_erased {
                    // Flatline
                    ui.painter().line_segment(
                        [egui::pos2(hb_rect.min.x, mid_y), egui::pos2(hb_rect.max.x, mid_y)],
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 0, 0)),
                    );
                } else {
                    // Heartbeat: amplitude proportional to personhood
                    let amp = hb_h * 0.35 * fill_pct;
                    for i in 0..segments {
                        let x0 = hb_rect.min.x + i as f32 * dx;
                        let x1 = x0 + dx;
                        let t0 = time.elapsed_secs() * 2.0 + i as f32 * 0.3;
                        let t1 = t0 + 0.3;
                        let ecg = |t: f32| -> f32 {
                            let phase = (t % 3.0) / 3.0;
                            if phase < 0.1 { (phase / 0.1 * std::f32::consts::PI).sin() * amp }
                            else if phase < 0.2 { -((phase - 0.1) / 0.1 * std::f32::consts::PI).sin() * amp * 0.5 }
                            else { 0.0 }
                        };
                        let y0 = mid_y - ecg(t0);
                        let y1 = mid_y - ecg(t1);
                        ui.painter().line_segment(
                            [egui::pos2(x0, y0), egui::pos2(x1, y1)],
                            egui::Stroke::new(1.0, surv_green),
                        );
                    }
                }

                // === Position ===
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "POS: ({:.0}, {:.0})",
                        transform.translation.x, transform.translation.y
                    ))
                        .color(surv_dim)
                        .size(8.0)
                        .monospace()
                );
            });

        if !is_open {
            to_unpin.push(pinned.entity);
        }
    }

    // Clean up closed/despawned pins
    for entity in to_unpin {
        sim_selection.unpin(entity);
    }
}

/// Renders the epilogue "mirror reveal" and EXPORT_MANIFESTO button during singularity.
/// Separate system to keep dashboard_ui under Bevy's tuple limit.
pub fn singularity_epilogue_ui(
    mut contexts: EguiContexts,
    singularity: Res<SingularityState>,
    context: Res<crate::interop::RealWorldContext>,
    history: Res<crate::oracle::SessionHistory>,
    election: Res<ElectionState>,
) {
    if !singularity.triggered {
        return;
    }

    let ctx = contexts.ctx_mut();
    let screen_rect = ctx.screen_rect();

    // Position below the main singularity text
    let mirror_rect = egui::Rect::from_center_size(
        egui::pos2(screen_rect.center().x, screen_rect.center().y + 160.0),
        egui::vec2(520.0, 220.0),
    );

    egui::Area::new(egui::Id::new("singularity_mirror"))
        .fixed_pos(mirror_rect.min)
        .show(ctx, |ui| {
            ui.set_max_size(mirror_rect.size());
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);

                // Mirror reveal lines
                let mirror_lines = crate::interop::generate_mirror_lines(&context);
                for line in &mirror_lines {
                    ui.label(
                        egui::RichText::new(line)
                            .color(egui::Color32::from_rgb(80, 80, 90))
                            .size(11.0)
                            .monospace()
                    );
                    ui.add_space(2.0);
                }

                ui.add_space(16.0);

                // EXPORT_MANIFESTO.TXT button
                let btn = ui.add(
                    egui::Button::new(
                        egui::RichText::new("▸ EXPORT_MANIFESTO.TXT")
                            .size(13.0)
                            .monospace()
                            .strong()
                    )
                    .fill(egui::Color32::from_rgb(230, 230, 235))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 110)))
                );

                if btn.clicked() {
                    let winning_party = match election.projected_winner {
                        Party::Vanguard => "The Vanguard",
                        Party::Consensus => "The Consensus Party",
                        Party::NoMandate => "No Mandate",
                    };
                    let sing_type = match singularity.singularity_type {
                        SingularityType::TotalConsensus => "Total Consensus",
                        SingularityType::TotalPolarization => "Total Polarization",
                        _ => "Singularity",
                    };
                    let manifesto = crate::interop::generate_manifesto(
                        &history,
                        &context,
                        winning_party,
                        sing_type,
                    );
                    crate::interop::trigger_manifesto_download(&manifesto);
                }
            });
        });
}

/// System: Renders the zoom indicator overlay and game legend panel.
/// Separate from dashboard_ui to stay under Bevy's tuple limit.
pub fn game_overlay_ui(
    mut contexts: EguiContexts,
    ui_state: Res<UiState>,
    cam_state: Res<crate::render::CameraState>,
    singularity: Res<SingularityState>,
) {
    if singularity.triggered {
        return;
    }

    let ctx = contexts.ctx_mut();
    let screen_rect = ctx.screen_rect();

    // === ZOOM INDICATOR ===
    let zoom_opacity = zoom_indicator_opacity(cam_state.zoom);
    if zoom_opacity > 0.01 {
        let alpha = (zoom_opacity * 180.0) as u8;
        egui::Area::new(egui::Id::new("zoom_indicator"))
            .fixed_pos(egui::pos2(screen_rect.max.x - 200.0, screen_rect.max.y - 60.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_premultiplied(10, 10, 12, alpha))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("🔍 Scroll to zoom out")
                                .color(egui::Color32::from_rgba_premultiplied(0, 169, 157, alpha))
                                .size(11.0)
                                .monospace()
                        );
                    });
            });
    }

    // === GAME LEGEND PANEL ===
    if ui_state.show_legend {
        egui::Window::new(
            egui::RichText::new("BROADCAST_OS // FIELD MANUAL")
                .strong()
                .size(11.0)
        )
            .id(egui::Id::new("game_legend"))
            .fixed_pos(egui::pos2(screen_rect.max.x - 280.0, 42.0))
            .fixed_size(egui::vec2(260.0, 0.0))
            .title_bar(true)
            .collapsible(false)
            .show(ctx, |ui| {
                let entries = legend_entries();
                for (idx, entry) in entries.iter().enumerate() {
                    if idx > 0 {
                        ui.separator();
                    }
                    ui.label(
                        egui::RichText::new(entry.category)
                            .color(egui::Color32::from_rgb(192, 192, 192))
                            .size(10.0)
                            .strong()
                    );
                    ui.add_space(2.0);
                    for (label, rgb) in &entry.items {
                        ui.horizontal(|ui| {
                            // Color swatch
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(10.0, 10.0), egui::Sense::hover()
                            );
                            ui.painter().rect_filled(
                                rect, 0.0,
                                egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                            );
                            ui.label(
                                egui::RichText::new(*label)
                                    .color(egui::Color32::from_rgb(180, 180, 180))
                                    .size(9.0)
                                    .monospace()
                            );
                        });
                    }
                    ui.add_space(4.0);
                }

                // Mechanics blurb
                ui.separator();
                ui.label(
                    egui::RichText::new("Mechanics")
                        .color(egui::Color32::from_rgb(192, 192, 192))
                        .size(10.0)
                        .strong()
                );
                ui.add_space(2.0);
                for line in [
                    "Agents drift toward nearby opinions",
                    "Media nodes attract attention",
                    "Attention decays with information overload",
                    "Revenue scales with engagement",
                ] {
                    ui.label(
                        egui::RichText::new(format!("· {}", line))
                            .color(egui::Color32::from_rgb(140, 140, 140))
                            .size(8.0)
                            .monospace()
                    );
                }
            });
    }
}

/// Render a Win95-inset stat box
fn stat_inset(ui: &mut egui::Ui, title: &str, value: &str, value_color: egui::Color32, subtitle: &str) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(26, 26, 26))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .color(egui::Color32::from_rgb(128, 128, 128))
                    .size(9.0)
                    .strong()
            );
            ui.label(
                egui::RichText::new(value)
                    .color(value_color)
                    .size(16.0)
                    .strong()
                    .italics()
            );
            ui.label(
                egui::RichText::new(subtitle)
                    .color(egui::Color32::from_rgb(100, 100, 100))
                    .size(8.0)
                    .monospace()
            );
        });
}

/// Compute the opacity of the zoom indicator hint.
///
/// Visible when zoomed in (zoom < 0.85). Fades linearly from 0.0 at zoom=0.85
/// to 1.0 at zoom=0.2. Returns 0.0 when at default zoom or zoomed out.
///
/// **Pure function** — fully testable.
pub fn zoom_indicator_opacity(zoom: f32) -> f32 {
    if zoom >= 0.85 {
        0.0
    } else {
        // Linear fade: 0.0 at zoom=0.85, 1.0 at zoom=0.2
        ((0.85 - zoom) / (0.85 - 0.2)).clamp(0.0, 1.0)
    }
}

/// Entry in the game legend panel.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendEntry {
    pub category: &'static str,
    pub items: Vec<(&'static str, [u8; 3])>, // (label, RGB color)
}

/// Generate the legend entries for the game key.
///
/// **Pure function** — returns the complete legend data.
pub fn legend_entries() -> Vec<LegendEntry> {
    vec![
        LegendEntry {
            category: "Agent Colors",
            items: vec![
                ("Consensus (center)",  [0, 169, 157]),    // Jazz Teal
                ("Polarized",           [117, 59, 189]),   // Jazz Purple
                ("Outraged (extreme)",  [255, 140, 0]),    // Sunset Orange
            ],
        },
        LegendEntry {
            category: "Node Types",
            items: vec![
                ("Echo Chamber",   [117, 59, 189]),   // Purple
                ("Public Square",  [0, 169, 157]),    // Teal
                ("Data Refinery",  [255, 140, 0]),    // Orange
            ],
        },
        LegendEntry {
            category: "Controls",
            items: vec![
                ("Scroll = Zoom",         [192, 192, 192]),
                ("Right-drag = Pan",      [192, 192, 192]),
                ("Left-click = Place",    [192, 192, 192]),
            ],
        },
    ]
}

/// Compute oscilloscope jitter noise for a given amplitude.
///
/// Used by the bandwidth oscilloscope in the taskbar. The amplitude is
/// derived from `(1.0 - snr_ratio)`, so when signal is pristine (snr = 1.0)
/// the amplitude is 0.0.
fn oscilloscope_jitter(rng: &mut impl rand::Rng, amp: f32) -> f32 {
    if amp > f32::EPSILON {
        rng.gen_range(-amp..amp)
    } else {
        0.0
    }
}

/// Generate context-sensitive "contract" text (social commentary)
fn generate_mandate(stats: &GlobalStats) -> &'static str {
    if stats.apathy_rate > 0.5 {
        "Client: State Dept.\nGoal: Maintain current\ndisengagement levels.\nStatus: MISSION COMPLETE."
    } else if stats.polarization_heat > 0.35 {
        "Client: Defense PAC\nGoal: Neutralize public\nsentiment re: Policy 902-B.\nDeadline: EOD."
    } else if stats.node_count == 0 {
        "No active contracts.\nDeploy infrastructure to\nattract government clients.\n\n// They will come."
    } else if stats.social_cohesion < 0.4 {
        "Client: Private Equity\nGoal: Suppress labor\norganization discourse.\nBudget: UNLIMITED."
    } else {
        "Client: Senate Cmte.\nGoal: Shape narrative on\nupcoming appropriations vote.\nClassification: ROUTINE."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Oscilloscope Jitter Tests ===

    #[test]
    fn test_oscilloscope_jitter_zero_amplitude_no_panic() {
        let mut rng = rand::thread_rng();
        let result = oscilloscope_jitter(&mut rng, 0.0);
        assert_eq!(result, 0.0, "Zero amplitude should produce zero jitter");
    }

    #[test]
    fn test_oscilloscope_jitter_tiny_amplitude_no_panic() {
        let mut rng = rand::thread_rng();
        let result = oscilloscope_jitter(&mut rng, f32::EPSILON / 2.0);
        assert!(result.abs() <= f32::EPSILON, "Sub-epsilon amplitude should produce near-zero jitter");
    }

    #[test]
    fn test_oscilloscope_jitter_positive_amplitude_in_bounds() {
        let mut rng = rand::thread_rng();
        let amp = 5.0;
        for _ in 0..100 {
            let j = oscilloscope_jitter(&mut rng, amp);
            assert!(j >= -amp && j < amp,
                "Jitter {} should be in [-{}, {})", j, amp, amp);
        }
    }

    #[test]
    fn test_oscilloscope_jitter_negative_amplitude_no_panic() {
        let mut rng = rand::thread_rng();
        let _ = oscilloscope_jitter(&mut rng, -1.0);
    }

    // === Zoom Indicator Tests (TDD RED → GREEN) ===

    #[test]
    fn test_zoom_indicator_hidden_at_default() {
        let opacity = zoom_indicator_opacity(1.0);
        assert!((opacity - 0.0).abs() < 0.001,
            "Default zoom should hide indicator. Got {}", opacity);
    }

    #[test]
    fn test_zoom_indicator_hidden_when_zoomed_out() {
        let opacity = zoom_indicator_opacity(2.0);
        assert!((opacity - 0.0).abs() < 0.001,
            "Zoomed out should hide indicator. Got {}", opacity);
    }

    #[test]
    fn test_zoom_indicator_visible_when_zoomed_in() {
        let opacity = zoom_indicator_opacity(0.5);
        assert!(opacity > 0.3,
            "Zoomed in to 0.5 should show indicator. Got {}", opacity);
    }

    #[test]
    fn test_zoom_indicator_max_at_min_zoom() {
        let opacity = zoom_indicator_opacity(0.2);
        assert!((opacity - 1.0).abs() < 0.01,
            "At min zoom, indicator should be fully visible. Got {}", opacity);
    }

    #[test]
    fn test_zoom_indicator_clamps_to_one() {
        // Even below min zoom, should not exceed 1.0
        let opacity = zoom_indicator_opacity(0.05);
        assert!(opacity <= 1.0,
            "Opacity should never exceed 1.0. Got {}", opacity);
    }

    // === Legend Tests (TDD RED → GREEN) ===

    #[test]
    fn test_legend_has_agent_colors_section() {
        let entries = legend_entries();
        assert!(entries.iter().any(|e| e.category == "Agent Colors"),
            "Legend should have an 'Agent Colors' section");
    }

    #[test]
    fn test_legend_has_controls_section() {
        let entries = legend_entries();
        assert!(entries.iter().any(|e| e.category == "Controls"),
            "Legend should have a 'Controls' section");
    }

    #[test]
    fn test_legend_has_node_types_section() {
        let entries = legend_entries();
        assert!(entries.iter().any(|e| e.category == "Node Types"),
            "Legend should have a 'Node Types' section");
    }

    #[test]
    fn test_legend_agent_colors_has_three_stops() {
        let entries = legend_entries();
        let colors = entries.iter().find(|e| e.category == "Agent Colors").unwrap();
        assert!(colors.items.len() >= 3,
            "Agent colors should have at least 3 stops. Got {}", colors.items.len());
    }

    #[test]
    fn test_legend_toggle_state() {
        let mut state = UiState::default();
        assert!(!state.show_legend, "Legend should be hidden by default");
        state.show_legend = true;
        assert!(state.show_legend, "Legend should be visible after toggle");
    }
}
