//! Damages `.3dm` files as the reading test does and reports the slow cases, read and import
//! timed apart: `cargo run --release -p curva-3dm --example damage -- [import] <files…>`.
use std::time::Instant;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let import = args.first().is_some_and(|a| a == "import");
    if import {
        args.remove(0);
    }
    for path in args {
        let data = std::fs::read(&path).expect("readable");
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for case in 0..200 {
            let mut damaged = data.clone();
            let mut spots = Vec::new();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let at = 32 + (seed as usize) % (damaged.len() - 32);
                damaged[at] = (seed >> 32) as u8;
                spots.push(at);
            }
            let start = Instant::now();
            let read = curva_3dm::read::read(&damaged);
            let read_time = start.elapsed().as_secs_f64();
            let mut import_time = 0.0;
            if import && let Ok(model) = &read {
                let start = Instant::now();
                let _ = curva_3dm::import::open(model);
                import_time = start.elapsed().as_secs_f64();
            }
            if read_time > 0.5 || import_time > 2.0 {
                println!(
                    "{path} case {case} at {spots:?}: read {read_time:.2}s import {import_time:.2}s"
                );
            }
        }
        println!("{path} done");
    }
}
