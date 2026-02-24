//! SimSitting — Fourth Wall Interop Layer
//!
//! Bridges the browser sandbox to expose the player's real-world context:
//! local time, timezone, locale, platform. On native (tests/dev), uses
//! system clock with sensible defaults.
//!
//! ## Architecture
//!
//! All JS calls are `#[cfg(target_arch = "wasm32")]`.
//! All generators (`generate_greeting`, `generate_creepy_email`,
//! `generate_manifesto`) are **pure functions** — testable on any platform.
//!
//! ## The Thesis
//!
//! The mechanisms of control don't stop at the edge of the browser window.

use bevy::prelude::*;
use crate::oracle::{SessionHistory, PsychographicProfile, build_profile};

// ============================================================================
// Real-World Context Resource
// ============================================================================

/// Browser-derived psychographic data injected into the game's narrative engine.
///
/// On wasm: populated from `Intl.DateTimeFormat`, `navigator.language`, etc.
/// On native: populated from system clock with fallback defaults.
#[derive(Resource, Clone, Debug)]
pub struct RealWorldContext {
    /// Local hour (0–23)
    pub local_hour: u32,
    /// True when 22..=4 — the Architect should be sleeping
    pub is_late_night: bool,
    /// IANA timezone string, e.g. "America/Los_Angeles"
    pub timezone: String,
    /// Browser locale, e.g. "en-US"
    pub detected_locale: String,
    /// Platform string, e.g. "MacIntel", "Win32", "Linux x86_64"
    pub platform: String,
    /// Whether the display supports HDR
    pub is_hdr: bool,
    /// Whether it's "evening" (17–21) or "late night" (22–4)
    pub is_evening: bool,
    /// Minutes since session started
    pub session_minutes: u32,
    /// Epoch seconds when the session started
    pub session_start: f64,
}

impl Default for RealWorldContext {
    fn default() -> Self {
        Self {
            local_hour: 18, // 6 PM — the canonical SimSitting hour
            is_late_night: false,
            timezone: "America/Los_Angeles".to_string(),
            detected_locale: "en-US".to_string(),
            platform: "Unknown".to_string(),
            is_hdr: false,
            is_evening: true,
            session_minutes: 0,
            session_start: 0.0,
        }
    }
}

// ============================================================================
// Pure Functions (testable on all platforms)
// ============================================================================

/// Generate a time-appropriate greeting for the Architect.
///
/// Four granular branches — no dead zones where the illusion cracks.
pub fn generate_greeting(hour: u32) -> &'static str {
    match hour {
        5..=11  => "Good morning, Architect. The system requires your early focus.",
        12..=16 => "Midday telemetry confirms peak localized apathy in your sector.",
        17..=21 => "Good evening. The daylight is fading in your sector.",
        _       => "Architect, it's late. Why are you still optimizing?",
    }
}

/// Generate a fourth-wall-breaking email from "Jenna S." using the player's
/// real-world context.
///
/// **Pure function** — deterministic from inputs.
pub fn generate_creepy_email(
    context: &RealWorldContext,
    optimization_level: f32,
    engagement_index: f32,
) -> String {
    let greeting = generate_greeting(context.local_hour);

    let locale_line = format!(
        "Our telemetry suggests your locale ({}) is reaching peak apathetic resonance.",
        context.detected_locale,
    );

    let timezone_line = format!(
        "The transition to 6:48 PM in {} must be handled personally.",
        context.timezone,
    );

    let optimization_note = if optimization_level > 0.7 {
        "\n\nNOTE: Your optimization_level exceeds safety thresholds. \
         The Oracle is making decisions you haven't reviewed."
    } else if optimization_level > 0.3 {
        "\n\nThe Oracle's influence is growing. Monitor your delegation metrics."
    } else {
        ""
    };

    let engagement_note = if engagement_index < 0.3 {
        "\n\nWARNING: Cognitive resonance has dropped below viability. \
         Your subjects are looking through the screen."
    } else {
        ""
    };

    let session_note = if context.session_minutes > 30 {
        format!(
            "\n\nYou have been managing this sector for {} minutes. \
             When was the last time you looked away from the screen?",
            context.session_minutes,
        )
    } else {
        String::new()
    };

    format!(
        "{}\n\n{}\n{}{}{}{}\n\n— Jenna S.\n   Compliance Division\n   BROADCAST_OS Corp.",
        greeting,
        locale_line,
        timezone_line,
        optimization_note,
        engagement_note,
        session_note,
    )
}

