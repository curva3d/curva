//! The Analyze tab's commands.

pub(crate) mod angle;
pub(crate) mod area;
pub(crate) mod area_centroid;
pub(crate) mod audit;
pub(crate) mod bounding_box;
pub(crate) mod contour;
pub(crate) mod count;
pub(crate) mod curvature_graph;
pub(crate) mod dir;
pub(crate) mod distance;
pub(crate) mod draft_analysis;
pub(crate) mod evaluate_pt;
pub(crate) mod intersect;
pub(crate) mod length;
pub(crate) mod plan;
pub(crate) mod radius;
pub(crate) mod section;
pub(crate) mod show_edges;
pub(crate) mod silhouette;
mod support;
pub(crate) mod volume_centroid;
pub(crate) mod what;
pub(crate) mod zebra;

/// This folder's commands, as the command list holds them; `commands/order.rs` keeps their
/// place in it.
pub(crate) static COMMANDS: &[crate::commands::CommandDef] = &[
    listed!("Dir", [], true, true, dir::Dir),
    listed!(
        "BoundingBox",
        ["bb"],
        true,
        false,
        bounding_box::BoundingBox
    ),
    listed!("Distance", ["dist"], true, false, distance::Distance),
    listed!("Length", ["len"], true, false, length::Length),
    listed!("Area", [], true, false, area::Area),
    recipe!("Angle", [], true, angle::Angle),
    recipe!("EvaluatePt", [], true, evaluate_pt::EvaluatePt),
    recipe!("Radius", [], true, radius::Radius),
    recipe!("What", [], true, what::What),
    recipe!("AreaCentroid", [], true, area_centroid::AreaCentroid),
    recipe!("VolumeCentroid", [], true, volume_centroid::VolumeCentroid),
    recipe!("ShowEdges", [], true, show_edges::ShowEdges),
    recipe!("Plan", [], false, plan::Plan),
    listed!("Zebra", [], false, false, zebra::Zebra),
    listed!("Count", [], false, false, count::Count),
    listed!("Audit", [], false, false, audit::Audit),
    listed!(
        "DraftAnalysis",
        [],
        false,
        false,
        draft_analysis::DraftAnalysis
    ),
    recipe!("Section", [], true, section::Section),
    recipe!("Contour", [], true, contour::Contour),
    recipe!("Intersect", [], true, intersect::Intersect),
    recipe!("CurvatureGraph", [], true, curvature_graph::CurvatureGraph),
    recipe!("Silhouette", [], true, silhouette::Silhouette),
];
