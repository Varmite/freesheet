fn main() {
    // The notes, questions, exams and courses are built into the program (src/content.rs),
    // so rebuild whenever they change.
    println!("cargo:rerun-if-changed=../content");
    // Android phones with 16 KB memory pages (new since Android 15) only load libraries
    // laid out for them. Newer NDKs do this already; say so for older ones too.
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if var("CARGO_CFG_TARGET_OS") == "android" && var("CARGO_CFG_TARGET_POINTER_WIDTH") == "64" {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
    }
    tauri_build::build()
}
