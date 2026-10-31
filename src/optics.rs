//! Exercise the actual WGSL integrator against analytic capture cones and
//! independent published Schwarzschild deflection values (Rs=1, M=0.5).
use std::error::Error;
use wgpu::util::DeviceExt;

pub fn validate() -> Result<(), Box<dyn Error>> {
    pollster::block_on(validate_gpu())
}

async fn validate_gpu() -> Result<(), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok_or("No GPU adapter for optics validation")?;
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("optics validation"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await?;
    let mut cases: Vec<(f32, f32, Option<f32>, u32)> = Vec::new();
    // (observer radius, angle from inward radial, expected deflection, fate)
    let critical_b = 3.0_f32 * 3.0_f32.sqrt() * 0.5;
    for r in [1.2_f32, 2.0, 10.0, 500.0] {
        let acute = (critical_b * (1.0 - 1.0 / r).sqrt() / r).asin();
        let edge = if r < 1.5 {
            std::f32::consts::PI - acute
        } else {
            acute
        };
        cases.push((r, edge - 0.0005, None, 0));
        cases.push((r, edge + 0.0005, None, 1));
    }
    // Reference deflections from independent quadrature, converted from M units.
    for (b, angle) in [
        (4.0_f32, 0.858730018_f32),
        (10.0, 0.236135995),
        (50.0, 0.041222539),
    ] {
        let r = 10000.0_f32;
        let psi = (b * (1.0 - 1.0 / r).sqrt() / r).asin();
        cases.push((r, psi, Some(angle), 1));
    }
    let mut kernel = String::from(
        r#"
@group(1) @binding(0) var<storage, read_write> validation: array<vec4<f32>>;
@compute @workgroup_size(1)
fn validate_main(@builtin(global_invocation_id) id: vec3<u32>) {
    var origin = vec3<f32>(0.0);
    var direction = vec3<f32>(0.0);
    switch id.x {
"#,
    );
    for (i, (r, theta, _, _)) in cases.iter().enumerate() {
        kernel.push_str(&format!(
            "case {i}u: {{ origin = vec3<f32>({r:.9}, 0.0, 0.0); direction = vec3<f32>({:.9}, {:.9}, 0.0); }}\n",
            -theta.cos(), theta.sin()).replace("\n", "
"));
    }
    kernel.push_str(
        r#"
        default: { return; }
    }
    let ray = integrate_ray(origin, direction, 0.0, 1.0, 512u, false);
    validation[id.x] = vec4<f32>(ray.direction, f32(ray.fate));
}
"#,
    );
    let shader_source = format!(
        "{}
{kernel}",
        include_str!("shaders/blackhole.wgsl")
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("actual blackhole WGSL plus validation entry"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });
    let empty = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[],
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&empty, &layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: "validate_main",
        compilation_options: Default::default(),
    });
    let output = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &vec![0u8; cases.len() * 16],
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (cases.len() * 16) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let empty_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &empty,
        entries: &[],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &empty_binding, &[]);
        pass.set_bind_group(1, &binding, &[]);
        pass.dispatch_workgroups(cases.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, (cases.len() * 16) as u64);
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()??;
    let mapped = slice.get_mapped_range();
    let results: &[[f32; 4]] = bytemuck::cast_slice(&mapped);
    for (i, ((r, theta, reference, fate), result)) in cases.iter().zip(results).enumerate() {
        if result[3] as u32 != *fate {
            return Err(format!("Ray {i} at r={r}: fate {}, expected {fate}", result[3]).into());
        }
        if let Some(expected) = reference {
            let initial = glam::Vec3::new(-theta.cos(), theta.sin(), 0.0);
            let outgoing = glam::Vec3::new(result[0], result[1], result[2]);
            let measured = initial.dot(outgoing).clamp(-1.0, 1.0).acos();
            let error = (measured - expected).abs();
            println!("Deflection: {measured:.7} rad; reference {expected:.7}; error {error:.2e}");
            if error > 0.0002 {
                return Err(format!("Deflection error too large for ray {i}: {error}").into());
            }
        } else {
            println!("Capture cone: r={r:6.1} Rs, theta={theta:.7}: fate {fate} OK");
        }
    }
    println!("All {} GPU optics checks passed.", cases.len());
    Ok(())
}
