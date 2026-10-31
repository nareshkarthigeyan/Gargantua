// Additive plasma trail and engine plume. Input vertices are already in world
// space with a linear HDR colour.

struct Scene {
    view_proj: mat4x4<f32>,
    cam_pos_time: vec4<f32>,
    cam_right_fov: vec4<f32>,
    cam_up_aspect: vec4<f32>,
    cam_forward_disk: vec4<f32>,
    viewport_disk: vec4<f32>,
    ship_pos: vec4<f32>,
    misc: vec4<f32>,
    cam_velocity: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene;

struct VIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.position = scene.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Premultiplied-ish additive output; the pipeline uses SrcAlpha / One.
    return vec4<f32>(in.color.rgb, in.color.a);
}
