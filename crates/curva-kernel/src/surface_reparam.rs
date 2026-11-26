//! A plain surface's parameter box, read and changed, as Rhino's Reparameterize does for surfaces
//! (#741): a one-face surface whose face is its whole surface, its knots stretched to the box
//! asked for. The shape stays exactly as it was.

use crate::KernelError;
use crate::body::Body;
use crate::surface::NurbsSurface;

impl Body {
    /// The one face's surface, when the body is a single untrimmed face.
    fn plain_surface(&self) -> Result<NurbsSurface, KernelError> {
        let record = self.record()?;
        let refuse =
            || KernelError("only an untrimmed single surface can be reparameterized".to_owned());
        if record.faces.len() != 1 || record.faces[0].loops.len() != 1 {
            return Err(refuse());
        }
        let surface = NurbsSurface::of_face(&record, 0)?;
        let matches = surface
            .domain()
            .iter()
            .flatten()
            .zip(record.faces[0].domain.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-9 * (1.0 + a.abs()));
        if matches { Ok(surface) } else { Err(refuse()) }
    }

    /// The surface's parameter box, `[[u0, u1], [v0, v1]]`.
    pub fn surface_domain(&self) -> Result<[[f64; 2]; 2], KernelError> {
        Ok(self.plain_surface()?.domain())
    }

    /// How long the surface's edges along u and along v are (its first edge each way), which
    /// Rhino's `Automatic` gives as the box.
    pub fn surface_edge_lengths(&self) -> Result<[f64; 2], KernelError> {
        let surface = self.plain_surface()?;
        let [[u0, _], [v0, _]] = surface.domain();
        let along_u = surface.iso_v(v0).to_curve()?.length()?;
        let along_v = surface.iso_u(u0).to_curve()?.length()?;
        Ok([along_u, along_v])
    }

    /// The same surface with parameter box `[[u0, u1], [v0, v1]]`.
    pub fn surface_reparameterized(&self, domain: [[f64; 2]; 2]) -> Result<Body, KernelError> {
        let mut surface = self.plain_surface()?;
        let old = surface.domain();
        // The engine holds a flat surface as a plane measured in distances, whatever box it is
        // given, so a plane keeps its own box; asked for it again, nothing changes.
        let flat = matches!(
            self.record()?.faces[0].surface,
            solidcore_record::brep::Surface::Plane { .. }
        );
        if flat {
            let same = old
                .iter()
                .flatten()
                .zip(domain.iter().flatten())
                .all(|(a, b)| (a - b).abs() < 1e-9 * (1.0 + a.abs()));
            return if same {
                Ok(self.clone())
            } else {
                Err(KernelError(
                    "a flat surface's parameters are its distances in Curva, so it keeps its box"
                        .to_owned(),
                ))
            };
        }
        let map = |knots: &mut Vec<f64>, from: [f64; 2], to: [f64; 2]| {
            let scale = (to[1] - to[0]) / (from[1] - from[0]);
            for k in knots.iter_mut() {
                *k = to[0] + (*k - from[0]) * scale;
            }
        };
        map(&mut surface.u_knots, old[0], domain[0]);
        map(&mut surface.v_knots, old[1], domain[1]);
        surface.to_body()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, Curve};

    /// A curved sheet: an arch ruled to a line, 10 along and 4 across.
    fn arch() -> Body {
        let bow = Curve::control_point(vec![[0.0; 3], [5.0, 0.0, 3.0], [10.0, 0.0, 0.0]], 2, false)
            .unwrap();
        let line = Curve::line([0.0, 4.0, 0.0], [10.0, 4.0, 0.0]).unwrap();
        Body::ruled(&bow, &line, false).unwrap()
    }

    #[test]
    fn a_curved_surfaces_box_changes_and_its_shape_does_not() {
        let sheet = arch();
        let area = sheet.area().unwrap();
        let moved = sheet
            .surface_reparameterized([[2.0, 5.0], [7.0, 9.0]])
            .unwrap();
        assert_eq!(moved.surface_domain().unwrap(), [[2.0, 5.0], [7.0, 9.0]]);
        assert!((moved.area().unwrap() - area).abs() < 1e-6);
    }

    #[test]
    fn a_plane_keeps_its_box_of_distances() {
        let plane = Body::ruled(
            &Curve::line([0.0; 3], [10.0, 0.0, 0.0]).unwrap(),
            &Curve::line([0.0, 4.0, 0.0], [10.0, 4.0, 0.0]).unwrap(),
            false,
        )
        .unwrap();
        let own = plane.surface_domain().unwrap();
        assert!(plane.surface_reparameterized(own).is_ok());
        assert!(
            plane
                .surface_reparameterized([[2.0, 5.0], [7.0, 9.0]])
                .is_err()
        );
    }
}
