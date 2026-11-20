use crate::model::{now_iso, AppData, DATA_VERSION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const APP_FOLDER: &str = "CosturaTaller";
const DATA_FILE: &str = "costura.json";
const BACKUP_PREFIX: &str = "costura-";
const MAX_BACKUPS: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub automatic: bool,
}

pub struct Store {
    dir: PathBuf,
    note: Option<String>,
}

impl Store {
    pub fn new() -> Result<Self, String> {
        if let Ok(custom) = std::env::var("COSTURA_DATA_DIR") {
            if !custom.trim().is_empty() {
                return Self::at(PathBuf::from(custom.trim()));
            }
        }

        let mut attempts: Vec<PathBuf> = Vec::new();
        if let Some(base) = dirs::data_dir() {
            attempts.push(base.join(APP_FOLDER));
        }
        if let Some(local_base) = dirs::data_local_dir() {
            let local_dir = local_base.join(APP_FOLDER);
            if !attempts.contains(&local_dir) {
                attempts.push(local_dir);
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                attempts.push(dir.join("datos"));
            }
        }
        if let Ok(cwd) = std::env::current_dir() {
            attempts.push(cwd.join("datos"));
        }

        let mut last_error = String::from("No se pudo determinar la carpeta de datos.");
        for (index, dir) in attempts.iter().enumerate() {
            match Self::at(dir.clone()) {
                Ok(mut store) => {
                    if index > 0 {
                        store.note = Some(format!(
                            "No se pudo usar la carpeta del usuario. Los datos se guardan en '{}'.",
                            dir.display()
                        ));
                    }
                    return Ok(store);
                }
                Err(err) => last_error = err,
            }
        }
        Err(last_error)
    }

