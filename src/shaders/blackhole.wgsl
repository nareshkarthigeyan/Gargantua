// Real-time Schwarzschild ray tracer.
//
// Units: G = c = 1, Rs = 1, M = 1/2. The event horizon is r = 1,
// the photon sphere is r = 1.5 and the ISCO is r = 3.
//
// A null geodesic in Schwarzschild satisfies the Binet equation
//     u'' + u = 3 M u²,  u = 1/r
// which becomes, in Cartesian areal coordinates and an affine parameter,
//     a = -3 M h² x / r⁵,  h = |x × dx/dλ|.
// We integrate that equation backwards from the camera. Every escaping ray
// samples the procedural sky; every equatorial crossing samples the accretion
// disk with gravitational redshift and relativistic Doppler beaming.

struct Scene {
    view_proj: mat4x4<f32>,
    cam_pos_time: vec4<f32>,      // xyz camera, w coordinate time
    cam_right_fov: vec4<f32>,     // xyz right, w tan(fov/2)
    cam_up_aspect: vec4<f32>,     // xyz up, w aspect
    cam_forward_disk: vec4<f32>,  // xyz forward, w disk outer radius
    viewport_disk: vec4<f32>,     // w, h, disk inner, exposure
    ship_pos: vec4<f32>,          // xyz ship, w lost
    misc: vec4<f32>,              // danger, max steps, real time, quality
    cam_velocity: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene;

const M: f32 = 0.5;
const TAU: f32 = 6.28318530718;
const DISK_INNER: f32 = 3.0;
const DISK_OUTER: f32 = 12.0;

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

fn hash31(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yxx) * q.zyx);
}

fn vnoise(p_in: vec3<f32>) -> f32 {
    let i = floor(p_in);
    var f = fract(p_in);
    f = f * f * (3.0 - 2.0 * f);
    let n000 = hash31(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));
    let nx00 = mix(n000, n100, f.x);
    let nx10 = mix(n010, n110, f.x);
    let nx01 = mix(n001, n101, f.x);
    let nx11 = mix(n011, n111, f.x);
    return mix(mix(nx00, nx10, f.y), mix(nx01, nx11, f.y), f.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    var q = p;
    var amp = 0.55;
    var sum = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        sum += amp * vnoise(q);
        q = q * 2.07 + vec3<f32>(3.1, 1.7, 5.3);
        amp *= 0.48;
    }
    return sum;
}

fn blackbody(temp_in: f32) -> vec3<f32> {
    let t = clamp(temp_in, 700.0, 30000.0) / 1000.0;
    var col = mix(
        vec3<f32>(1.0, 0.10, 0.015),
        vec3<f32>(1.0, 0.36, 0.055),
        smoothstep(0.8, 2.4, t),
    );
    col = mix(col, vec3<f32>(1.0, 0.70, 0.30), smoothstep(2.4, 4.8, t));
    col = mix(col, vec3<f32>(1.0, 0.94, 0.82), smoothstep(4.8, 7.5, t));
    col = mix(col, vec3<f32>(0.75, 0.86, 1.0), smoothstep(7.5, 14.0, t));
    return col;
}

// Conserved angular momentum fixes the force throughout a ray's path.
fn lens_accel(x: vec3<f32>, h2: f32) -> vec3<f32> {
    let r2 = max(dot(x, x), 0.1225);
    return -1.5 * h2 * x / (r2 * r2 * sqrt(r2));
}

struct RayState { x: vec3<f32>, v: vec3<f32> };

fn launch_ray(origin: vec3<f32>, direction: vec3<f32>) -> RayState {
    let radial = normalize(origin);
    let n = normalize(direction);
    let nr = radial * dot(n, radial);
    let alpha = sqrt(max(1.0 - 1.0 / length(origin), 0.00001));
    return RayState(origin, nr + (n - nr) / alpha);
}

// Fourth-order Runge-Kutta in affine parameter, with radius/angle bounds.
fn step_ray(ray: RayState, h2: f32, dt: f32) -> RayState {
    let a1 = lens_accel(ray.x, h2);
    let v2 = ray.v + a1 * (dt * 0.5);
    let a2 = lens_accel(ray.x + ray.v * (dt * 0.5), h2);
    let v3 = ray.v + a2 * (dt * 0.5);
    let a3 = lens_accel(ray.x + v2 * (dt * 0.5), h2);
    let v4 = ray.v + a3 * dt;
    let a4 = lens_accel(ray.x + v3 * dt, h2);
    return RayState(
        ray.x + (ray.v + 2.0 * v2 + 2.0 * v3 + v4) * (dt / 6.0),
        ray.v + (a1 + 2.0 * a2 + 2.0 * a3 + a4) * (dt / 6.0));
}

