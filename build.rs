fn main() {
    // SQLx incorpora le migration nel binario. Su Rust stable questo build
    // script fa ricompilare il progetto quando cambia la cartella migrations/.
    println!("cargo:rerun-if-changed=migrations");
}