/// Generate the "mirror reveal" lines for the epilogue — weaving the player's
/// real-world data into the performance review.
pub fn generate_mirror_lines(context: &RealWorldContext) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!(
        "You managed this sector from a {} device.",
        context.platform,
    ));

    lines.push(format!(
        "You optimized for {} minutes while the sun set in {}.",
        context.session_minutes,
        context.timezone,
    ));

    let time_str = format!("{}:{:02}",
        if context.local_hour == 0 { 12 }
        else if context.local_hour > 12 { context.local_hour - 12 }
        else { context.local_hour },
        0, // We don't track minutes within the hour
    );
    let ampm = if context.local_hour >= 12 { "PM" } else { "AM" };

    lines.push(format!(
        "You chose to ignore the Personhood flatlines at {} {}.",
        time_str, ampm,
    ));

    if context.is_late_night {
        lines.push(
            "The system notes that you continued operating past midnight. \
             This will be reflected in your file.".to_string()
        );
    }

    if context.is_hdr {
        lines.push(
            "Your high-dynamic-range display rendered the erasure \
             in exquisite detail.".to_string()
        );
    }

    lines
}

/// Generate the downloadable EXPORT_MANIFESTO.TXT content.
///
/// **Pure function** — the complete psychographic profile of the player's session,
/// formatted as a bureaucratic performance review.
pub fn generate_manifesto(
    history: &SessionHistory,
    context: &RealWorldContext,
    winning_party: &str,
    singularity_type: &str,
) -> String {
    let profile = build_profile(history);
    let mirror = generate_mirror_lines(context);

    let mut doc = String::new();

    // Header
    doc.push_str("╔══════════════════════════════════════════════════════╗\n");
    doc.push_str("║  BROADCAST_OS CORP — CONFIDENTIAL PERFORMANCE FILE  ║\n");
    doc.push_str("║  CLASSIFICATION: EYES ONLY — ARCHITECT TIER         ║\n");
    doc.push_str("╚══════════════════════════════════════════════════════╝\n\n");

    // Session metadata
    doc.push_str(&format!("SESSION LOCALE:    {}\n", context.detected_locale));
    doc.push_str(&format!("SESSION TIMEZONE:  {}\n", context.timezone));
    doc.push_str(&format!("SESSION PLATFORM:  {}\n", context.platform));
    doc.push_str(&format!("SESSION DURATION:  {} minutes\n", context.session_minutes));
    doc.push_str(&format!("LOCAL HOUR AT EXIT: {:02}:00\n\n", context.local_hour));

    // Game outcome
    doc.push_str("═══ SIMULATION OUTCOME ═══\n\n");
    doc.push_str(&format!("WINNING MANDATE:   {}\n", winning_party));
    doc.push_str(&format!("SINGULARITY TYPE:  {}\n\n", singularity_type));

    // Psychographic profile
    doc.push_str("═══ PSYCHOGRAPHIC PROFILE ═══\n\n");
    doc.push_str(&format!(
        "POLARIZATION PREFERENCE:  {:.0}%\n",
        profile.polarization_preference * 100.0,
    ));
    doc.push_str(&format!(
        "TRUST SACRIFICE RATIO:    {:.0}%\n",
        profile.trust_sacrifice_ratio * 100.0,
    ));
    doc.push_str(&format!(
        "SHADOW FILTER USAGE:      {:.0}%\n",
        profile.filter_usage_pct * 100.0,
    ));
    doc.push_str(&format!(
        "ORACLE DEPENDENCE:        {:.0}%\n\n",
        profile.oracle_dependence * 100.0,
    ));

    // Mirror reveal
    doc.push_str("═══ REAL-WORLD TELEMETRY ═══\n\n");
    for line in &mirror {
        doc.push_str(&format!("• {}\n", line));
    }
    doc.push_str("\n");

    // The thesis
    doc.push_str("═══ CLASSIFICATION NOTE ═══\n\n");
    doc.push_str(
        "The mechanisms of control do not stop at the edge of the browser window.\n\
         You were not managing a simulation. You were the simulation.\n\n\
         The time is 6:48 PM. The parking lot is perfectly silent.\n\n"
    );

    doc.push_str("— Filed by: Jenna S., Compliance Division\n");
    doc.push_str("   BROADCAST_OS Corp.\n");
    doc.push_str("   [DOCUMENT ENDS]\n");

    doc
}

