fn main() {
    // SQLx incorpora le migration nel binario. Su Rust stable questo build
    // script fa ricompilare il progetto quando cambia la cartella migrations/.
    println!("cargo:rerun-if-changed=migrations");

    // Su Android (Termux, il Galaxy S9 che fa da server) il collegamento va
    // limitato a un thread. LLD ne lancia uno per core e ognuno tiene la
    // propria copia delle strutture di link: sul telefono la memoria finisce a
    // meta' collegamento, e il sintomo non e' un messaggio chiaro ma un
    // segmentation fault di `cc`.
    //
    // Questa scelta sta qui e non in `.cargo/config.toml` per due motivi: e'
    // condizionata al target senza doverlo nominare, e non puo' essere
    // annullata da una `RUSTFLAGS` impostata nell'ambiente, che invece
    // sostituirebbe il file di configurazione invece di aggiungersi.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=-Wl,--threads=1");
    }
}
