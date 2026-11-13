//! Prints the chunks of a `.3dm` file.
fn main() {
    let path = std::env::args().nth(1).expect("a file");
    let depth = std::env::args()
        .nth(2)
        .and_then(|d| d.parse().ok())
        .unwrap_or(3);
    let data = std::fs::read(path).expect("readable");
    print!("{}", curva_3dm::dump(&data, depth).expect("a .3dm file"));
}
