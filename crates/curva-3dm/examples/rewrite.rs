//! Reads `.3dm` files and writes each again with Curva's writer, into a folder, for opening in Rhino:
//! `cargo run -p curva-3dm --example rewrite -- <out-folder> <files…>`.
//! Extrusion objects and revolved or summed surfaces are left out, as the export turns them into
//! B-reps and NURBS first.
use curva_3dm::model::{Geometry, Surface};

fn as_is(s: &Surface) -> bool {
    matches!(s, Surface::Nurbs(_) | Surface::Plane { .. })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().expect("an output folder"));
    std::fs::create_dir_all(&out).expect("output folder");
    for path in args {
        let mut model =
            curva_3dm::read::read(&std::fs::read(&path).expect("readable")).expect("a .3dm");
        model.objects.retain(|o| match &o.geometry {
            Geometry::Extrusion(_) => false,
            Geometry::Surface(s) => as_is(s),
            Geometry::Brep(b) => b.surfaces.iter().flatten().all(as_is),
            _ => true,
        });
        let name = std::path::Path::new(&path)
            .file_name()
            .expect("a file name");
        let target = out.join(name);
        std::fs::write(&target, curva_3dm::write::write(&model)).expect("written");
        println!("{} ({} objects)", target.display(), model.objects.len());
    }
}
