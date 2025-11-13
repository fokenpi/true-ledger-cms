fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Tauri app failed to start");
}
