# `assets/` — SimSitting Asset Files

## Shaders

### `shaders/opinion_physics.wgsl`
The GPU compute shader that processes 100,000 agents per frame.

**Bind Groups:**
| Group | Binding | Type | Content |
|---|---|---|---|
| `@group(0)` | `@binding(0)` | `storage<read_write>` | `SimAgentGpu[]` — 100k agents (32B each) |
| `@group(1)` | `@binding(0)` | `storage<read_write>` | `GpuAnalytics` — sum/max/min accumulators (32B) |
| `@group(2)` | `@binding(0)` | `storage<read_write>` | `PoliticalData` — 10 atomic u32 histogram buckets + total_votes (44B) |

**Workgroup:** 256 threads × 391 dispatches = 100,096 invocations (100,000 agents)

**Operations per invocation:**
1. Read agent opinion + engagement
2. Sample influence map (zone effects)
3. Deffuant-Weisbuch opinion interaction
4. Atomic fixed-point accumulation for analytics
5. Atomic histogram bucket increment for elections

### `shaders/post_process.wgsl`
Fragment shader for CRT scanline overlay and aesthetic morph. The `optimization_level` uniform (0.0–1.0) interpolates between the dithered CRT view and a sterile, clinical image.
