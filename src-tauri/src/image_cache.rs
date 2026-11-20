// ─────────────────────────────────────────────────────────────────────────────
// Costura v2.0 — Gestor de Caché Local de Imágenes en Disco con Expulsión LRU
// ─────────────────────────────────────────────────────────────────────────────
// - Límite estricto: 150 MB (157,286,400 bytes).
// - Purga LRU automática al 85 % (127.5 MB = 133,693,440 bytes) cuando se supera el límite.
// - Protección estricta: Jamás expulsa archivos marcados con `pending_upload == true`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_CACHE_BYTES: u64 = 150 * 1024 * 1024; // 150 MB
pub const TARGET_PURGE_BYTES: u64 = (MAX_CACHE_BYTES * 85) / 100; // 127.5 MB (85 %)
const CACHE_SUBDIR: &str = "cache/order_images";
const MANIFEST_FILE: &str = "lru_manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CachedImageEntry {
    pub key: String,
    pub file_name: String,
    pub order_id: String,
    pub image_id: String,
    pub size_bytes: u64,
    pub last_accessed_ms: u64,
    pub pending_upload: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CacheManifest {
    entries: BTreeMap<String, CachedImageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageCacheStats {
    pub total_bytes: u64,
    pub max_bytes: u64,
    pub target_purge_bytes: u64,
    pub total_files: usize,
    pub pending_upload_files: usize,
    pub pending_upload_bytes: u64,
    pub evicted_last_run: usize,
}

pub struct ImageDiskCache {
    root_dir: PathBuf,
    max_bytes: u64,
    target_purge_bytes: u64,
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sanitize_token(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "item".to_string()
    } else {
        cleaned
    }
}

impl ImageDiskCache {
    pub fn new(base_data_dir: &Path) -> Result<Self, String> {
        Self::with_limits(base_data_dir, MAX_CACHE_BYTES, TARGET_PURGE_BYTES)
    }

    pub fn with_limits(
        base_data_dir: &Path,
        max_bytes: u64,
        target_purge_bytes: u64,
    ) -> Result<Self, String> {
        let root_dir = base_data_dir.join(CACHE_SUBDIR);
        fs::create_dir_all(&root_dir)
            .map_err(|e| format!("No se pudo crear la carpeta de caché de imágenes: {e}"))?;
        Ok(Self {
            root_dir,
            max_bytes,
            target_purge_bytes,
        })
    }

    fn manifest_path(&self) -> PathBuf {
        self.root_dir.join(MANIFEST_FILE)
    }

    fn load_manifest(&self) -> CacheManifest {
        let path = self.manifest_path();
        if !path.exists() {
            return CacheManifest::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save_manifest(&self, manifest: &CacheManifest) -> Result<(), String> {
        let json = serde_json::to_string_pretty(manifest)
            .map_err(|e| format!("No se pudo serializar el manifiesto LRU: {e}"))?;
        fs::write(self.manifest_path(), json)
            .map_err(|e| format!("No se pudo guardar el manifiesto LRU: {e}"))?;
        Ok(())
    }

    pub fn entry_key(order_id: &str, image_id: &str) -> String {
        format!("{}__{}", sanitize_token(order_id), sanitize_token(image_id))
    }

    /// Guarda o actualiza una imagen en el caché local y ejecuta la purga LRU al 85 %
    /// si el tamaño total supera `max_bytes` (150 MB), protegiendo siempre `pending_upload == true`.
    pub fn put_image(
        &self,
        order_id: &str,
        image_id: &str,
        bytes: &[u8],
        pending_upload: bool,
        timestamp_override_ms: Option<u64>,
    ) -> Result<(CachedImageEntry, ImageCacheStats), String> {
        let key = Self::entry_key(order_id, image_id);
        let file_name = format!("{key}.webp");
        let file_path = self.root_dir.join(&file_name);

        fs::write(&file_path, bytes)
            .map_err(|e| format!("No se pudo escribir la imagen en caché: {e}"))?;

        let mut manifest = self.load_manifest();
        let accessed = timestamp_override_ms.unwrap_or_else(now_epoch_ms);
        let entry = CachedImageEntry {
            key: key.clone(),
            file_name,
            order_id: order_id.trim().to_string(),
            image_id: image_id.trim().to_string(),
            size_bytes: bytes.len() as u64,
            last_accessed_ms: accessed,
            pending_upload,
        };
        manifest.entries.insert(key, entry.clone());

        let evicted = self.enforce_lru_quota(&mut manifest);
        self.save_manifest(&manifest)?;
        let stats = self.compute_stats(&manifest, evicted);
        Ok((entry, stats))
    }

    /// Marca una imagen como lectura reciente (actualiza `last_accessed_ms` para LRU).
    pub fn touch_image(
        &self,
        order_id: &str,
        image_id: &str,
        timestamp_override_ms: Option<u64>,
    ) -> Result<Option<PathBuf>, String> {
        let key = Self::entry_key(order_id, image_id);
        let mut manifest = self.load_manifest();
        let Some(entry) = manifest.entries.get_mut(&key) else {
            return Ok(None);
        };
        let path = self.root_dir.join(&entry.file_name);
        if !path.exists() {
            manifest.entries.remove(&key);
            let _ = self.save_manifest(&manifest);
            return Ok(None);
        }
        entry.last_accessed_ms = timestamp_override_ms.unwrap_or_else(now_epoch_ms);
        self.save_manifest(&manifest)?;
        Ok(Some(path))
    }

    /// Marca una imagen en caché como ya subida a Firebase Storage (`pending_upload = false`),
    /// haciéndola elegible para futuras purgas LRU cuando se llene el disco.
    pub fn mark_uploaded(&self, order_id: &str, image_id: &str) -> Result<bool, String> {
        let key = Self::entry_key(order_id, image_id);
        let mut manifest = self.load_manifest();
        if let Some(entry) = manifest.entries.get_mut(&key) {
            entry.pending_upload = false;
            entry.last_accessed_ms = now_epoch_ms();
            let _ = self.enforce_lru_quota(&mut manifest);
            self.save_manifest(&manifest)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn stats(&self) -> ImageCacheStats {
        let manifest = self.load_manifest();
        self.compute_stats(&manifest, 0)
    }

    pub fn purge_if_needed(&self) -> Result<ImageCacheStats, String> {
        let mut manifest = self.load_manifest();
        let evicted = self.enforce_lru_quota(&mut manifest);
        self.save_manifest(&manifest)?;
        Ok(self.compute_stats(&manifest, evicted))
    }

    fn compute_stats(&self, manifest: &CacheManifest, evicted_last_run: usize) -> ImageCacheStats {
        let mut total_bytes = 0u64;
        let mut pending_upload_files = 0usize;
        let mut pending_upload_bytes = 0u64;

        for e in manifest.entries.values() {
            total_bytes = total_bytes.saturating_add(e.size_bytes);
            if e.pending_upload {
                pending_upload_files += 1;
                pending_upload_bytes = pending_upload_bytes.saturating_add(e.size_bytes);
            }
        }

        ImageCacheStats {
            total_bytes,
            max_bytes: self.max_bytes,
            target_purge_bytes: self.target_purge_bytes,
            total_files: manifest.entries.len(),
            pending_upload_files,
            pending_upload_bytes,
            evicted_last_run,
        }
    }

    /// Si el tamaño total excede `self.max_bytes` (150 MB), elimina los archivos más antiguos
    /// según `last_accessed_ms` que NO estén pendientes de subida (`pending_upload == false`)
    /// hasta que `total_bytes <= self.target_purge_bytes` (127.5 MB / 85 %).
    fn enforce_lru_quota(&self, manifest: &mut CacheManifest) -> usize {
        // Primero limpiamos entradas huérfanas cuyo archivo ya no exista en disco
        let orphan_keys: Vec<String> = manifest
            .entries
            .iter()
            .filter(|(_, e)| !self.root_dir.join(&e.file_name).exists())
            .map(|(k, _)| k.clone())
            .collect();
        for k in orphan_keys {
            manifest.entries.remove(&k);
        }

        let mut total_bytes: u64 = manifest.entries.values().map(|e| e.size_bytes).sum();
        if total_bytes <= self.max_bytes {
            return 0;
        }

        // Candidatos a expulsión: SOLO aquellos con `pending_upload == false`
        let mut candidates: Vec<CachedImageEntry> = manifest
            .entries
            .values()
            .filter(|e| !e.pending_upload)
            .cloned()
            .collect();

        // Orden LRU estricto: menor `last_accessed_ms` primero (más antiguo primero)
        candidates.sort_by(|a, b| {
            a.last_accessed_ms
                .cmp(&b.last_accessed_ms)
                .then_with(|| a.key.cmp(&b.key))
        });

        let mut evicted = 0usize;
        for candidate in candidates {
            if total_bytes <= self.target_purge_bytes {
                break;
            }
            let file_path = self.root_dir.join(&candidate.file_name);
            let _ = fs::remove_file(&file_path);
            manifest.entries.remove(&candidate.key);
            total_bytes = total_bytes.saturating_sub(candidate.size_bytes);
            evicted += 1;
        }

        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("img-cache-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn constantes_de_150mb_y_purga_85_por_ciento_son_exactas() {
        assert_eq!(MAX_CACHE_BYTES, 157_286_400); // 150 * 1024 * 1024
        assert_eq!(TARGET_PURGE_BYTES, 133_693_440); // 127.5 * 1024 * 1024 (85 %)
    }

    #[test]
    fn purga_lru_al_85_por_ciento_y_protege_imagenes_pendientes_de_subida() {
        let base = temp_cache_dir();
        // Simulamos un límite a escala proporcional: max = 1000 bytes, target 85% = 850 bytes
        let cache = ImageDiskCache::with_limits(&base, 1000, 850).unwrap();

        // 1. Imagen muy antigua (t=100) pero PENDIENTE DE SUBIDA (300 bytes) -> NUNCA debe borrarse
        cache
            .put_image("ord_1", "img_pending", &vec![1u8; 300], true, Some(100))
            .unwrap();

        // 2. Imagen antigua ya sincronizada (t=200, 300 bytes) -> Candidata #1 a expulsión LRU
        cache
            .put_image("ord_1", "img_old_synced", &vec![2u8; 300], false, Some(200))
            .unwrap();

        // 3. Imagen intermedia ya sincronizada (t=300, 300 bytes) -> Candidata #2
        cache
            .put_image("ord_2", "img_mid_synced", &vec![3u8; 300], false, Some(300))
            .unwrap();

        // Hasta aquí: 900 bytes <= 1000 bytes (sin purga aún)
        assert_eq!(cache.stats().total_bytes, 900);
        assert_eq!(cache.stats().total_files, 3);

        // Tocamos `img_old_synced` en t=400 para que deje de ser la más antigua
        cache
            .touch_image("ord_1", "img_old_synced", Some(400))
            .unwrap();

        // 4. Insertamos nueva imagen sincronizada (t=500, 200 bytes) -> Total = 1100 bytes (> 1000)
        // Debe purgar hasta bajar a <= 850 bytes.
        // Como `img_pending` (t=100) está protegida (`pending_upload = true`),
        // y `img_old_synced` fue tocada en t=400, la más antigua elegible es `img_mid_synced` (t=300, 300 bytes).
        // 1100 - 300 = 800 bytes (<= 850 bytes).
        let (_, stats) = cache
            .put_image("ord_3", "img_new_synced", &vec![4u8; 200], false, Some(500))
            .unwrap();

        assert_eq!(stats.evicted_last_run, 1);
        assert_eq!(stats.total_bytes, 800);
        assert!(stats.total_bytes <= 850);
        assert_eq!(stats.pending_upload_files, 1);

        // Verificamos en disco qué archivos quedaron
        let manifest = cache.load_manifest();
        assert!(
            manifest
                .entries
                .contains_key(&ImageDiskCache::entry_key("ord_1", "img_pending")),
            "La imagen pendiente de subida jamás debe ser eliminada por LRU"
        );
        assert!(
            !manifest
                .entries
                .contains_key(&ImageDiskCache::entry_key("ord_2", "img_mid_synced")),
            "La imagen sincronizada con acceso más antiguo debía ser expulsada"
        );
        assert!(
            manifest
                .entries
                .contains_key(&ImageDiskCache::entry_key("ord_1", "img_old_synced")),
            "La imagen tocada recientemente debe conservarse"
        );

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn desmarca_pending_upload_al_confirmar_subida_a_storage() {
        let base = temp_cache_dir();
        let cache = ImageDiskCache::with_limits(&base, 500, 425).unwrap();

        cache
            .put_image("ord_9", "img_1", &vec![9u8; 300], true, Some(10))
            .unwrap();
        assert_eq!(cache.stats().pending_upload_files, 1);

        let updated = cache.mark_uploaded("ord_9", "img_1").unwrap();
        assert!(updated);
        assert_eq!(cache.stats().pending_upload_files, 0);

        let _ = fs::remove_dir_all(base);
    }
}
