fn main() {
    // ui/themes.css is a copy of ui-common/themes.css, which every app shares word for word. The
    // copy is committed so the pages and the preview harness work on their own; CI fails if it
    // drifts.
    let shared = "../../../ui-common/themes.css";
    println!("cargo:rerun-if-changed={shared}");
    if let Ok(css) = std::fs::read(shared) {
        if std::fs::read("../ui/themes.css").ok().as_ref() != Some(&css) {
            std::fs::write("../ui/themes.css", css).expect("ui/themes.css");
        }
    }
    tauri_build::build()
}
