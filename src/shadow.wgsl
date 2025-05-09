// Vertex shader
struct DirectionalLightUniformData {
    color: vec3<f32>,
    light_direction: vec3<f32>,
    intensity: f32,
    view_projection: mat4x4<f32>,
}

struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> camera: Camera;

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>
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
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let world_position = model_matrix * vec4<f32>(model.position, 1.0);
    var out: VertexOutput;
    out.clip_position = directionalLight.view_projection * world_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) {}