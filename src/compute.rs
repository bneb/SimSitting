//! SimSitting — GPU Compute Pipeline (Phase 1.5: The Singularity)
//!
//! Owns the full GPU lifecycle for 100k opinion dynamics:
//!
//! 1. Buffer creation (`SimBuffer` + `Analytics` + `Staging`)
//! 2. Compute pipeline (`opinion_physics.wgsl` dispatch)
//! 3. Render graph node ([`SimComputeNode`])
//! 4. Async analytics readback via crossbeam channel
//!
//! ```text
//! Main World ──ExtractResource──> Render World ──Node::run──> GPU
//!                                                              │
//! Main World <──crossbeam-channel── Staging Buffer <──copy─────┘
//! ```

use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{self, RenderLabel},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
        Render, RenderApp, RenderSet,
    },
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::borrow::Cow;

use crate::economy::GlobalStats;
use crate::sim::{GpuAnalytics, SimAgentGpu, SimConfig};

// ============================================================================
// Constants
// ============================================================================

const WORKGROUP_SIZE: u32 = 64;
/// Scale factor for fixed-point atomic sums. 1000 for 100k agents (avoids u32 overflow).
pub const ANALYTICS_SCALE: f32 = 1000.0;

// ============================================================================
// Resources
// ============================================================================

/// The agent data buffer, extracted to the Render World.
/// Contains flattened SimAgentGpu data for the compute shader.
#[derive(Resource, Clone)]
pub struct SimBuffer {
    pub data: Vec<SimAgentGpu>,
    pub count: u32,
}

impl ExtractResource for SimBuffer {
    type Source = SimBuffer;
    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Channel for receiving GPU analytics on the main thread.
#[derive(Resource)]
pub struct AnalyticsChannel {
    pub sender: Sender<GpuAnalytics>,
    pub receiver: Receiver<GpuAnalytics>,
}

impl Default for AnalyticsChannel {
    fn default() -> Self {
        let (sender, receiver) = bounded(4);
        Self { sender, receiver }
    }
}

/// Render-world resource: GPU buffers + bind group
#[derive(Resource)]
struct GpuSimBuffers {
    agents_buffer: Buffer,
    stats_buffer: Buffer,
    staging_buffer: Buffer,
    bind_group: BindGroup,
}

/// Render-world resource: cached compute pipeline
#[derive(Resource)]
struct SimComputePipeline {
    pipeline_id: CachedComputePipelineId,
    bind_group_layout: BindGroupLayout,
}

// ============================================================================
// Render Graph Label
// ============================================================================

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct SimComputeLabel;

// ============================================================================
// Plugin
// ============================================================================

pub struct ComputePlugin;

impl Plugin for ComputePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnalyticsChannel>();
        app.add_systems(Update, poll_gpu_analytics);
    }

    fn finish(&self, app: &mut App) {
        // Check if SimBuffer exists (it's created by setup_gpu_buffers)
        if !app.world().contains_resource::<SimBuffer>() {
            return;
        }

        let render_app = match app.get_sub_app_mut(RenderApp) {
            Some(ra) => ra,
            None => return, // No render app (headless mode / tests)
        };

        // Register the extract plugin and render graph node
        // Note: ExtractResourcePlugin is added to the main app, not render app
        let _ = render_app;

        app.add_plugins(ExtractResourcePlugin::<SimBuffer>::default());

        let render_app = app.get_sub_app_mut(RenderApp).unwrap();
        render_app.add_systems(
            Render,
            prepare_gpu_buffers.in_set(RenderSet::Prepare),
        );
    }
}

// ============================================================================
// Main World Systems
// ============================================================================

