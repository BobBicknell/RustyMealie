// RustyMeals Tauri core — library entry point.
//
// The mobile entry point (`run`) is annotated with `tauri::mobile_entry_point`
// so the same code path is used on Android/iOS and desktop.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .run(tauri::generate_context!())
        .expect("error while running RustyMeals");
}
