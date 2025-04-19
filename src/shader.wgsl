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
};

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    range: f32,
}
@group(2) @binding(0)
var<uniform> light: Light;

struct PointLights {
    lights: array<Light>,
}

@group(2) @binding(1)
var<storage, read> pointLights: PointLights;

struct DirectionalLightUniformData {
    color: vec3<f32>,
    light_direction: vec3<f32>,
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
    out.clip_position = camera.view_proj * world_position;
    out.tex_coords = model.tex_coords;
    out.tangent_position = tangent_matrix * world_position.xyz;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.tangent_light_position = tangent_matrix * light.position;
    out.position = model.position;
    
    return out;
}


 // Fragment shader

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
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.tex_coords);

    let light_distance = length(light.position - in.position);
    let attenuation = 2.0 / (1.0 + (light.range * 0.0014) * light_distance + (light.range * 0.000007) * (light_distance * light_distance)); 

    let ambient_strength = 0.75;
    let ambient_color = light.color * ambient_strength * attenuation;

    var tangent_normal = object_normal.xyz * 2.0 - 1.0;
    tangent_normal = normalize(tangent_normal);
    let light_dir = normalize(in.tangent_light_position - in.tangent_position);
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);

    let half_dir = normalize(view_dir + light_dir);
    
    let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength * attenuation;

    let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
    var specular_color = specular_strength * light.color * attenuation;
    
//    if (diffuse_strength <= 0.35) {
//        specular_color = vec3(0.0);
//    }
    var result = (ambient_color + diffuse_color + specular_color) * object_color.xyz;
    
    //directional light
//    let dl_light_dir = normalize(-directionalLight.light_direction);
//    let diffuse = vec3<f32>(0.5, 0.5, 0.5);
//    let specular = vec3<f32>(1.0, 1.0, 1.0);
//    let dl_diff = max(dot(tangent_normal, dl_light_dir), 0.0);
//    let dl_diffuse = diffuse * dl_diff ;

//    let dl_view_dir = max(normalize( + view_dir),0.0);
//    let dl_specular_strength = pow(max(dot(tangent_normal, dl_half_dir), 0.0), 32.0);
//    var dl_specular_color = directionalLight.color * dl_specular_strength ;
    
//    if (dl_diffuse_strength <= 0.35) {
//        dl_specular_color = vec3(0.0);
//    }
//    let dl_result = dl_diffuse * object_color.xyz;
//    result += dl_result;
    


    
    var len = arrayLength(&pointLights.lights);
// render over all light in Vec<light::Light> //
    for (var i =0u; i < len ; i++) {
        let lightpos = pointLights.lights[i].position;
        let lightcolor = pointLights.lights[i].color;
        let lightrange = pointLights.lights[i].range;

        //let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
        //let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.tex_coords);

        let light_distance = length(lightpos - in.position);
        let attenuation = 2.0 / (1.0 + (lightrange * 0.0014) * light_distance + (lightrange * 0.000007) * (light_distance * light_distance)); 

        //let ambient_strength = 0.05;
        //let ambient_color = lightcolor * ambient_strength * attenuation;

        let tangent_normal = object_normal.xyz * 2.0 - 1.0;
        let light_dir = normalize(in.tangent_light_position - in.tangent_position);
        let view_dir = normalize(in.tangent_view_position - in.tangent_position);

        let half_dir = normalize(view_dir + light_dir);
    
        let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
        let diffuse_color = lightcolor * diffuse_strength * attenuation;

        let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
        var specular_color = specular_strength * lightcolor * attenuation;
    
//        if (diffuse_strength <= 0.35) {
//            specular_color = vec3(0.0);
//        }
        let newresult = (diffuse_color + specular_color) * object_color.xyz;
        result += newresult;

    }


    return vec4<f32>(result, object_color.a);
}