//! The Surfaces tab's commands.

pub(crate) mod blend_srf;
pub(crate) mod dup_border;
pub(crate) mod dup_edge;
pub(crate) mod dup_face_border;
pub(crate) mod edge_srf;
pub(crate) mod extend_srf;
pub(crate) mod extract_isocurve;
pub(crate) mod extract_srf;
pub(crate) mod extrude_crv;
pub(crate) mod extrude_crv_along_crv;
pub(crate) mod extrude_crv_tapered;
pub(crate) mod extrude_crv_to_point;
pub(crate) mod extrude_srf;
pub(crate) mod fillet_srf;
pub(crate) mod interp_crv_on_srf;
pub(crate) mod loft;
pub(crate) mod match_srf;
pub(crate) mod network_srf;
pub(crate) mod offset_crv_on_srf;
pub(crate) mod offset_srf;
pub(crate) mod patch;
pub(crate) mod planar_srf;
pub(crate) mod plane;
pub(crate) mod project;
pub(crate) mod pull;
pub(crate) mod rail_revolve;
pub(crate) mod rebuild_uv;
pub(crate) mod revolve;
pub(crate) mod shrink_trimmed_srf;
pub(crate) mod split_srf;
pub(crate) mod srf_pt;
mod support;
pub(crate) mod sweep1;
pub(crate) mod sweep2;
pub(crate) mod unroll_srf;
pub(crate) mod untrim;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    recipe!("Plane", [], true, plane::Plane),
    recipe!("SrfPt", [], true, srf_pt::SrfPt),
    recipe!("ExtrudeSrf", [], true, extrude_srf::ExtrudeSrf),
    recipe!("DupEdge", [], true, dup_edge::DupEdge),
    recipe!("DupBorder", [], true, dup_border::DupBorder),
    recipe!(
        "ExtrudeCrvTapered",
        [],
        true,
        extrude_crv_tapered::ExtrudeCrvTapered
    ),
    recipe!(
        "ExtrudeCrvAlongCrv",
        [],
        true,
        extrude_crv_along_crv::ExtrudeCrvAlongCrv
    ),
    recipe!(
        "ExtrudeCrvToPoint",
        [],
        true,
        extrude_crv_to_point::ExtrudeCrvToPoint
    ),
    recipe!("RailRevolve", [], true, rail_revolve::RailRevolve),
    recipe!("ExtractSrf", [], true, extract_srf::ExtractSrf),
    recipe!("DupFaceBorder", [], true, dup_face_border::DupFaceBorder),
    recipe!("EdgeSrf", [], true, edge_srf::EdgeSrf),
    recipe!("NetworkSrf", [], true, network_srf::NetworkSrf),
    recipe!("Patch", [], true, patch::Patch),
    recipe!("ExtendSrf", [], true, extend_srf::ExtendSrf),
    recipe!("Untrim", [], true, untrim::Untrim),
    recipe!("Project", [], true, project::Project),
    recipe!("RebuildUV", [], true, rebuild_uv::RebuildUv),
    recipe!(
        "ExtractIsocurve",
        [],
        true,
        extract_isocurve::ExtractIsocurve
    ),
    recipe!(
        "ShrinkTrimmedSrf",
        [],
        true,
        shrink_trimmed_srf::ShrinkTrimmedSrf
    ),
    recipe!("BlendSrf", [], true, blend_srf::BlendSrf),
    recipe!("MatchSrf", [], true, match_srf::MatchSrf),
    recipe!("Sweep2", [], true, sweep2::Sweep2),
    recipe!("Pull", [], true, pull::Pull),
    recipe!(
        "InterpCrvOnSrf",
        [],
        true,
        interp_crv_on_srf::InterpCrvOnSrf
    ),
    recipe!(
        "OffsetCrvOnSrf",
        [],
        true,
        offset_crv_on_srf::OffsetCrvOnSrf
    ),
    recipe!("UnrollSrf", [], true, unroll_srf::UnrollSrf),
    recipe!("FilletSrf", [], true, fillet_srf::FilletSrf<false>),
    recipe!("ChamferSrf", [], true, fillet_srf::FilletSrf<true>),
    recipe!("SplitSrf", [], true, split_srf::SplitSrf),
    recipe!("ExtrudeCrv", [], true, extrude_crv::ExtrudeCrv),
    recipe!("PlanarSrf", [], true, planar_srf::PlanarSrf),
    recipe!("Revolve", [], true, revolve::Revolve),
    recipe!("Loft", [], true, loft::Loft),
    recipe!("Sweep1", [], true, sweep1::Sweep1),
    recipe!("OffsetSrf", [], true, offset_srf::OffsetSrf),
];
