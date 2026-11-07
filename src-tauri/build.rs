fn main() {
    // The notes, questions, exams and courses are built into the program (src/content.rs),
    // so rebuild whenever they change.
    println!("cargo:rerun-if-changed=../content");
    tauri_build::build()
}
