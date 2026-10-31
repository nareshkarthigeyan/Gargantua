// Bloom and final grade. Scene colour is linear HDR; the target is an
// non-sRGB swapchain image, so gamma is applied explicitly at the end.

struct Post {
    params: vec4<f32>, // bloom intensity, exposure, time, grain
    screen: vec4<f32>, // width, height, danger, aspect
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var<uniform> post: Post;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VOut {
    let p = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    var out: VOut;
    out.position = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    var hdr = textureSample(scene_tex, scene_sampler, uv).rgb;
    let bloom = textureSample(bloom_tex, scene_sampler, uv).rgb * post.params.x;
    // Optical bloom stays attached to bright emission; the warm response
    // preserves a white-hot disk core with a visible amber halo.
    let energy = dot(bloom, vec3<f32>(0.2126, 0.7152, 0.0722));
    hdr += mix(bloom, energy * vec3<f32>(2.4, 0.36, 0.015), 0.8);

    // Gentle chromatic warm/cool grading before the display transform.
    hdr *= vec3<f32>(1.02, 1.0, 0.97);

    var col = aces(hdr * post.params.y);
    col = pow(max(col, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));

    let centered = uv * 2.0 - 1.0;
    let vig = smoothstep(1.55, 0.42, length(centered));
    col *= 0.66 + 0.34 * vig;

    // Space stays black. Proximity feedback comes from camera motion and
    // instruments, rather than a fabricated red sky or sensor noise.
    return vec4<f32>(col, 1.0);
}
