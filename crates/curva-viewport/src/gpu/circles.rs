//! Drawing whole circles from their centre and axes, one instance each (#153).

use egui_wgpu::wgpu;

use super::shaders::{CIRCLE_SHADER, inked, table_wgsl};
use super::{LINE_DEPTH_BIAS, PipelineParts, SEGMENT_CORNERS, Uploaded, pipeline};
use crate::{CIRCLE_SEGMENTS, PlacedCircle};

/// The bytes of one [`PlacedCircle`] on the graphics card.
pub(crate) const CIRCLE_SIZE: u64 = 56;

/// `circles` as the graphics card reads them: centre, axes, colour, width, object.
pub(crate) fn circle_bytes(circles: &[PlacedCircle]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(circles.len() * CIRCLE_SIZE as usize);
    for circle in circles {
        for value in [circle.centre, circle.x, circle.y, circle.colour]
            .iter()
            .flatten()
            .chain([&circle.width])
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&circle.object.to_le_bytes());
    }
    bytes
}

/// The pipeline drawing circles, as the model lines' pipeline draws segments; `ink` draws them in
/// black ink, for the Hidden display mode (`inked`).
pub(crate) fn circle_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    pipeline_layout: &wgpu::PipelineLayout,
    ink: bool,
) -> wgpu::RenderPipeline {
    let shader = if ink {
        inked(CIRCLE_SHADER)
    } else {
        CIRCLE_SHADER.to_owned()
    };
    pipeline(
        device,
        format,
        pipeline_layout,
        &PipelineParts {
            label: if ink {
                "curva inked circles"
            } else {
                "curva circles"
            },
            shader: &format!(
                "{}const CIRCLE_SEGMENTS: u32 = {CIRCLE_SEGMENTS}u;\n{shader}",
                table_wgsl()
            ),
            stride: CIRCLE_SIZE,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32,
                5 => Uint32
            ],
            topology: wgpu::PrimitiveTopology::TriangleList,
            bias: LINE_DEPTH_BIAS,
            see_through: false,
            per_instance: true,
        },
    )
}

/// Draws `circles` with `pipeline`, block by block: every circle's segments, six corners each.
pub(crate) fn draw_circles(
    pass: &mut wgpu::RenderPass<'_>,
    pipeline: &wgpu::RenderPipeline,
    circles: &Uploaded<PlacedCircle>,
) {
    if circles.count == 0 {
        return;
    }
    pass.set_pipeline(pipeline);
    for (buffer, count) in &circles.chunks {
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..SEGMENT_CORNERS * CIRCLE_SEGMENTS, 0..*count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_takes_its_size_in_bytes() {
        let circle = PlacedCircle {
            centre: [1.0, 2.0, 3.0],
            x: [4.0; 3],
            y: [5.0; 3],
            colour: [0.5; 3],
            width: -2.0,
            object: 9,
        };
        let bytes = circle_bytes(&[circle, circle]);
        assert_eq!(bytes.len() as u64, 2 * CIRCLE_SIZE);
        assert_eq!(&bytes[..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[48..52], &(-2.0f32).to_le_bytes());
        assert_eq!(&bytes[52..56], &9u32.to_le_bytes());
    }
}
