// CRT/Dither Overlay for that 90s "Airwave" vibe
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::globals::Globals

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

@group(0) @binding(2) var<uniform> globals: Globals;

// Ordered Dither Matrix (4x4) to simulate 90s limited color depth
const dither_matrix = array<f32, 16>(
    0.0,  0.5,  0.125, 0.625,
    0.75, 0.25, 0.875, 0.375,
    0.187, 0.687, 0.062, 0.562,
    0.937, 0.437, 0.812, 0.312
);

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, screen_sampler, in.uv);
    
    // Apply 6:47PM Purple Tint
    let sunset_tint = vec3<f32>(0.46, 0.23, 0.74); // #753BBD
    let final_rgb = mix(color.rgb, sunset_tint, 0.2);
    
    // CRT Scanlines based on time and uv
    let time = globals.time;
    let scanline = sin(in.position.y * 2.0 + time * 10.0) * 0.05;
    
    // 90s Ordered Dither
    let x = u32(in.position.x) % 4u;
    let y = u32(in.position.y) % 4u;
    let dither = dither_matrix[y * 4u + x];
    let dither_val = (dither - 0.5) * 0.1;
    
    return vec4<f32>(final_rgb - scanline + dither_val, 1.0);
}
