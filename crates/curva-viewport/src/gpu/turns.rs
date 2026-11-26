//! Drawing the silhouettes of curved faces from their turns, one instance each (`turns.rs`).

use egui_wgpu::wgpu;

use super::shaders::{TURN_SHADER, table_wgsl};
use super::{LINE_DEPTH_BIAS, PipelineParts, SEGMENT_CORNERS, Uploaded, pipeline};
use crate::PlacedTurn;

/// The bytes of one [`PlacedTurn`] on the graphics card.
pub(crate) const TURN_SIZE: u64 = 68;

/// `turns` as the graphics card reads them: the shared line's ends, the two far corners, colour,
/// width (the default line's) and object, each drawn as its object's edges.
pub(crate) fn turn_bytes(turns: &[PlacedTurn]) -> Vec<u8> {
    let width = crate::line_width(None, false);
    let mut bytes = Vec::with_capacity(turns.len() * TURN_SIZE as usize);
    for turn in turns {
        for value in [turn.a, turn.b, turn.c, turn.d, turn.colour]
            .iter()
            .flatten()
            .chain([&width])
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let packed = super::upload::packed(turn.object, turn.kind as u32);
        bytes.extend_from_slice(&packed.to_le_bytes());
    }
    bytes
}

/// The pipeline drawing turns, as the model lines' pipeline draws segments, in their own colours.
pub(crate) fn turn_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    pipeline_layout: &wgpu::PipelineLayout,
) -> wgpu::RenderPipeline {
    pipeline(
        device,
        format,
        pipeline_layout,
        &PipelineParts {
            label: "curva silhouettes",
            shader: &format!("{}{TURN_SHADER}", table_wgsl()),
            stride: TURN_SIZE,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x3,
                5 => Float32, 6 => Uint32
            ],
            topology: wgpu::PrimitiveTopology::TriangleList,
            bias: LINE_DEPTH_BIAS,
            see_through: false,
            per_instance: true,
            smooth: true,
            behind: false,
        },
    )
}

/// Draws `turns` with `pipeline`, block by block: one segment's six corners each.
pub(crate) fn draw_turns(
    pass: &mut wgpu::RenderPass<'_>,
    pipeline: &wgpu::RenderPipeline,
    turns: &Uploaded<PlacedTurn>,
) {
    if turns.count == 0 {
        return;
    }
    pass.set_pipeline(pipeline);
    for (buffer, count) in &turns.chunks {
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..SEGMENT_CORNERS, 0..*count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineKind;

    #[test]
    fn a_turn_takes_its_size_in_bytes_and_draws_as_an_edge() {
        let turn = PlacedTurn {
            a: [1.0, 2.0, 3.0],
            b: [4.0; 3],
            c: [5.0; 3],
            d: [6.0; 3],
            colour: [0.5; 3],
            object: 9,
            kind: LineKind::Edge,
        };
        let bytes = turn_bytes(&[turn, turn]);
        assert_eq!(bytes.len() as u64, 2 * TURN_SIZE);
        assert_eq!(&bytes[..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[36..40], &6.0f32.to_le_bytes());
        let edge = LineKind::Edge as u32;
        assert_eq!(&bytes[64..68], &((edge << 24) | 9).to_le_bytes());
        // A mesh's turn draws as the mesh's edges.
        let mesh = PlacedTurn {
            kind: LineKind::MeshEdge,
            ..turn
        };
        let kind = LineKind::MeshEdge as u32;
        assert_eq!(
            &turn_bytes(&[mesh])[64..68],
            &((kind << 24) | 9).to_le_bytes()
        );
    }
}
