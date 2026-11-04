//! One-shot regenerator for docs/capability-matrix.md (kept out of the
//! build graph — run explicitly: `cargo run --example regen_capability_matrix`).
fn main() {
    let generated = dulce_de_leche::catalog::render_capability_matrix();
    std::fs::write("docs/capability-matrix.md", generated).expect("write matrix");
    println!("docs/capability-matrix.md regenerated");
}