// Angular sky catalogue. Stars live on the sky, rather than in sparse 3D
// cells that the unit sphere almost never intersects.
fn star_layer(dir: vec3<f32>, scale: f32, threshold: f32,
    seed: vec3<f32>, size: f32, boost: f32) -> vec3<f32> {
    let uv = vec2<f32>(atan2(dir.z, dir.x) / TAU + 0.5,
        asin(clamp(dir.y, -1.0, 1.0)) / 3.14159265 + 0.5);
    let grid = vec2<f32>(scale * 2.0, scale);
    let p = uv * grid;
    let cell = floor(p);
    let rnd = hash33(vec3<f32>(cell, seed.x) + seed);
    if (rnd.x < threshold) { return vec3<f32>(0.0); }
    let center = vec2<f32>(0.22) + rnd.yz * 0.56;
    let d = length(fract(p) - center);
    // Keep a resolved core even at half resolution.
    let footprint = scale / max(scene.viewport_disk.y, 1.0);
    let radius = max(size, footprint * 0.15);
    let core = exp(-d * d / (radius * radius));
    let tint = blackbody((3200.0 + 14000.0 * rnd.y * rnd.y) * boost);
    let twinkle = 0.84 + 0.16 * sin(scene.misc.z * (0.8 + rnd.z) + rnd.y * TAU);
    return tint * core * (0.22 + 1.15 * pow(rnd.z, 3.0)) * twinkle;
}

fn sky(dir_in: vec3<f32>, frequency_shift: f32) -> vec3<f32> {
    let dir = normalize(dir_in);
    let boost = clamp(frequency_shift, 0.1, 12.0);

    // Sparse, layered stars. Six-sided avoidance of a grid seam is unnecessary
    // at these angular sizes, and keeps the shader light enough for real time.
    var col = vec3<f32>(0.0);
    col += star_layer(dir, 90.0, 0.94, vec3<f32>(1.3, 7.1, 2.7), 0.045, boost);
    col += star_layer(dir, 220.0, 0.965, vec3<f32>(4.2, 2.9, 8.3), 0.040, boost);
    col += star_layer(dir, 440.0, 0.98, vec3<f32>(7.4, 0.3, 5.2), 0.035, boost);
    col += star_layer(dir, 700.0, 0.99, vec3<f32>(2.2, 9.8, 1.4), 0.036, boost)
        * 0.65;


    col *= clamp(pow(frequency_shift, 3.0), 0.03, 128.0);
    // Compress point-source highlights before bloom, preserving tiny stars
    // while retaining Doppler color and relative intensity changes.
    let peak = max(col.r, max(col.g, col.b));
    return col / (1.0 + 1.5 * peak);
}

fn disk_emission(
    hit: vec3<f32>,
    angular_momentum_y: f32,
    time: f32,
    observer_shift: f32,
) -> vec4<f32> {
    let r = length(vec2<f32>(hit.x, hit.z));
    if (r < DISK_INNER || r > DISK_OUTER) {
        return vec4<f32>(0.0);
    }

    let phi = atan2(hit.z, hit.x);
    let omega = sqrt(M) / pow(r, 1.5);
    let theta = phi - omega * time;

    // Keplerian shear. The high azimuthal frequency is what turns the disk
    // from a smooth fog into resolved filaments; the log-radius term keeps
    // the same feature density from the ISCO to the outer edge.
    // Fine radial bands with long azimuthal coherence: this is what makes the
    // disk read as sheared orbital material instead of terrestrial clouds.
    let q = vec3<f32>(cos(theta) * 3.2, sin(theta) * 3.2, log(r) * 22.0);
    let n1 = fbm(q);
    let n2 = fbm(vec3<f32>(cos(theta) * 11.0, sin(theta) * 11.0, log(r) * 7.0)
        + vec3<f32>(1.7, 8.3, 2.1));
    let mix = n1 * 0.76 + n2 * 0.34;
    let filaments = smoothstep(0.32, 0.70, mix);
    let rings = 0.55 + 0.45 * sin(r * 12.0 + n1 * 6.0 - time * 0.03);

    // Soft edges. The inner edge is at the ISCO, where the disk is truncated.
    let inner_edge = smoothstep(DISK_INNER, DISK_INNER + 0.42, r);
    let outer_edge = 1.0 - smoothstep(DISK_OUTER * 0.66, DISK_OUTER, r);
    let edge = inner_edge * outer_edge;
    if (edge <= 0.001) {
        return vec4<f32>(0.0);
    }

    // Standard thin-disk temperature profile, with a hotter inner torus. The
    // outer disk is deliberately cool enough to stay amber instead of beige.
    let temp = 7000.0 * pow(DISK_INNER / r, 0.80) * (0.95 + 0.18 * filaments);

    // Frequency ratio from conserved photon E and L, and the emitter's
    // circular Schwarzschild 4-velocity. Backward rays have opposite spatial
    // angular momentum to the arriving future-directed photon.
    let emitter_ut = inverseSqrt(1.0 - 1.5 / r);
    let g = clamp(observer_shift /
        (emitter_ut * (1.0 + omega * angular_momentum_y)), 0.02, 24.0);

    let observed_temp = temp * g;
    let tint = blackbody(observed_temp);
    // The g^3 factor keeps the approaching side brighter, but the coefficient
    // is deliberately restrained: it is very easy to clip an accretion disk.
    let brightness = 0.42 * pow(max(g, 0.08), 3.0) * edge
        * (0.24 + 0.95 * filaments) * (0.66 + 0.44 * rings)
        * pow(DISK_INNER / r, 1.65);
    let hot_core = vec3<f32>(1.0, 0.80, 0.54) * 0.030 * inner_edge * pow(g, 2.2);
    let alpha = 1.0; // optically thick disk; background stars cannot shine through

    // Camera highlight compensation preserves the filaments under enormous
    // observed frequency shifts instead of clipping the whole disk to white.
    let meter_shift = min(observer_shift, 24.0);
    let highlight_compensation = 1.0 / (1.0 + 0.025 * pow(meter_shift, 3.0));
    return vec4<f32>((tint * brightness + hot_core) * highlight_compensation, alpha);
}

