#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dialog;
mod model;
mod store;

use std::sync::Mutex;
use store::Store;

pub struct AppState {
    pub store: Store,
    pub data: model::AppData,
    pub warning: Option<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed_demo = args.iter().any(|a| a == "--seed-demo");

    let (store, data, warning) = match Store::new() {
        Ok(store) => {
            let (data, load_warning) = store.load();
            let warning = load_warning.or_else(|| store.note().map(str::to_string));
            let _ = store.ensure_daily_backup(&data);
            (store, data, warning)
        }
        Err(err) => {
            let fallback = std::env::temp_dir().join("CosturaTaller");
            let store = match Store::at(fallback) {
                Ok(store) => store,
                Err(fatal) => {
                    eprintln!("No se pudo iniciar el almacenamiento: {fatal}");
                    std::process::exit(1);
                }
            };
            (
                store,
                model::AppData::default(),
                Some(format!(
                    "No se pudo usar la carpeta de datos del usuario ({err}). Se usará una carpeta temporal."
                )),
            )
        }
    };

    if seed_demo {
        let demo = commands::demo_data();
        if let Err(err) = store.save(&demo) {
            eprintln!("No se pudieron guardar los datos de ejemplo: {err}");
            std::process::exit(1);
        }
        println!("Datos de ejemplo guardados en {}", store.data_file().display());
        return;
    }

    // WebView2 guarda su caché junto a los datos de la app. Así la carpeta es
    // siempre escribible y todo lo de Costura queda en un solo lugar.
    let webview_dir = store.dir().join("webview");
    let _ = std::fs::create_dir_all(&webview_dir);
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_dir);
    }

    tauri::Builder::default()
        .manage(Mutex::new(AppState {
            store,
            data,
            warning,
        }))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::dismiss_warning,
            commands::save_customer,
            commands::delete_customer,
            commands::save_order,
            commands::delete_order,
            commands::set_order_status,
            commands::add_order_note,
            commands::delete_order_note,
            commands::save_reminder,
            commands::toggle_reminder,
            commands::delete_reminder,
            commands::save_settings,
            commands::create_backup,
            commands::list_backups,
            commands::restore_backup,
            commands::open_folder,
            commands::reset_all,
            commands::load_demo,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Costura");
}