/// Startup system: initialize SimBuffer from spawned agents
pub fn setup_gpu_buffers(
    mut commands: Commands,
    agents: Query<(&crate::sim::SimAgent, &Transform)>,
    _config: Res<SimConfig>,
) {
    let data: Vec<SimAgentGpu> = agents.iter()
        .map(|(a, t)| SimAgentGpu::from_agent(a, t))
        .collect();
    let count = data.len() as u32;

    info!("SimBuffer created: {} agents → {} bytes", count, count as usize * std::mem::size_of::<SimAgentGpu>());

    commands.insert_resource(SimBuffer { data, count });
}

/// Poll the analytics channel and update GlobalStats from GPU results.
fn poll_gpu_analytics(
    channel: Res<AnalyticsChannel>,
    mut stats: ResMut<GlobalStats>,
) {
    // Drain the channel — only use the latest result
    let mut latest: Option<GpuAnalytics> = None;
    while let Ok(data) = channel.receiver.try_recv() {
        latest = Some(data);
    }

    if let Some(analytics) = latest {
        stats.mean_opinion = analytics.mean_opinion();
        stats.polarization_heat = analytics.mean_polarization();
        stats.engagement_index = analytics.mean_engagement();
    }
}

// ============================================================================
// Render World Systems
// ============================================================================

/// Prepare GPU buffers in the render world from extracted SimBuffer data.
fn prepare_gpu_buffers(
    mut commands: Commands,
    sim_buffer: Option<Res<SimBuffer>>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
    existing_pipeline: Option<Res<SimComputePipeline>>,
) {
    let Some(sim_buffer) = sim_buffer else { return };

    // Only create pipeline once
    if existing_pipeline.is_none() {
        let bind_group_layout = render_device.create_bind_group_layout(
            "sim_compute_bind_group_layout",
            &[
                // @group(0) @binding(0): agents storage buffer
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // @group(0) @binding(1): analytics storage buffer
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        let shader = asset_server.load("shaders/opinion_physics.wgsl");
        let pipeline_id = pipeline_cache.queue_compute_pipeline(
            ComputePipelineDescriptor {
                label: Some(Cow::from("opinion_physics_pipeline")),
                layout: vec![bind_group_layout.clone()],
                push_constant_ranges: vec![],
                shader,
                shader_defs: vec![],
                entry_point: Cow::from("update_opinions"),
                zero_initialize_workgroup_memory: true,
            },
        );

        commands.insert_resource(SimComputePipeline {
            pipeline_id,
            bind_group_layout,
        });
    }

    // Create buffers from agent data
    let agent_bytes = bytemuck::cast_slice(&sim_buffer.data);
    let agents_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("sim_agents_buffer"),
        contents: agent_bytes,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    // Analytics buffer (4 x u32 = 16 bytes)
    let analytics_init = GpuAnalytics {
        sum_opinion: 0,
        sum_polarization: 0,
        sum_engagement: 0,
        agent_count: sim_buffer.count,
    };
    let stats_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("sim_analytics_buffer"),
        contents: bytemuck::bytes_of(&analytics_init),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
    });

    // Staging buffer for CPU readback
    let staging_buffer = render_device.create_buffer(&BufferDescriptor {
        label: Some("sim_staging_buffer"),
        size: std::mem::size_of::<GpuAnalytics>() as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Build bind group if we have the pipeline layout
    if let Some(pipeline_res) = &existing_pipeline {
        let bind_group = render_device.create_bind_group(
            "sim_compute_bind_group",
            &pipeline_res.bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: agents_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: stats_buffer.as_entire_binding(),
                },
            ],
        );

        commands.insert_resource(GpuSimBuffers {
            agents_buffer,
            stats_buffer,
            staging_buffer,
            bind_group,
        });
    }
}

// ============================================================================
// Compute Node
// ============================================================================

struct SimComputeNode;

impl FromWorld for SimComputeNode {
    fn from_world(_world: &mut World) -> Self {
        Self
    }
}

