// Lit spacecraft pass. Vertices are transformed into world space on the CPU
// so this shader only needs the shared camera uniform.

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
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emissive: f32,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) emissive: f32,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.position = scene.view_proj * vec4<f32>(in.position, 1.0);
    out.world = in.position;
    out.normal = in.normal;
    out.color = in.color;
    out.emissive = in.emissive;
    return out;
}

fn hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3));
    q *= 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let view_dir = normalize(scene.cam_pos_time.xyz - in.world);
    var n = normalize(in.normal);
    if (dot(n, view_dir) < 0.0) {
        n = -n;
    }

    // Key light: the accretion disk / photon sphere ahead. Fill: cold sky.
    let to_bh = normalize(-in.world);
    let disk_key = max(dot(n, to_bh), 0.0);
    let sky_fill = max(dot(n, normalize(vec3<f32>(0.25, 0.9, 0.35))), 0.0);
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 2.6);

    let base = pow(max(in.color, vec3<f32>(0.0)), vec3<f32>(2.2));
    let warm = vec3<f32>(1.0, 0.58, 0.24);
    let cool = vec3<f32>(0.24, 0.62, 1.0);

    // Blinn specular from the hot disk.
    let half_dir = normalize(to_bh + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), 52.0) * (0.25 + 0.75 * disk_key);

    let grain = 0.88 + 0.12 * hash(in.world * 83.0);
    let diffuse = warm * disk_key * 0.54 + cool * sky_fill * 0.11
        + vec3<f32>(0.035, 0.041, 0.052);
    var color = base * diffuse * grain;
    color += warm * spec * 0.20;
    color += cool * rim * 0.12;

    // Emissive strips, windows, engine throat. Emissive is already in the
    // linear HDR domain and blooms in the post pass.
    color += in.color * in.emissive;
    return vec4<f32>(color, 1.0);
}
