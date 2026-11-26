//! The built-in styles: today's display modes, as styles (§2.3). They cannot be changed, only
//! copied.

use super::*;

/// The built-in styles' names, in the order they are listed.
pub const BUILT_IN: [&str; 6] = [
    "Wireframe",
    "Shaded",
    "Ghosted",
    "Hidden",
    "Technical",
    "Massing",
];

/// The sky of the Massing style (#551): light blue at the top to grey at the bottom.
const MASSING_TOP: Colour = Colour([170, 204, 232]);
const MASSING_BOTTOM: Colour = Colour([184, 184, 179]);

// Shaded's settings, which every built-in style starts from.
const USED_FOR: UsedFor = UsedFor {
    views: true,
    plans: false,
    sections: false,
    sheets: true,
};
const BACKGROUND: Background = Background {
    fill: Fill::Theme,
    top: Colour([43, 47, 54]),
    bottom: Colour([27, 29, 34]),
    grain: 0.0,
};
const GRID: Grid = Grid {
    shown: true,
    colour: None,
    over_objects: false,
    axes: true,
    world_axes: true,
    z_axis: false,
};
const LIGHTING: Lighting = Lighting {
    vector: VectorFaces::Tones,
    occlusion: OcclusionLight::Studio,
    rendered: RenderedLight::SunAndSky,
    shine: 0.6,
    ambient: Colour([25, 25, 25]),
};
const SUN: Sun = Sun {
    compass: 150.0,
    height: 40.0,
};
const SHADOWS: Shadows = Shadows {
    shown: false,
    edge: ShadowEdge::Soft,
    colour: Colour::BLACK,
    strength: 0.35,
    softness: 6.0,
    ground_only: false,
};
const OCCLUSION: AmbientOcclusion = AmbientOcclusion {
    shown: false,
    strength: 0.45,
    spread: 400.0,
    colour: Colour::BLACK,
};
const RENDERED: RenderedLook = RenderedLook {
    material_from: MaterialFrom::Material,
    textures: true,
    environment: Environment::Sky,
    reflections: 0.35,
    exposure: 0.0,
    glow: false,
};
const FINISH: Finish = Finish {
    grain: 0.0,
    vignette: 0.0,
    wash: 0.0,
};
const FACES: Faces = Faces {
    shown: true,
    colour: ColourFrom::Object,
    opacity: 1.0,
    back: BackFaces::AsFront,
    flat: false,
};
const EDGES: Edges = Edges {
    shown: true,
    width: LineWidth::Object(1.0),
    colour: LineColour::One(Colour::EDGE),
    pattern: Pattern::Solid,
};
/// A line of its own in its object's colour, as a SubD's cage edges and a mesh's wires draw in
/// every view (#472).
const OWN_LINE: LineLook = LineLook {
    shown: true,
    width: LineWidth::Object(1.0),
    colour: Some(LineColour::Object),
};
/// A line of its own drawn as the edges are.
const EDGE_LINE: LineLook = LineLook {
    shown: true,
    width: LineWidth::Object(1.0),
    colour: None,
};
const NURBS: NurbsLook = NurbsLook {
    faces: FACES,
    edges: EDGES,
    tangent_seams: true,
    naked_edges: false,
    isocurves: LineLook {
        shown: false,
        width: LineWidth::Object(0.5),
        colour: None,
    },
    isocurves_on_flat: false,
    outline: OutlineChoice::AsStyle,
};
const MESHES: MeshLook = MeshLook {
    same_as_nurbs: true,
    faces: FACES,
    edges: EDGES,
    outline: OutlineChoice::AsStyle,
    // Rhino 8's Shaded shows mesh wires, one pixel wide (#627).
    wires: Wires {
        shown: true,
        width: WireWidth::OnePixel,
        colour: Some(LineColour::Object),
    },
    vertices: false,
};
const SUBD: SubdLook = SubdLook {
    same_as_nurbs: true,
    faces: FACES,
    outline: OutlineChoice::AsStyle,
    smooth: OWN_LINE,
    creases: EDGE_LINE,
    boundary: EDGE_LINE,
    reflected_half: true,
    symmetry_axis: true,
};
const CURVES: CurveLook = CurveLook {
    shown: true,
    width: LineWidth::Object(1.0),
    colour: LineColour::Object,
    pattern: Pattern::Solid,
    ends: LineEnds::Round,
    curvature: false,
};
const POINTS: PointLook = PointLook {
    shown: true,
    size: 3.0,
    shape: PointShape::Square,
    clouds: true,
    cloud_size: 2.0,
};
const ANNOTATION: AnnotationLook = AnnotationLook {
    text: true,
    dimensions: true,
    colour: LineColour::Object,
};
const LIGHTS: LightObjects = LightObjects {
    shown: true,
    light_with_them: false,
};
const CLIPPING: ClippingLook = ClippingLook {
    planes_shown: false,
    fill: true,
    fill_colour: LineColour::Object,
    hatch: Hatch::Plain,
    cut_edges: true,
    cut_edge_width: 0.75,
};
const OUTLINES: Outlines = Outlines {
    shown: true,
    width: 0.25,
    colour: Colour::EDGE,
    folds: false,
    where_objects_meet: false,
};
const HIDDEN_LINES: HiddenLines = HiddenLines {
    shown: false,
    pattern: Pattern::Dotted,
    width: LineWidth::Object(1.0),
    colour: Colour::BLACK,
    opacity: 1.0,
};
const LINE_RULES: LineRules = LineRules {
    weight: 1.0,
    by_distance: false,
    near: 5.0,
    far: 50.0,
    far_width: 0.3,
    far_fade: 0.6,
};
const DRAWINGS: Drawings = Drawings {
    cut: Cut {
        width: 0.5,
        colour: Colour::BLACK,
        fill: CutFill::Hatch,
        fill_colour: Colour([26, 26, 26]),
        hatch: Hatch::Diagonal,
        hatch_scale: 1.0,
        plan_height: 1.2,
    },
    beyond: Beyond {
        shown: true,
        width: 0.18,
        colour: Colour::BLACK,
        fade: true,
        depth: 10.0,
        faces: false,
    },
    above: Above::Dashed,
    above_colour: Colour::BLACK,
    below: false,
};
const BEHAVIOUR: Behaviour = Behaviour {
    selection_colour: Colour([224, 120, 47]),
    selected_show_through: false,
    locked: Locked::Greyed,
    locked_opacity: 0.5,
    boxes_while_moving: false,
    transparency: true,
    highlights: true,
};