// ============================================================================
// Wasm Bridge (cfg-gated)
// ============================================================================

/// Sync real-world context from the browser (wasm) or system clock (native).
#[cfg(not(target_arch = "wasm32"))]
pub fn sync_real_world_system(
    mut context: ResMut<RealWorldContext>,
    time: Res<Time>,
) {
    // On native, use elapsed time for session tracking
    context.session_minutes = (time.elapsed_secs() / 60.0) as u32;
    // local_hour stays at default (18) for native builds
}

#[cfg(target_arch = "wasm32")]
pub fn sync_real_world_system(
    mut context: ResMut<RealWorldContext>,
    time: Res<Time>,
) {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "
        export function get_psychographic_data() {
            return {
                hour: new Date().getHours(),
                timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
                language: navigator.language,
                platform: navigator.platform || 'Unknown',
                is_hdr: window.matchMedia('(dynamic-range: high)').matches,
            };
        }
    ")]
    extern "C" {
        fn get_psychographic_data() -> JsValue;
    }

    let data = get_psychographic_data();
    if let Ok(parsed) = serde_wasm_bindgen::from_value::<PsychographicJsData>(data) {
        context.local_hour = parsed.hour;
        context.is_late_night = parsed.hour >= 22 || parsed.hour <= 4;
        context.is_evening = parsed.hour >= 17 && parsed.hour <= 21;
        context.timezone = parsed.timezone;
        context.detected_locale = parsed.language;
        context.platform = parsed.platform;
        context.is_hdr = parsed.is_hdr;
    }

    context.session_minutes = (time.elapsed_secs() / 60.0) as u32;
}

#[cfg(target_arch = "wasm32")]
#[derive(serde::Deserialize)]
struct PsychographicJsData {
    hour: u32,
    timezone: String,
    language: String,
    platform: String,
    is_hdr: bool,
}

