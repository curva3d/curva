//! Clipping planes on views Bevy draws: Bevy has none, so its standard surface is extended (with
//! Bevy's own `ExtendedMaterial`) by a few lines that leave out what lies beyond a view's planes
//! (`clip.wesl`). The filled cut faces are Curva's own, worked out from the engine's exact cuts and
//! handed to Bevy with the other faces (`curva-app/src/clipping.rs`).
// CUSTOM BEVY PART: clip-planes

use bevy::{
    asset::uuid_handle,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

/// The surface every Bevy view draws faces with: Bevy's standard one, cut by the view's planes.
pub(super) type CurvaMaterial = ExtendedMaterial<StandardMaterial, Clipping>;

const CLIP_SHADER: Handle<Shader> = uuid_handle!("6c8f6a8e-3b0c-4f7e-9a51-2d6b0c1e7f42");

/// The most planes one view is cut by, as Curva's own drawing allows (`crate::MOST_CLIPS`).
const MOST: usize = 4;

/// The planes cutting a view, for the graphics card.
#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Clips {
    planes: [Vec4; MOST],
    /// x: how many planes are in use.
    count: UVec4,
}

impl Clips {
    /// Up to four planes `[nx, ny, nz, d]`, each cutting away where `n·p + d > 0`.
    pub(super) fn of(planes: &[[f32; 4]]) -> Self {
        let mut clips = Self::default();
        for (slot, plane) in clips.planes.iter_mut().zip(planes) {
            *slot = Vec4::from_array(*plane);
        }
        clips.count.x = planes.len().min(MOST) as u32;
        clips
    }
}

/// The extension Bevy's standard surface is given: the view's planes.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub(super) struct Clipping {
    #[uniform(100)]
    pub(super) clips: Clips,
}

impl MaterialExtension for Clipping {
    fn fragment_shader() -> ShaderRef {
        CLIP_SHADER.into()
    }
}

/// Teaches `app` the clipped surface: its program and Bevy's plugin for it.
pub(super) fn install(app: &mut App) {
    let shader = Shader::from_wesl(include_str!("clip.wesl"), "curva/clip.wesl");
    let _ = app
        .world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(CLIP_SHADER.id(), shader);
    app.add_plugins(MaterialPlugin::<CurvaMaterial>::default());
}