    pub fn at(dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("No se pudo crear la carpeta de datos: {e}"))?;
        fs::create_dir_all(dir.join("backups"))
            .map_err(|e| format!("No se pudo crear la carpeta de respaldos: {e}"))?;
        Ok(Self { dir, note: None })
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn data_file(&self) -> PathBuf {
        self.dir.join(DATA_FILE)
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    /// Carpeta de respaldos configurada por el usuario, o la predeterminada.
    pub fn resolve_backups_dir(&self, data: &AppData) -> PathBuf {
        let configured = data.settings.backup_dir.trim();
        if !configured.is_empty() {
            let path = PathBuf::from(configured);
            if fs::create_dir_all(&path).is_ok() {
                return path;
            }
        }
        let default = self.backups_dir();
        let _ = fs::create_dir_all(&default);
        default
    }

    /// Carga los datos. Devuelve también un aviso cuando hubo que recuperar algo.
    pub fn load(&self) -> (AppData, Option<String>) {
        let path = self.data_file();
        let (mut data, warning) = if !path.exists() {
            (AppData::default(), None)
        } else {
            match read_data(&path) {
                Ok(data) => (data, None),
                Err(primary_error) => {
                    let bak = path.with_extension("json.bak");
                    if bak.exists() {
                        if let Ok(data) = read_data(&bak) {
                            let mut recovered = data;
                            self.hydrate_firebase_from_env(&mut recovered);
                            return (
                                recovered,
                                Some(format!(
                                    "El archivo principal estaba dañado ({primary_error}). Se recuperó la copia de seguridad automática."
                                )),
                            );
                        }
                    }

                    let quarantine = self
                        .dir
                        .join(format!("costura-danado-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                    let _ = fs::rename(&path, &quarantine);
                    (
                        AppData::default(),
                        Some(format!(
                            "El archivo de datos no se pudo leer ({primary_error}). Se guardó aparte como '{}' y la app inició vacía. Puedes restaurar un respaldo desde Ajustes.",
                            quarantine.file_name().and_then(|n| n.to_str()).unwrap_or("costura-danado.json")
                        )),
                    )
                }
            }
        };
        self.hydrate_firebase_from_env(&mut data);
        (data, warning)
    }

    /// Hidrata la configuración de Firebase en memoria desde el archivo `.env` local
    /// sin necesidad de hardcodear claves en el repositorio.
    pub fn hydrate_firebase_from_env(&self, data: &mut AppData) {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(".env"));
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join(".env"));
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join(".env"));
            }
        }
        candidates.push(self.dir.join(".env"));

        for env_path in candidates {
            if env_path.exists() {
                if let Ok(raw) = fs::read_to_string(&env_path) {
                    if data.firebase.apply_dotenv_str(&raw) {
                        break;
                    }
                }
            }
        }
    }

    pub fn save(&self, data: &AppData) -> Result<(), String> {
        let path = self.data_file();
        let tmp = self.dir.join("costura.json.tmp");

        let mut data = data.clone();
        data.version = DATA_VERSION;
        // Redacta api_key antes de escribir en disco para no dejar secretos en texto plano en costura.json
        data.firebase = data.firebase.redacted_for_disk();
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| format!("No se pudieron preparar los datos: {e}"))?;

        {
            let mut file = fs::File::create(&tmp)
                .map_err(|e| format!("No se pudo escribir el archivo temporal: {e}"))?;
            file.write_all(json.as_bytes())
                .map_err(|e| format!("No se pudo escribir el archivo temporal: {e}"))?;
            file.sync_all()
                .map_err(|e| format!("No se pudo sincronizar el archivo: {e}"))?;
        }

        if path.exists() {
            let bak = path.with_extension("json.bak");
            let _ = fs::copy(&path, &bak);
        }

        fs::rename(&tmp, &path).map_err(|e| format!("No se pudo guardar el archivo de datos: {e}"))?;
        Ok(())
    }

    pub fn create_backup(&self, data: &AppData, automatic: bool) -> Result<BackupInfo, String> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let file = if automatic {
            format!("{BACKUP_PREFIX}{stamp}-auto.json")
        } else {
            format!("{BACKUP_PREFIX}{stamp}.json")
        };
        let path = self.resolve_backups_dir(data).join(&file);

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| format!("No se pudo preparar el respaldo: {e}"))?;
        fs::write(&path, json).map_err(|e| format!("No se pudo escribir el respaldo: {e}"))?;

        self.prune_backups(data);

        Ok(BackupInfo {
            file,
            created_at: now_iso(),
            size_bytes: fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            automatic,
        })
    }

    pub fn ensure_daily_backup(&self, data: &AppData) -> Result<(), String> {
        if data.customers.is_empty() && data.orders.is_empty() && data.reminders.is_empty() {
            return Ok(());
        }
        let today = chrono::Local::now().format("%Y%m%d").to_string();
        let already = self
            .list_backups(data)
            .iter()
            .any(|b| b.automatic && b.file.contains(&today));
        if !already {
            self.create_backup(data, true)?;
        }
        Ok(())
    }

    pub fn list_backups(&self, data: &AppData) -> Vec<BackupInfo> {
        let mut items = Vec::new();
        let Ok(entries) = fs::read_dir(self.resolve_backups_dir(data)) else {
            return items;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(file) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
                continue;
            };
            let meta = entry.metadata().ok();
            let created_at = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
                })
                .unwrap_or_default();
            items.push(BackupInfo {
                automatic: file.contains("-auto"),
                file,
                created_at,
                size_bytes: meta.map(|m| m.len()).unwrap_or(0),
            });
        }
        items.sort_by(|a, b| b.file.cmp(&a.file));
        items
    }

    pub fn read_backup(&self, data: &AppData, file: &str) -> Result<AppData, String> {
        let safe = sanitize_backup_name(file)?;
        let path = self.resolve_backups_dir(data).join(&safe);
        if !path.exists() {
            return Err("Ese respaldo ya no existe.".into());
        }
        read_data(&path)
    }

    fn prune_backups(&self, data: &AppData) {
        let mut items = self.list_backups(data);
        if items.len() <= MAX_BACKUPS {
            return;
        }
        // Conserva los más recientes; los nombres ordenan por fecha.
        items.sort_by(|a, b| b.file.cmp(&a.file));
        let dir = self.resolve_backups_dir(data);
        for old in items.into_iter().skip(MAX_BACKUPS) {
            let _ = fs::remove_file(dir.join(old.file));
        }
    }
}

fn read_data(path: &Path) -> Result<AppData, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("no se pudo leer: {e}"))?;
    let mut data: AppData =
        serde_json::from_str(&raw).map_err(|e| format!("el formato no es válido: {e}"))?;
    if data.version > DATA_VERSION {
        return Err(format!(
            "el archivo fue creado por una versión más nueva (v{}).",
            data.version
        ));
    }
    data.version = DATA_VERSION;
    data.refresh_customer_aggregates();
    Ok(data)
}