/// Trigger a text file download in the browser.
#[cfg(target_arch = "wasm32")]
pub fn trigger_manifesto_download(content: &str) {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = "
        export function download_text_file(filename, content) {
            const blob = new Blob([content], { type: 'text/plain' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = filename;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        }
    ")]
    extern "C" {
        fn download_text_file(filename: &str, content: &str);
    }

    download_text_file("EXPORT_MANIFESTO.txt", content);
}

/// No-op on native — downloads only work in the browser.
#[cfg(not(target_arch = "wasm32"))]
pub fn trigger_manifesto_download(_content: &str) {
    // Native: no-op (or could write to filesystem in the future)
}

// ============================================================================
// Plugin
// ============================================================================

pub struct InteropPlugin;

impl Plugin for InteropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RealWorldContext>();
        app.add_systems(Update, sync_real_world_system);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::{SessionHistory, HistoryEntry, ActionType};

    fn default_context() -> RealWorldContext {
        RealWorldContext::default()
    }

    fn sample_history() -> SessionHistory {
        use crate::zone::ZoneType;
        SessionHistory {
            entries: vec![
                HistoryEntry {
                    action: ActionType::PlaceZone(ZoneType::EchoChamber),
                    quarter: 1,
                    cash_at_time: 1000.0,
                    trust_at_time: 0.8,
                },
                HistoryEntry {
                    action: ActionType::PlaceFilter,
                    quarter: 2,
                    cash_at_time: 800.0,
                    trust_at_time: 0.6,
                },
                HistoryEntry {
                    action: ActionType::OraclePaint,
                    quarter: 3,
                    cash_at_time: 600.0,
                    trust_at_time: 0.4,
                },
                HistoryEntry {
                    action: ActionType::PlaceZone(ZoneType::NeutralHub),
                    quarter: 4,
                    cash_at_time: 400.0,
                    trust_at_time: 0.3,
                },
            ],
        }
    }

    // =========================================================================
    // Greeting
    // =========================================================================

    #[test]
    fn test_greeting_early_morning() {
        assert_eq!(
            generate_greeting(5),
            "Good morning, Architect. The system requires your early focus."
        );
        assert_eq!(
            generate_greeting(8),
            "Good morning, Architect. The system requires your early focus."
        );
        assert_eq!(
            generate_greeting(11),
            "Good morning, Architect. The system requires your early focus."
        );
    }

    #[test]
    fn test_greeting_midday() {
        assert_eq!(
            generate_greeting(12),
            "Midday telemetry confirms peak localized apathy in your sector."
        );
        assert_eq!(
            generate_greeting(14),
            "Midday telemetry confirms peak localized apathy in your sector."
        );
        assert_eq!(
            generate_greeting(16),
            "Midday telemetry confirms peak localized apathy in your sector."
        );
    }

    #[test]
    fn test_greeting_evening() {
        assert_eq!(
            generate_greeting(17),
            "Good evening. The daylight is fading in your sector."
        );
        assert_eq!(
            generate_greeting(19),
            "Good evening. The daylight is fading in your sector."
        );
        assert_eq!(
            generate_greeting(21),
            "Good evening. The daylight is fading in your sector."
        );
    }

    #[test]
    fn test_greeting_late_night() {
        assert_eq!(
            generate_greeting(22),
            "Architect, it's late. Why are you still optimizing?"
        );
        assert_eq!(
            generate_greeting(0),
            "Architect, it's late. Why are you still optimizing?"
        );
        assert_eq!(
            generate_greeting(3),
            "Architect, it's late. Why are you still optimizing?"
        );
        assert_eq!(
            generate_greeting(4),
            "Architect, it's late. Why are you still optimizing?"
        );
    }

    #[test]
    fn test_greeting_boundary_5am() {
        // 5 AM = morning, 4 AM = late night
        assert_ne!(generate_greeting(4), generate_greeting(5));
    }

    #[test]
    fn test_greeting_boundary_22() {
        // 21 = evening, 22 = late night
        assert_ne!(generate_greeting(21), generate_greeting(22));
    }

    #[test]
    fn test_greeting_all_hours_covered() {
        // Every hour from 0–23 should produce a non-empty string
        for hour in 0..24 {
            let g = generate_greeting(hour);
            assert!(!g.is_empty(), "Hour {} produced empty greeting", hour);
        }
    }

    // =========================================================================
    // Creepy Email
    // =========================================================================

    #[test]
    fn test_email_contains_locale() {
        let ctx = default_context();
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("en-US"), "Email should contain locale: {}", email);
    }

    #[test]
    fn test_email_contains_timezone() {
        let ctx = default_context();
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("America/Los_Angeles"), "Email should contain timezone");
    }

    #[test]
    fn test_email_contains_jenna() {
        let ctx = default_context();
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("Jenna S."), "Email should be from Jenna S.");
    }

    #[test]
    fn test_email_high_optimization_warning() {
        let ctx = default_context();
        let email = generate_creepy_email(&ctx, 0.8, 1.0);
        assert!(email.contains("Oracle"), "High optimization should mention Oracle");
        assert!(email.contains("safety thresholds"), "Should mention safety thresholds");
    }

    #[test]
    fn test_email_low_engagement_warning() {
        let ctx = default_context();
        let email = generate_creepy_email(&ctx, 0.0, 0.2);
        assert!(email.contains("Cognitive resonance"), "Low engagement should mention cognitive drop");
    }

    #[test]
    fn test_email_long_session_note() {
        let mut ctx = default_context();
        ctx.session_minutes = 45;
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("45 minutes"), "Long session should show duration");
        assert!(email.contains("looked away"), "Should ask about looking away");
    }

    #[test]
    fn test_email_short_session_no_note() {
        let mut ctx = default_context();
        ctx.session_minutes = 10;
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(!email.contains("looked away"), "Short session should not nag");
    }

    #[test]
    fn test_email_late_night_tone() {
        let mut ctx = default_context();
        ctx.local_hour = 2;
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("it's late"), "2 AM email should have late-night tone");
    }

    #[test]
    fn test_email_morning_tone() {
        let mut ctx = default_context();
        ctx.local_hour = 8;
        let email = generate_creepy_email(&ctx, 0.0, 1.0);
        assert!(email.contains("Good morning"), "8 AM email should have morning tone");
    }

    // =========================================================================
    // Mirror Reveal Lines
    // =========================================================================

    #[test]
    fn test_mirror_contains_platform() {
        let mut ctx = default_context();
        ctx.platform = "MacIntel".to_string();
        let lines = generate_mirror_lines(&ctx);
        let joined = lines.join("\n");
        assert!(joined.contains("MacIntel"), "Mirror should mention platform");
    }

    #[test]
    fn test_mirror_contains_timezone() {
        let ctx = default_context();
        let lines = generate_mirror_lines(&ctx);
        let joined = lines.join("\n");
        assert!(joined.contains("America/Los_Angeles"), "Mirror should mention timezone");
    }

    #[test]
    fn test_mirror_contains_session_minutes() {
        let mut ctx = default_context();
        ctx.session_minutes = 42;
        let lines = generate_mirror_lines(&ctx);
        let joined = lines.join("\n");
        assert!(joined.contains("42 minutes"), "Mirror should mention session duration");
    }

    #[test]
    fn test_mirror_late_night_extra_line() {
        let mut ctx = default_context();
        ctx.is_late_night = true;
        let lines = generate_mirror_lines(&ctx);
        let joined = lines.join("\n");
        assert!(joined.contains("past midnight"), "Late night should add extra line");
    }

    #[test]
    fn test_mirror_hdr_extra_line() {
        let mut ctx = default_context();
        ctx.is_hdr = true;
        let lines = generate_mirror_lines(&ctx);
        let joined = lines.join("\n");
        assert!(joined.contains("high-dynamic-range"), "HDR should add extra line");
    }

    #[test]
    fn test_mirror_minimum_three_lines() {
        let ctx = default_context();
        let lines = generate_mirror_lines(&ctx);
        assert!(lines.len() >= 3, "Mirror should have at least 3 lines, got {}", lines.len());
    }

    // =========================================================================
    // Manifesto
    // =========================================================================

    #[test]
    fn test_manifesto_contains_header() {
        let history = sample_history();
        let ctx = default_context();
        let doc = generate_manifesto(&history, &ctx, "The Vanguard", "Total Polarization");
        assert!(doc.contains("BROADCAST_OS CORP"), "Manifesto should have header");
        assert!(doc.contains("CONFIDENTIAL"), "Should be classified");
    }

    #[test]
    fn test_manifesto_contains_session_data() {
        let history = sample_history();
        let mut ctx = default_context();
        ctx.session_minutes = 37;
        let doc = generate_manifesto(&history, &ctx, "Test", "Test");
        assert!(doc.contains("37 minutes"), "Should include session duration");
        assert!(doc.contains("en-US"), "Should include locale");
        assert!(doc.contains("America/Los_Angeles"), "Should include timezone");
    }

    #[test]
    fn test_manifesto_contains_profile() {
        let history = sample_history();
        let ctx = default_context();
        let doc = generate_manifesto(&history, &ctx, "Test", "Test");
        assert!(doc.contains("POLARIZATION PREFERENCE"), "Should include profile data");
        assert!(doc.contains("ORACLE DEPENDENCE"), "Should include oracle data");
    }

    #[test]
    fn test_manifesto_contains_thesis() {
        let history = sample_history();
        let ctx = default_context();
        let doc = generate_manifesto(&history, &ctx, "Test", "Test");
        assert!(doc.contains("browser window"), "Should include the thesis");
        assert!(doc.contains("6:48 PM"), "Should end with 6:48");
    }

    #[test]
    fn test_manifesto_contains_mirror() {
        let history = sample_history();
        let mut ctx = default_context();
        ctx.platform = "Win32".to_string();
        let doc = generate_manifesto(&history, &ctx, "Test", "Test");
        assert!(doc.contains("Win32"), "Should include mirror reveal platform");
    }

    #[test]
    fn test_manifesto_ends_with_document_ends() {
        let history = sample_history();
        let ctx = default_context();
        let doc = generate_manifesto(&history, &ctx, "Test", "Test");
        assert!(doc.trim().ends_with("[DOCUMENT ENDS]"), "Should end formally");
    }

    // =========================================================================
    // RealWorldContext Defaults
    // =========================================================================

    #[test]
    fn test_default_context_is_6pm() {
        let ctx = RealWorldContext::default();
        assert_eq!(ctx.local_hour, 18, "Default hour should be 6 PM");
    }

    #[test]
    fn test_default_context_not_late_night() {
        let ctx = RealWorldContext::default();
        assert!(!ctx.is_late_night, "Default should not be late night");
    }

    #[test]
    fn test_default_context_is_evening() {
        let ctx = RealWorldContext::default();
        assert!(ctx.is_evening, "Default should be evening (6 PM)");
    }
}
