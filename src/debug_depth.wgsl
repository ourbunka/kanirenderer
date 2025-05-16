struct VertexInput{
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Define quad size and position in clip space
    let quad_size = vec2<f32>(0.4, 0.4); // Size (e.g., 20% of screen width/height)
    let quad_offset = vec2<f32>(0.5, 0.5); // Offset to top-right (adjust as needed)
    
    // Scale and translate the quad
    let scaled_pos = input.position * quad_size + quad_offset;

    out.position = vec4<f32>(scaled_pos, 0.0, 1.0);
    // Flip the v-coordinate to correct upside-down texture
    out.uv = vec2<f32>(input.position.x * 0.5 + 0.5, 1.0 - (input.position.y * 0.5 + 0.5));
    return out;
}

//fragment
@group(0) @binding(0)
var t_depth: texture_depth_2d;
@group(0) @binding(1)
var s_depth: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let depth = textureSample(t_depth, s_depth, input.uv);

    let near = 0.1;
    let far = 10000.0;

    let linear_depth = near * far / (far - depth * ( far - near ));
    let normalized_depth = linear_depth / far;

    // Add a white border if UV is near the edge
    let border_width = 0.01;
    if (input.uv.x < border_width || input.uv.x > (1.0 - border_width) || input.uv.y < border_width || input.uv.y > (1.0 - border_width)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); // White border
    }
    return vec4<f32>(normalized_depth, normalized_depth, normalized_depth, 1.0);
}