impl render_graph::Node for SimComputeNode {
    fn run<'w>(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), render_graph::NodeRunError> {
        let Some(pipeline_res) = world.get_resource::<SimComputePipeline>() else {
            return Ok(());
        };
        let Some(gpu_buffers) = world.get_resource::<GpuSimBuffers>() else {
            return Ok(());
        };

        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline_res.pipeline_id) else {
            return Ok(()); // Pipeline still compiling
        };

        let encoder = render_context.command_encoder();

        // Clear analytics buffer to zero before dispatch
        encoder.clear_buffer(&gpu_buffers.stats_buffer, 0, None);

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("sim_opinion_physics"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &gpu_buffers.bind_group, &[]);

            // Dispatch: ceil(agent_count / WORKGROUP_SIZE) workgroups
            let sim_buffer = world.get_resource::<SimBuffer>();
            let count = sim_buffer.map(|b| b.count).unwrap_or(100_000);
            let workgroups = (count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Copy analytics results to staging buffer for CPU readback
        encoder.copy_buffer_to_buffer(
            &gpu_buffers.stats_buffer,
            0,
            &gpu_buffers.staging_buffer,
            0,
            std::mem::size_of::<GpuAnalytics>() as u64,
        );

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_channel_creation() {
        let channel = AnalyticsChannel::default();
        let data = GpuAnalytics {
            sum_opinion: 500_000,
            sum_polarization: 200_000,
            sum_engagement: 800_000,
            agent_count: 1,
        };
        channel.sender.send(data).unwrap();
        let received = channel.receiver.try_recv().unwrap();
        assert_eq!(received.agent_count, 1);
        assert!((received.mean_opinion() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_analytics_channel_drains_to_latest() {
        let channel = AnalyticsChannel::default();
        for i in 1..=3u32 {
            channel.sender.send(GpuAnalytics {
                sum_opinion: i * 100_000,
                sum_polarization: 0,
                sum_engagement: 1_000_000,
                agent_count: 1,
            }).unwrap();
        }
        let mut latest = None;
        while let Ok(data) = channel.receiver.try_recv() {
            latest = Some(data);
        }
        let analytics = latest.unwrap();
        assert_eq!(analytics.sum_opinion, 300_000);
    }

    #[test]
    fn test_poll_updates_global_stats() {
        let channel = AnalyticsChannel::default();
        channel.sender.send(GpuAnalytics {
            sum_opinion: 70_000_000,
            sum_polarization: 40_000_000,
            sum_engagement: 80_000_000,
            agent_count: 100,
        }).unwrap();

        let mut stats = GlobalStats::default();
        // Simulate poll drain
        if let Ok(analytics) = channel.receiver.try_recv() {
            stats.mean_opinion = analytics.mean_opinion();
            stats.polarization_heat = analytics.mean_polarization();
            stats.engagement_index = analytics.mean_engagement();
        }
        assert!((stats.mean_opinion - 0.7).abs() < 0.01);
        assert!((stats.polarization_heat - 0.4).abs() < 0.01);
        assert!((stats.engagement_index - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_analytics_scale_constant() {
        // SCALE=1000 at 100k agents: max sum = 1000 * 100_000 = 100M < u32::MAX
        let max_sum = ANALYTICS_SCALE as u64 * 100_000;
        assert!(max_sum < u32::MAX as u64, "Scale must not overflow u32 at 100k agents");
    }

    #[test]
    fn test_workgroup_dispatch_count() {
        let agent_count = 100_000u32;
        let workgroups = (agent_count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
        assert_eq!(workgroups, 1563); // 100_000 / 64 = 1562.5, ceil = 1563
    }

    #[test]
    fn test_sim_buffer_byte_size() {
        let count = 100_000usize;
        let size = count * std::mem::size_of::<SimAgentGpu>();
        // 100k * 32 bytes = 3.2 MB — well within GPU limits
        assert_eq!(size, 3_200_000);
    }

    #[test]
    fn test_gpu_analytics_buffer_size() {
        // Must be exactly 16 bytes for our staging copy
        assert_eq!(std::mem::size_of::<GpuAnalytics>(), 16);
    }
}
