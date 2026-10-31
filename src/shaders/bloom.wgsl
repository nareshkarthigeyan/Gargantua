// Half-resolution bright-pass and separable Gaussian bloom.

struct Params {
    values: vec4<f32>, // threshold, unused, offset x, offset y
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

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

@fragment
fn bright_fs(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, src_sampler, in.uv).rgb;
    let m = max(max(c.r, c.g), c.b);
    let threshold = params.values.x;
    let soft = smoothstep(threshold, threshold + 0.8, m);
    return vec4<f32>(c * soft, 1.0);
}

@fragment
fn blur_fs(in: VOut) -> @location(0) vec4<f32> {
    let step = params.values.zw;
    let uv = in.uv;
    var col = textureSample(src_tex, src_sampler, uv).rgb * 0.227027;
    col += textureSample(src_tex, src_sampler, uv + step * 1.0).rgb * 0.1945946;
    col += textureSample(src_tex, src_sampler, uv - step * 1.0).rgb * 0.1945946;
    col += textureSample(src_tex, src_sampler, uv + step * 2.0).rgb * 0.1216216;
    col += textureSample(src_tex, src_sampler, uv - step * 2.0).rgb * 0.1216216;
    col += textureSample(src_tex, src_sampler, uv + step * 3.0).rgb * 0.054054;
    col += textureSample(src_tex, src_sampler, uv - step * 3.0).rgb * 0.054054;
    col += textureSample(src_tex, src_sampler, uv + step * 4.0).rgb * 0.016216;
    col += textureSample(src_tex, src_sampler, uv - step * 4.0).rgb * 0.016216;
    return vec4<f32>(col, 1.0);
}
