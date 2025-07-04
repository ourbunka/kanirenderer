// Vertex shader

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
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) tangent_position: vec3<f32>,
    @location(2) tangent_light_position: vec3<f32>,
    @location(3) tangent_view_position: vec3<f32>,
    @location(4) position: vec3<f32>,
    @location(5) world_tangent: vec3<f32>,
    @location(6) world_bitangent: vec3<f32>,
    @location(7) world_normal: vec3<f32>,
    @location(8) world_position: vec3<f32>,
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
@group(2) @binding(0)
var<uniform> light: Light;

@group(2) @binding(1)
var<uniform> pointLight: PointLight;

struct PointLights {
    lights: array<PointLight>,
}

@group(2) @binding(1)
var<storage, read> pointLights: PointLights;

struct DirectionalLightUniformData {
    color: vec3<f32>,
    light_direction: vec3<f32>,
    intensity: f32,
    view_projection: mat4x4<f32>,
}

@group(2) @binding(2)
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

    let normal_matrix = mat3x3<f32>(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );
    
    let world_normal = normalize(normal_matrix * model.normal);
    let world_tangent = normalize(normal_matrix * model.tangent);
    let world_bitangent = normalize(normal_matrix * model.bitangent);
    let tangent_matrix = transpose(mat3x3<f32>(world_tangent, world_bitangent, world_normal));
    
    let world_position = model_matrix * vec4<f32>(model.position, 1.0);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_position.xyz, 1.0);
    out.tex_coords = model.tex_coords;
    out.tangent_position = tangent_matrix * world_position.xyz;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.tangent_light_position = tangent_matrix * light.position;
    out.position = model.position;
    out.world_tangent = world_tangent;
    out.world_bitangent = world_bitangent;
    out.world_normal = world_normal;
    out.world_position = world_position.xyz;
    return out;
}

 // Fragment shader

fn reinnhard_tonemap(input: vec3<f32>)->vec3<f32>{
    let mapped_color = input.rgb / (input.rgb + vec3(1.0));
    return mapped_color;
}

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var t_normal: texture_2d<f32>;
@group(0) @binding(3)
var s_normal: sampler;


@fragment

fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color_texture: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords).rgba;
    var object_color: vec3<f32> = color_texture.rgb;
    let alpha = color_texture.a;

    let object_normal: vec3<f32> = textureSample(t_normal, s_normal, in.tex_coords).rgb;
    let light_distance = length(light.position - in.world_position);

    let constant = 1.0;
    let linear = 0.09;
    let quadratic = 0.032;
    
    // Adjust attenuation to respect the light's range
    let attenuation = 1.0 / (constant + linear * light_distance + quadratic * light_distance * light_distance);
    let range_attenuation = clamp(1.0 - pow(light_distance / light.range, 4.0), 0.0, 1.0);
    
    let ambient_light_color = vec3<f32>(1.0, 1.0, 1.0);
    let ambient_strength = 0.05;
    let ambient_color = ambient_light_color * ambient_strength;

    var tangent_normal = object_normal.rgb * 2.0 - 1.0;
    tangent_normal = normalize(tangent_normal);
    let light_dir = normalize(in.tangent_light_position - in.tangent_position);
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);

    let half_dir = normalize(view_dir + light_dir);
    
    let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
    var specular_color = specular_strength * light.color;

    var result = vec3<f32>(0.0, 0.0, 0.0);
    
    let tangent_matrix = mat3x3<f32>(in.world_tangent, in.world_bitangent, in.world_normal);

    //directional light
    let dl_world_normal = normalize(tangent_matrix * tangent_normal);
    let dl_light_dir = normalize(-directionalLight.light_direction);
    let dl_diffuse_factor = max(dot(dl_world_normal, dl_light_dir), 0.0);
    let dl_diffuse_color = dl_diffuse_factor* directionalLight.color * 10.0 ; //10.0 intensity

    let dl_view_dir = view_dir;
    let dl_half_dir = normalize(dl_light_dir+dl_view_dir);
    let dl_specular_factor = pow(max(dot(dl_world_normal, dl_half_dir), 0.0), 8.0); //8.0 is specular power
    var dl_specular_color = dl_specular_factor* directionalLight.color * 10.0 ; 
    
    let dl_result = (dl_diffuse_color + dl_specular_color) * object_color;
    //add directional light result
    result += dl_result;
    result +=  (ambient_color * object_color);

    //add movable point light result
    result += ((diffuse_color + specular_color)*attenuation*range_attenuation)* object_color;

    var lights = arrayLength(&pointLights.lights);
// render over all light in Vec<light::Light> //
    for (var i =0u; i < lights ; i++) {
        let lightpos = pointLights.lights[i].position;
        let lightcolor = pointLights.lights[i].color;
        let lightrange = pointLights.lights[i].range;

        let light_distance = length(lightpos - in.world_position);

        let constant = 1.0;
        let linear = 0.09;
        let quadratic = 0.032;
    
        // Adjust attenuation to respect the light's range
        let attenuation = 1.0 / (constant + linear * light_distance + quadratic * light_distance * light_distance);
        let range_attenuation = clamp(1.0 - pow(light_distance / lightrange, 4.0), 0.0, 1.0);
        
        let tangent_normal = object_normal.xyz * 2.0 - 1.0;
        let light_dir = normalize((tangent_matrix*lightpos) - in.tangent_position);
        let view_dir = normalize(in.tangent_view_position - in.tangent_position);

        let half_dir = normalize(view_dir + light_dir);
    
        let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
        let diffuse_color = lightcolor * diffuse_strength;

        let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
        var specular_color = specular_strength * lightcolor;
    
        let newresult = ((diffuse_color + specular_color)*attenuation*range_attenuation)* object_color;
        result += newresult;

    }
    let out = reinnhard_tonemap(result);
    return vec4<f32>(out,1.0);
}