impl ViewStyle {
    /// The built-in style called `name`, if there is one.
    #[must_use]
    pub fn built_in(name: &str) -> Option<Self> {
        Some(match name {
            "Wireframe" => Self::wireframe(),
            "Shaded" => Self::shaded(),
            "Ghosted" => Self::ghosted(),
            "Hidden" => Self::hidden(),
            "Technical" => Self::technical(),
            "Massing" => Self::massing(),
            _ => return None,
        })
    }

    /// Surfaces and solids in their own colours and lit, with dark edges and outlines on top.
    /// Every other built-in style starts from this one.
    #[must_use]
    pub fn shaded() -> Self {
        Self {
            name: "Shaded".into(),
            engine: Engine::Occlusion,
            used_for: USED_FOR,
            background: BACKGROUND,
            grid: GRID,
            lighting: LIGHTING,
            sun: SUN,
            shadows: SHADOWS,
            occlusion: OCCLUSION,
            rendered: RENDERED,
            finish: FINISH,
            nurbs: NURBS,
            meshes: MESHES,
            subd: SUBD,
            curves: CURVES,
            points: POINTS,
            annotation: ANNOTATION,
            lights: LIGHTS,
            clipping: CLIPPING,
            outlines: OUTLINES,
            hidden_lines: HIDDEN_LINES,
            line_rules: LINE_RULES,
            drawings: DRAWINGS,
            behaviour: BEHAVIOUR,
            unknown: Vec::new(),
        }
    }

    /// Lines only: curves, and the edges of surfaces and solids in their objects' colours.
    #[must_use]
    pub fn wireframe() -> Self {
        let mut style = Self::shaded();
        style.name = "Wireframe".into();
        style.engine = Engine::Vector;
        style.lighting.vector = VectorFaces::Flat;
        style.nurbs.faces.shown = false;
        style.nurbs.edges.colour = LineColour::Object;
        style.outlines.shown = false;
        style
    }

    /// Surfaces and solids see-through at half strength, so the edges behind them show.
    #[must_use]
    pub fn ghosted() -> Self {
        let mut style = Self::shaded();
        style.name = "Ghosted".into();
        style.nurbs.faces.opacity = 0.5;
        // The light wireframe edges show through the faces, in their objects' colours.
        style.nurbs.edges.colour = LineColour::Object;
        style
    }

    /// Rhino 8's Hidden mode (#164): faces flat in paper white hiding what is behind them, and
    /// the visible lines and outlines in black ink, on white.
    #[must_use]
    pub fn hidden() -> Self {
        let mut style = Self::shaded();
        style.name = "Hidden".into();
        style.engine = Engine::Vector;
        style.lighting.vector = VectorFaces::Flat;
        style.background.fill = Fill::OneColour;
        style.background.top = Colour::WHITE;
        style.background.bottom = Colour::WHITE;
        style.nurbs.faces.colour = ColourFrom::One(Colour::WHITE);
        ink(&mut style);
        // A mesh shows only its outline, no wires, as in Rhino 8 (#663).
        style.meshes.wires.shown = false;
        style
    }

    /// Rhino 8's Technical mode (#164): as Hidden, with the lines faces hide drawn dotted.
    #[must_use]
    pub fn technical() -> Self {
        let mut style = Self::hidden();
        style.name = "Technical".into();
        style.hidden_lines.shown = true;
        style
    }

    /// A white study model (#551): white faces lit by the sun, with their shadows, thin black
    /// edges and thick black outlines, on a sky shading from light blue to grey.
    #[must_use]
    pub fn massing() -> Self {
        let mut style = Self::shaded();
        style.name = "Massing".into();
        style.lighting.occlusion = OcclusionLight::Sun;
        style.shadows.shown = true;
        style.background.fill = Fill::Gradient;
        style.background.top = MASSING_TOP;
        style.background.bottom = MASSING_BOTTOM;
        style.nurbs.faces.colour = ColourFrom::One(Colour::WHITE);
        ink(&mut style);
        style.outlines.width = 0.75;
        style
    }
}

/// Every line of `style` in black ink: edges, curves, wires, annotation and outlines.
fn ink(style: &mut ViewStyle) {
    style.nurbs.edges.colour = LineColour::One(Colour::BLACK);
    style.curves.colour = LineColour::One(Colour::BLACK);
    style.outlines.colour = Colour::BLACK;
    let black = Some(LineColour::One(Colour::BLACK));
    style.meshes.wires.colour = black;
    style.subd.smooth.colour = black;
    style.annotation.colour = LineColour::One(Colour::BLACK);
}
