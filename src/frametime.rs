use std::time::Duration;

use bytemuck::Zeroable;

#[derive(Debug)]
pub struct FrameTimeGraph {
    pub max_points: usize,
    pub buffer: Vec<f32>, //in ms , max length 256
    pub current_index: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameTimeGraphRaw {
    pub vec: [[f32;2];256]
}

impl FrameTimeGraph {
    pub fn new() -> Self {
        Self {
            max_points: 256 as usize,
            buffer: vec![0.0; 256],
            current_index: 0,
        }
    }

    pub fn update(&mut self, delta_time: Duration) {
        let time_ms = delta_time.as_secs_f32() * 1000.0;
        self.buffer[self.current_index] = time_ms;
        self.current_index = (self.current_index +1)% self.max_points;
    }

    pub fn get_vertices(&self, w: f32, h:f32) -> FrameTimeGraphRaw {
        let graph_width = 400; // Desired width of the overlay
        let graph_height = 100; // Desired height of the overlay
        let width = w; // screen width
        let height = h; // screen height

        // Place the graph in the bottom-right corner
        let x_offset = width as f32 - graph_width as f32 - 25.0;
        let y_offset = height as f32 - graph_height as f32 - (height - graph_height as f32 - 25.0);

        let max_time = 8.333; 
        //let max_time = self.buffer.iter().copied().fold(0.0, f32::max);
        
        let mut raw = FrameTimeGraphRaw::zeroed();
        for i in 0..self.max_points {
            let time = self.buffer[i];
            //let x = 0.5 - (i as f32 / self.max_points as f32) * (graph_width as f32 / width as f32);
            //let y = -0.5 + (time / max_time) * (graph_height as f32 / height as f32);
            let x = x_offset + (i as f32 / self.max_points as f32) * graph_width as f32;
            let y = y_offset + ((time / max_time) * graph_height as f32);

            // Convert to NDC coordinates
            let x_ndc = 2.0 * (x / width as f32) - 1.0;
            let y_ndc = 2.0 * (y / height as f32) - 1.0;
            raw.vec[i] = [x_ndc, y_ndc];
        }
        raw
    }
}