fn sanitize_backup_name(file: &str) -> Result<String, String> {
    let name = file.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || !name.ends_with(".json")
    {
        return Err("Nombre de respaldo inválido.".into());
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Customer, Order, Reminder};

    fn temp_store() -> Store {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(uuid::Uuid::new_v4().to_string());
        Store::at(dir).unwrap()
    }

    fn sample() -> AppData {
        let customer = Customer {
            id: "c1".into(),
            full_name: "Ana Pérez".into(),
            created_at: now_iso(),
            ..Default::default()
        };
        let order = Order {
            id: "o1".into(),
            customer_id: customer.id.clone(),
            piece_name: "Blusa".into(),
            due_date: "2026-12-01".into(),
            created_at: now_iso(),
            ..Default::default()
        };
        let reminder = Reminder {
            id: "r1".into(),
            title: "Prueba".into(),
            due_date: "2026-12-01".into(),
            due_time: Some("10:00".into()),
            created_at: now_iso(),
            ..Default::default()
        };
        AppData {
            customers: vec![customer],
            orders: vec![order],
            reminders: vec![reminder],
            ..Default::default()
        }
    }

    #[test]
    fn guarda_y_recupera_datos() {
        let store = temp_store();
        let data = sample();
        store.save(&data).unwrap();
        let (loaded, warning) = store.load();
        assert!(warning.is_none());
        assert_eq!(loaded.customers.len(), 1);
        assert_eq!(loaded.orders[0].piece_name, "Blusa");
        assert_eq!(loaded.reminders.len(), 1);
    }

    #[test]
    fn respaldo_se_crea_lista_y_restaura() {
        let store = temp_store();
        let data = sample();
        store.save(&data).unwrap();
        let info = store.create_backup(&data, false).unwrap();
        assert!(info.file.ends_with(".json"));
        let list = store.list_backups(&data);
        assert_eq!(list.len(), 1);
        let restored = store.read_backup(&data, &list[0].file).unwrap();
        assert_eq!(restored.customers.len(), 1);
    }

    #[test]
    fn rechaza_nombres_de_respaldo_peligrosos() {
        let store = temp_store();
        let data = sample();
        assert!(store.read_backup(&data, "..\\costura.json").is_err());
        assert!(store.read_backup(&data, "C:\\Windows\\win.ini").is_err());
        assert!(store.read_backup(&data, "no-existe.json").is_err());
    }

    #[test]
    fn recupera_desde_bak_si_el_principal_esta_danado() {
        let store = temp_store();
        let data = sample();
        store.save(&data).unwrap();
        store.save(&data).unwrap(); // genera .bak
        fs::write(store.data_file(), "{ esto no es json").unwrap();
        let (loaded, warning) = store.load();
        assert!(warning.is_some());
        assert_eq!(loaded.customers.len(), 1);
    }

    #[test]
    fn respaldo_diario_solo_una_vez() {
        let store = temp_store();
        let data = sample();
        store.ensure_daily_backup(&data).unwrap();
        store.ensure_daily_backup(&data).unwrap();
        let autos = store
            .list_backups(&data)
            .into_iter()
            .filter(|b| b.automatic)
            .count();
        assert_eq!(autos, 1);
    }

    #[test]
    fn respeta_carpeta_de_respaldos_configurada() {
        let store = temp_store();
        let mut data = sample();
        let custom = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("backups-{}", uuid::Uuid::new_v4()));
        data.settings.backup_dir = custom.to_string_lossy().to_string();
        let info = store.create_backup(&data, false).unwrap();
        assert!(custom.join(&info.file).exists());
        assert_eq!(store.list_backups(&data).len(), 1);
        let _ = fs::remove_dir_all(custom);
    }

    #[test]
    fn hidrata_firebase_desde_env_y_redacta_api_key_en_costura_json() {
        let store = temp_store();
        let mut data = sample();
        data.firebase.api_key = "SECRET_IN_MEMORY_KEY".into();
        data.firebase.project_id = "tallercostura-9f69d".into();
        store.save(&data).unwrap();

        // Verifica que en el archivo físico costura.json NO se guarde la api_key en texto plano
        let raw_on_disk = fs::read_to_string(store.data_file()).unwrap();
        assert!(!raw_on_disk.contains("SECRET_IN_MEMORY_KEY"));
        assert!(raw_on_disk.contains("REEMPLAZAR_CON_FIREBASE_API_KEY"));
    }
}