struct RayResult {
    color: vec3<f32>,
    direction: vec3<f32>,
    fate: u32, // 0 captured, 1 infinity, 2 unresolved, 3 opaque disk
};

// Shared by the fragment shader and GPU physics validation.
fn integrate_ray(origin: vec3<f32>, direction: vec3<f32>, time: f32,
    observer_shift: f32, max_steps: u32, draw_disk: bool) -> RayResult {
    var ray = launch_ray(origin, direction);
    let angular_momentum = cross(ray.x, ray.v);
    let h2 = dot(angular_momentum, angular_momentum);
    let h = sqrt(h2);
    let escape_r = max(length(origin) * 2.0, 2000.0);
    var result = RayResult(vec3<f32>(0.0), direction, 2u);
    var coordinate_delay = 0.0;

    // Radial rays are exact; this also avoids an ill-defined orbital plane.
    if (h < 1e-6) {
        if (dot(ray.x, ray.v) < 0.0) { result.fate = 0u; }
        else { result.fate = 1u; }
        return result;
    }
    for (var i = 0u; i < max_steps; i += 1u) {
        let r = length(ray.x);
        if (r <= 1.0001) { result.fate = 0u; break; }
        if (r > escape_r && dot(ray.x, ray.v) > 0.0) {
            result.fate = 1u;
            break;
        }
        // At most 0.025 radians per step and 16% of the current radius.
        // RK4 preserves weak-field deflection and the critical capture cone.
        let dt = min(r * 0.16, 0.025 * r * r / h);
        let old = ray;
        ray = step_ray(ray, h2, dt);
        // Retarded emission time: dt/dlambda = E/(1-Rs/r), with E=1.
        let old_lapse = max(1.0 - 1.0/r, 0.0001);
        let new_lapse = max(1.0 - 1.0/length(ray.x), 0.0001);
        let delay_before = coordinate_delay;
        coordinate_delay += dt*0.5*(1.0/old_lapse + 1.0/new_lapse);
        if (length(ray.x) <= 1.0001) { result.fate = 0u; break; }
        if (draw_disk && old.x.y * ray.x.y <= 0.0 &&
            abs(old.x.y - ray.x.y) > 1e-8) {
            let f = clamp(old.x.y / (old.x.y - ray.x.y), 0.0, 1.0);
            let hit = mix(old.x, ray.x, f);
            let sample = disk_emission(hit, angular_momentum.y, time - mix(delay_before, coordinate_delay, f), observer_shift);
            if (sample.a > 0.0) {
                result.color = sample.rgb;
                result.fate = 3u;
                break;
            }
        }
    }
    result.direction = normalize(ray.v);
    return result;
}

fn trace_ray(origin: vec3<f32>, observed_dir: vec3<f32>, time: f32) -> vec3<f32> {
    let beta = scene.cam_velocity.xyz;
    let beta2 = dot(beta, beta);
    var static_dir = observed_dir;
    var energy_static = 1.0;
    if (beta2 > 1e-10) {
        let gamma = inverseSqrt(max(1.0 - beta2, 0.00001));
        let bn = dot(beta, observed_dir);
        energy_static = gamma * (1.0 - bn);
        static_dir = normalize((observed_dir +
            ((gamma - 1.0) * bn / beta2 - gamma) * beta) / energy_static);
    }
    let alpha = sqrt(max(1.0 - 1.0 / length(origin), 0.00001));
    let observer_shift = 1.0 / (alpha * energy_static);
    let ray = integrate_ray(origin, static_dir, time, observer_shift,
        u32(clamp(scene.misc.y, 128.0, 1024.0)), true);
    if (ray.fate == 1u) {
        return sky(ray.direction, observer_shift);
    }
    return ray.color;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let screen = in.uv * 2.0 - 1.0;
    let tan_half = scene.cam_right_fov.w;
    let aspect = scene.cam_up_aspect.w;
    let dir = normalize(
        scene.cam_forward_disk.xyz
            + scene.cam_right_fov.xyz * (screen.x * tan_half * aspect)
            + scene.cam_up_aspect.xyz * (-screen.y * tan_half),
    );
    let time = scene.cam_pos_time.w;
    var color = trace_ray(scene.cam_pos_time.xyz, dir, time);

    // A restrained vignette keeps the HUD legible without crushing the sky.
    let vignette = smoothstep(1.50, 0.35, length(screen));
    color *= 0.72 + 0.28 * vignette;

    return vec4<f32>(color, 1.0);
}
