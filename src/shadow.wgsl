struct DirectionalLightUniformData {
    color: vec3<f32>,
    light_direction: vec3<f32>,
    intensity: f32,
    view_projection: mat4x4<f32>,
}

// Vertex shader

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) dummy_uv: vec2<f32>, 
};

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    range: f32,
}
struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
    range: f32,
    tangent_light_position: vec3<f32>,
}
@group(0) @binding(0)
var<uniform> light: Light;

struct PointLights {
    lights: array<PointLight>,
}

@group(0) @binding(1)
var<storage, read> pointLights: PointLights;


@group(0) @binding(2)
var<uniform> directionalLight: DirectionalLightUniformData;


@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = directionalLight.view_projection * vec4<f32>(model.position, 1.0);
    out.dummy_uv = vec2<f32>(0.0, 0.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) {}