use chrono::{NaiveDate, NaiveTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DATA_VERSION: u32 = 2;

pub const GARMENT_TYPES: [&str; 5] = ["camisa", "pantalon", "vestido", "bolso", "otro"];
pub const SUPPLY_CATEGORIES: [&str; 7] = [
    "tela",
    "hilo",
    "boton",
    "cierre",
    "elastico",
    "entretela",
    "otro",
];
pub const SUPPLY_UNITS: [&str; 7] = ["yarda", "metro", "unidad", "cono", "rollo", "par", "otro"];
pub const CUT_DIRECTIONS: [&str; 4] = ["", "al_hilo", "al_bies", "contrahilo"];
pub const ORDER_STATUSES: [&str; 6] = [
    "pendiente",
    "en_corte",
    "en_confeccion",
    "prueba_ajuste",
    "terminado",
    "entregado",
];
pub const PRIORITIES: [&str; 3] = ["normal", "urgente", "prioritaria"];

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Genera un ID determinista para movimientos de kárdex vinculados a un pedido y su revisión.
/// Evita descuentos duplicados ante reintentos offline (`WriteBatch` idempotente).
pub fn deterministic_movement_id(order_id: &str, revision: u64, supply_id: &str) -> String {
    format!("mov_{order_id}_r{revision}_{supply_id}")
}

// ────────────────────────────── Ajustes (/talleres/{uid}) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub workshop_name: String,
    pub owner_name: String,
    pub currency: String,
    pub tax_rate_percent: f64,
    pub tax_label: String,
    pub default_due_days: i64,
    pub backup_dir: String,
    pub language: String,
    pub updated_at: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            workshop_name: "Mi taller de costura".to_string(),
            owner_name: String::new(),
            currency: "$".to_string(),
            tax_rate_percent: 0.0,
            tax_label: "IVA".to_string(),
            default_due_days: 7,
            backup_dir: String::new(),
            language: "es".to_string(),
            updated_at: now_iso(),
        }
    }
}

// ────────────────────────────── Configuración Firebase (Huecos de Conexión) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FirebaseConnectionSlots {
    pub mode: String, // "offline_local", "emulator", "cloud"
    pub use_emulators: bool,
    pub project_id: String,
    pub api_key: String,
    pub auth_domain: String,
    pub storage_bucket: String,
    pub messaging_sender_id: String,
    pub app_id: String,
    pub google_web_client_id: String,
    pub active_uid: Option<String>,
    pub emulator_auth_host: String,
    pub emulator_firestore_host: String,
    pub emulator_storage_host: String,
}

impl Default for FirebaseConnectionSlots {
    fn default() -> Self {
        Self {
            mode: "offline_local".to_string(),
            use_emulators: true,
            project_id: "REEMPLAZAR_PROJECT_ID".to_string(),
            api_key: "REEMPLAZAR_CON_FIREBASE_API_KEY".to_string(),
            auth_domain: "REEMPLAZAR_PROJECT_ID.firebaseapp.com".to_string(),
            storage_bucket: "REEMPLAZAR_PROJECT_ID.firebasestorage.app".to_string(),
            messaging_sender_id: "REEMPLAZAR_MESSAGING_SENDER_ID".to_string(),
            app_id: "REEMPLAZAR_WEB_APP_ID".to_string(),
            google_web_client_id: "REEMPLAZAR_GOOGLE_OAUTH_WEB_CLIENT_ID.apps.googleusercontent.com"
                .to_string(),
            active_uid: None,
            emulator_auth_host: "http://127.0.0.1:9099".to_string(),
            emulator_firestore_host: "127.0.0.1:8080".to_string(),
            emulator_storage_host: "127.0.0.1:9199".to_string(),
        }
    }
}

impl FirebaseConnectionSlots {
    pub fn is_cloud_configured(&self) -> bool {
        !self.project_id.trim().is_empty()
            && !self.project_id.starts_with("REEMPLAZAR_")
            && !self.api_key.trim().is_empty()
            && !self.api_key.starts_with("REEMPLAZAR_")
    }

    /// Parsea un contenido de archivo `.env` (formato `KEY="VALUE"` o `KEY=VALUE`)
    /// sin exponer ni hardcodear secretos en el código fuente.
    pub fn apply_dotenv_str(&mut self, raw: &str) -> bool {
        let mut applied_any = false;
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((k, v)) = trimmed.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let mut val = v.trim();
            if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
                || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
            {
                val = &val[1..val.len() - 1];
            }
            let val = val.trim();
            if val.is_empty() || val.starts_with("REEMPLAZAR_") {
                continue;
            }
            match key {
                "FIREBASE_API_KEY" | "COSTURA_FIREBASE_API_KEY" => {
                    self.api_key = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_AUTH_DOMAIN" | "COSTURA_FIREBASE_AUTH_DOMAIN" => {
                    self.auth_domain = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_PROJECT_ID" | "COSTURA_FIREBASE_PROJECT_ID" => {
                    self.project_id = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_STORAGE_BUCKET" | "COSTURA_FIREBASE_STORAGE_BUCKET" => {
                    self.storage_bucket = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_MESSAGING_SENDER_ID" | "COSTURA_FIREBASE_MESSAGING_SENDER_ID" => {
                    self.messaging_sender_id = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_APP_ID" | "COSTURA_FIREBASE_APP_ID" => {
                    self.app_id = val.to_string();
                    applied_any = true;
                }
                "FIREBASE_GOOGLE_WEB_CLIENT_ID" | "COSTURA_GOOGLE_WEB_CLIENT_ID" => {
                    self.google_web_client_id = val.to_string();
                    applied_any = true;
                }
                "COSTURA_FIREBASE_MODE" | "FIREBASE_MODE" => {
                    self.mode = val.to_string();
                }
                "COSTURA_FIREBASE_USE_EMULATORS" | "FIREBASE_USE_EMULATORS" => {
                    self.use_emulators = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes");
                }
                "COSTURA_EMULATOR_AUTH_URL" => self.emulator_auth_host = val.to_string(),
                "COSTURA_EMULATOR_FIRESTORE_HOST" => self.emulator_firestore_host = val.to_string(),
                "COSTURA_EMULATOR_STORAGE_HOST" => self.emulator_storage_host = val.to_string(),
                _ => {}
            }
        }
        if applied_any && self.is_cloud_configured() && self.mode == "offline_local" {
            self.mode = "cloud".to_string();
            self.use_emulators = false;
        }
        applied_any
    }

    /// Devuelve una copia sin secretos (`api_key` vacía) para persistir en `costura.json`
    /// cuando las credenciales provienen de `.env`.
    pub fn redacted_for_disk(&self) -> Self {
        let mut copy = self.clone();
        if !copy.api_key.starts_with("REEMPLAZAR_") {
            copy.api_key = "REEMPLAZAR_CON_FIREBASE_API_KEY".to_string();
        }
        copy
    }
}

// ────────────────────────────── Datos Globales ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppData {
    pub version: u32,
    pub settings: Settings,
    pub firebase: FirebaseConnectionSlots,
    pub customers: Vec<Customer>,
    pub orders: Vec<Order>,
    pub reminders: Vec<Reminder>,
    pub supplies: Vec<Supply>,
    pub movements: Vec<SupplyMovement>,
    pub audit_log: Vec<AuditEvent>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            settings: Settings::default(),
            firebase: FirebaseConnectionSlots::default(),
            customers: Vec::new(),
            orders: Vec::new(),
            reminders: Vec::new(),
            supplies: Vec::new(),
            movements: Vec::new(),
            audit_log: Vec::new(),
        }
    }
}

// ────────────────────────────── Colección: clientes ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Customer {
    pub id: String,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
    pub pending_balance: f64,
    pub active_orders_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for Customer {
    fn default() -> Self {
        Self {
            id: String::new(),
            full_name: String::new(),
            phone: String::new(),
            email: String::new(),
            address: String::new(),
            notes: String::new(),
            pending_balance: 0.0,
            active_orders_count: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

// ────────────────────────────── Colección: pedidos ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Measurements {
    pub kind: String,
    pub values: BTreeMap<String, f64>,
    pub options: BTreeMap<String, String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OrderNote {
    pub id: String,
    pub timestamp: String,
    pub stage: String,
    pub content: String,
}

/// Metadatos de imagen en Firebase Storage (/talleres/{uid}/pedidos/{orderId}/{imageId}.webp)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OrderImageMeta {
    pub id: String,
    pub storage_path: String,
    pub download_url: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Order {
    pub id: String,
    pub customer_id: String,
    pub piece_name: String,
    pub garment_type: String,
    pub custom_type: String,
    pub style_name: String,
    pub status: String,
    pub priority: String,
    pub measurements: Measurements,
    pub fabric_details: String,
    pub estimated_cost: f64,
    pub deposit_paid: f64,
    pub due_date: String,
    pub delivery_date: Option<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub notes: Vec<OrderNote>,
    pub supplies: Vec<SupplyUsage>,
    pub images: Vec<String>,
    pub storage_images: Vec<OrderImageMeta>,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            id: String::new(),
            customer_id: String::new(),
            piece_name: String::new(),
            garment_type: "camisa".to_string(),
            custom_type: String::new(),
            style_name: String::new(),
            status: "pendiente".to_string(),
            priority: "normal".to_string(),
            measurements: Measurements::default(),
            fabric_details: String::new(),
            estimated_cost: 0.0,
            deposit_paid: 0.0,
            due_date: String::new(),
            delivery_date: None,
            revision: 1,
            created_at: String::new(),
            updated_at: String::new(),
            notes: Vec::new(),
            supplies: Vec::new(),
            images: Vec::new(),
            storage_images: Vec::new(),
        }
    }
}

impl Order {
    pub fn balance(&self) -> f64 {
        round_money((self.estimated_cost - self.deposit_paid).max(0.0))
    }

    pub fn is_active(&self) -> bool {
        self.status != "entregado"
    }
}

// ────────────────────────────── Colección: agenda ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub description: String,
    pub due_date: String,
    pub due_time: Option<String>,
    pub priority: String,
    pub completed: bool,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for Reminder {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            due_date: String::new(),
            due_time: None,
            priority: "normal".to_string(),
            completed: false,
            customer_id: None,
            order_id: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

// ────────────────────────────── Colecciones: insumos y movimientos ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Supply {
    pub id: String,
    pub name: String,
    pub category: String,
    pub unit: String,
    pub stock: f64,
    pub min_stock: f64,
    pub unit_cost: f64,
    pub supplier: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
}

impl Default for Supply {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            category: "tela".to_string(),
            unit: "yarda".to_string(),
            stock: 0.0,
            min_stock: 0.0,
            unit_cost: 0.0,
            supplier: String::new(),
            notes: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

impl Supply {
    pub fn is_low(&self) -> bool {
        self.stock <= self.min_stock
    }

    pub fn is_out(&self) -> bool {
        self.stock <= 0.0
    }

    pub fn value(&self) -> f64 {
        (self.stock.max(0.0)) * self.unit_cost
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SupplyUsage {
    pub supply_id: String,
    pub quantity: f64,
    pub cut: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SupplyMovement {
    pub id: String,
    pub supply_id: String,
    pub timestamp: String,
    pub delta: f64,
    pub kind: String,
    pub reason: String,
    pub order_id: Option<String>,
    pub order_revision: Option<u64>,
}

impl Default for SupplyMovement {
    fn default() -> Self {
        Self {
            id: String::new(),
            supply_id: String::new(),
            timestamp: String::new(),
            delta: 0.0,
            kind: "ajuste".to_string(),
            reason: String::new(),
            order_id: None,
            order_revision: None,
        }
    }
}

// ────────────────────────────── Colección: auditoria (ISO/IEC 27002 8.15) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: String,
    pub action: String,      // ej. "customer.delete", "order.delete", "supply.adjust", "account.reset"
    pub entity_type: String, // "clientes", "pedidos", "insumos", "talleres"
    pub entity_id: String,
    pub summary: String,
}

// ────────────────────────────── Payloads Firestore v2 (/talleres/{uid}/...) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirestoreWorkshopDoc {
    pub version: u32,
    pub workshop_name: String,
    pub owner_name: String,
    pub currency: String,
    pub tax_rate_percent: f64,
    pub tax_label: String,
    pub default_due_days: i64,
    pub language: String,
    pub updated_at: String,
}

impl From<&Settings> for FirestoreWorkshopDoc {
    fn from(s: &Settings) -> Self {
        Self {
            version: DATA_VERSION,
            workshop_name: if s.workshop_name.trim().is_empty() {
                "Mi taller de costura".to_string()
            } else {
                s.workshop_name.trim().to_string()
            },
            owner_name: s.owner_name.trim().to_string(),
            currency: if s.currency.trim().is_empty() {
                "$".to_string()
            } else {
                s.currency.trim().to_string()
            },
            tax_rate_percent: s.tax_rate_percent.clamp(0.0, 100.0),
            tax_label: s.tax_label.trim().to_string(),
            default_due_days: s.default_due_days.clamp(1, 365),
            language: match s.language.trim() {
                "en" | "pt" => s.language.trim().to_string(),
                _ => "es".to_string(),
            },
            updated_at: if s.updated_at.trim().is_empty() {
                now_iso()
            } else {
                s.updated_at.clone()
            },
        }
    }
}

/// Estructura exacta de lotes (`WriteBatch`) preparada para sincronizar con Firestore
/// cuando el usuario conecte su `projectId`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirestoreWorkshopSnapshot {
    pub root_path: String,
    pub workshop_doc: FirestoreWorkshopDoc,
    pub collections: FirestoreCollectionsPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirestoreCollectionsPayload {
    pub clientes: Vec<Customer>,
    pub pedidos: Vec<Order>,
    pub insumos: Vec<Supply>,
    pub movimientos: Vec<SupplyMovement>,
    pub agenda: Vec<Reminder>,
    pub auditoria: Vec<AuditEvent>,
}

impl AppData {
    /// Recalcula los saldos por cobrar (`pending_balance`) y el conteo de pedidos activos (`active_orders_count`)
    /// para todos los clientes (equivalente local al `FieldValue.increment` conmutativo en Firestore).
    pub fn refresh_customer_aggregates(&mut self) {
        let mut balances: BTreeMap<String, (f64, i64)> = BTreeMap::new();
        for order in &self.orders {
            let entry = balances.entry(order.customer_id.clone()).or_insert((0.0, 0));
            if order.is_active() {
                entry.0 = round_money(entry.0 + order.balance());
                entry.1 += 1;
            }
        }
        for customer in &mut self.customers {
            if let Some((bal, count)) = balances.get(&customer.id) {
                customer.pending_balance = *bal;
                customer.active_orders_count = *count;
            } else {
                customer.pending_balance = 0.0;
                customer.active_orders_count = 0;
            }
            if customer.updated_at.is_empty() {
                customer.updated_at = customer.created_at.clone();
            }
        }

        // Normaliza cualquier recordatorio heredado que tuviera fecha+hora combinada en due_date
        for reminder in &mut self.reminders {
            if let Some((d, t)) = reminder.due_date.clone().split_once('T') {
                reminder.due_date = d.trim().to_string();
                if reminder.due_time.is_none() {
                    let hhmm = t.trim();
                    if hhmm.len() >= 5 && parse_time(&hhmm[..5]).is_some() {
                        reminder.due_time = Some(hhmm[..5].to_string());
                    }
                }
            }
            if reminder.updated_at.is_empty() {
                reminder.updated_at = reminder.created_at.clone();
            }
        }
    }

    /// Exporta el estado actual al formato jerárquico de colecciones de Cloud Firestore `/talleres/{uid}/...`
    pub fn to_firestore_snapshot(&self, uid: &str) -> FirestoreWorkshopSnapshot {
        let safe_uid = if uid.trim().is_empty() {
            "PENDIENTE_UID_FIREBASE"
        } else {
            uid.trim()
        };
        FirestoreWorkshopSnapshot {
            root_path: format!("/talleres/{safe_uid}"),
            workshop_doc: FirestoreWorkshopDoc::from(&self.settings),
            collections: FirestoreCollectionsPayload {
                clientes: self.customers.clone(),
                pedidos: self.orders.clone(),
                insumos: self.supplies.clone(),
                movimientos: self.movements.clone(),
                agenda: self.reminders.clone(),
                auditoria: self.audit_log.clone(),
            },
        }
    }

    /// Genera un payload de materialización completa que garantiza al menos 1 documento válido
    /// en el documento raíz `/talleres/{uid}` y en cada una de las 6 subcolecciones
    /// (`clientes`, `pedidos`, `insumos`, `movimientos`, `agenda`, `auditoria`) para crear y verificar
    /// todas las tablas en Cloud Firestore bajo `firebase/firestore.rules`.
    pub fn to_materialized_firestore_snapshot(&self, uid: &str) -> FirestoreWorkshopSnapshot {
        let mut snap = self.to_firestore_snapshot(uid);
        let now = now_iso();
        let today_str = today();

        if snap.collections.clientes.is_empty() {
            snap.collections.clientes.push(Customer {
                id: "cli_inicial_taller".to_string(),
                full_name: "Cliente General de Taller".to_string(),
                phone: String::new(),
                email: String::new(),
                address: String::new(),
                notes: "Registro inicial para materializar la tabla clientes en Cloud Firestore.".to_string(),
                pending_balance: 0.0,
                active_orders_count: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        if snap.collections.insumos.is_empty() {
            snap.collections.insumos.push(Supply {
                id: "ins_inicial_taller".to_string(),
                name: "Hilo poliéster base".to_string(),
                category: "hilo".to_string(),
                unit: "cono".to_string(),
                stock: 1.0,
                min_stock: 1.0,
                unit_cost: 0.0,
                supplier: "Inventario inicial".to_string(),
                notes: "Insumo base para materializar la tabla insumos.".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        let first_customer_id = snap.collections.clientes[0].id.clone();
        let first_supply_id = snap.collections.insumos[0].id.clone();

        if snap.collections.pedidos.is_empty() {
            let mut values = BTreeMap::new();
            values.insert("largo".to_string(), 20.0);
            values.insert("ancho".to_string(), 15.0);
            snap.collections.pedidos.push(Order {
                id: "ped_inicial_taller".to_string(),
                customer_id: first_customer_id.clone(),
                piece_name: "Proyecto base de verificación".to_string(),
                garment_type: "otro".to_string(),
                custom_type: "Muestra de taller".to_string(),
                style_name: "A medida".to_string(),
                status: "entregado".to_string(),
                priority: "normal".to_string(),
                measurements: Measurements {
                    kind: "otro".to_string(),
                    values,
                    options: BTreeMap::new(),
                    notes: "Medidas en cm".to_string(),
                },
                fabric_details: "Registro base para materializar la tabla pedidos.".to_string(),
                estimated_cost: 0.0,
                deposit_paid: 0.0,
                due_date: today_str.clone(),
                delivery_date: Some(today_str.clone()),
                revision: 1,
                created_at: now.clone(),
                updated_at: now.clone(),
                notes: vec![OrderNote {
                    id: "nota_inicial".to_string(),
                    timestamp: now.clone(),
                    stage: "recepcion".to_string(),
                    content: "Tabla pedidos inicializada en Firestore.".to_string(),
                }],
                supplies: Vec::new(),
                images: Vec::new(),
                storage_images: Vec::new(),
            });
        }

        if snap.collections.movimientos.is_empty() {
            let order_id = snap.collections.pedidos[0].id.clone();
            let det_id = deterministic_movement_id(&order_id, 1, &first_supply_id);
            snap.collections.movimientos.push(SupplyMovement {
                id: det_id,
                supply_id: first_supply_id,
                timestamp: now.clone(),
                delta: 1.0,
                kind: "entrada".to_string(),
                reason: "Inicialización de kárdex en Cloud Firestore".to_string(),
                order_id: Some(order_id),
                order_revision: Some(1),
            });
        }

        if snap.collections.agenda.is_empty() {
            snap.collections.agenda.push(Reminder {
                id: "age_inicial_taller".to_string(),
                title: "Revisión de agenda sincronizada".to_string(),
                description: "Recordatorio inicial con dueDate YYYY-MM-DD y dueTime HH:mm separados.".to_string(),
                due_date: today_str,
                due_time: Some("09:00".to_string()),
                priority: "normal".to_string(),
                completed: true,
                customer_id: Some(first_customer_id),
                order_id: None,
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        if snap.collections.auditoria.is_empty() {
            let safe_uid = if uid.trim().is_empty() {
                "PENDIENTE_UID_FIREBASE"
            } else {
                uid.trim()
            };
            snap.collections.auditoria.push(AuditEvent {
                id: format!("aud_init_{}", new_id()),
                timestamp: now,
                action: "workshop.firestore_init".to_string(),
                entity_type: "talleres".to_string(),
                entity_id: safe_uid.to_string(),
                summary: "Estructura de tablas v2 inicializada en Cloud Firestore".to_string(),
            });
        }

        snap
    }
}

// ────────────────────────────── Entradas de la UI ──────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomerInput {
    pub id: Option<String>,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
}

impl Default for CustomerInput {
    fn default() -> Self {
        Self {
            id: None,
            full_name: String::new(),
            phone: String::new(),
            email: String::new(),
            address: String::new(),
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OrderInput {
    pub id: Option<String>,
    pub customer_id: String,
    pub piece_name: String,
    pub garment_type: String,
    pub custom_type: String,
    pub style_name: String,
    pub status: Option<String>,
    pub priority: String,
    pub measurements: Measurements,
    pub fabric_details: String,
    pub estimated_cost: f64,
    pub deposit_paid: f64,
    pub due_date: String,
    pub initial_note: String,
    pub supplies: Vec<SupplyUsage>,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ReminderInput {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
    pub due_date: String,
    pub due_time: Option<String>,
    pub priority: String,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SupplyInput {
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub unit: String,
    pub stock: f64,
    pub min_stock: f64,
    pub unit_cost: f64,
    pub supplier: String,
    pub notes: String,
}

// ────────────────────────────── Validación ──────────────────────────────

pub fn is_valid_garment(value: &str) -> bool {
    GARMENT_TYPES.contains(&value)
}

pub fn is_valid_status(value: &str) -> bool {
    ORDER_STATUSES.contains(&value)
}

pub fn is_valid_priority(value: &str) -> bool {
    PRIORITIES.contains(&value)
}

pub fn is_valid_category(value: &str) -> bool {
    SUPPLY_CATEGORIES.contains(&value)
}

pub fn is_valid_unit(value: &str) -> bool {
    SUPPLY_UNITS.contains(&value)
}

pub fn is_valid_cut(value: &str) -> bool {
    CUT_DIRECTIONS.contains(&value)
}

pub fn parse_date(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
}

pub fn parse_time(value: &str) -> Option<NaiveTime> {
    let trimmed = value.trim();
    if trimmed.len() != 5 {
        return None;
    }
    NaiveTime::parse_from_str(trimmed, "%H:%M").ok()
}

fn clean(value: &str) -> String {
    value.trim().to_string()
}

pub fn normalize_customer(input: &CustomerInput) -> Result<Customer, String> {
    let full_name = clean(&input.full_name);
    if full_name.is_empty() {
        return Err("Escribe el nombre del cliente.".into());
    }
    if full_name.chars().count() > 120 {
        return Err("El nombre del cliente es demasiado largo.".into());
    }
    let email = clean(&input.email);
    if !email.is_empty() && (!email.contains('@') || email.len() > 160) {
        return Err("El correo electrónico no parece válido.".into());
    }
    let phone = clean(&input.phone);
    if phone.chars().count() > 40 {
        return Err("El teléfono es demasiado largo.".into());
    }
    let address = clean(&input.address);
    if address.chars().count() > 300 {
        return Err("La dirección es demasiado larga.".into());
    }
    let notes = clean(&input.notes);
    if notes.chars().count() > 2000 {
        return Err("Las notas del cliente son demasiado largas.".into());
    }

    Ok(Customer {
        id: input.id.clone().unwrap_or_default(),
        full_name,
        phone,
        email,
        address,
        notes,
        pending_balance: 0.0,
        active_orders_count: 0,
        created_at: String::new(),
        updated_at: now_iso(),
    })
}

pub fn normalize_order(input: &OrderInput, data: &AppData) -> Result<Order, String> {
    if !data.customers.iter().any(|c| c.id == input.customer_id) {
        return Err("Selecciona un cliente válido para el pedido.".into());
    }
    let piece_name = clean(&input.piece_name);
    if piece_name.is_empty() {
        return Err("Escribe el nombre de la prenda o proyecto.".into());
    }
    if piece_name.chars().count() > 160 {
        return Err("El nombre de la prenda es demasiado largo.".into());
    }

    let garment_type = clean(&input.garment_type).to_lowercase();
    if !is_valid_garment(&garment_type) {
        return Err("El tipo de prenda no es válido.".into());
    }

    let priority = clean(&input.priority).to_lowercase();
    if !is_valid_priority(&priority) {
        return Err("La prioridad no es válida.".into());
    }

    let status = input
        .status
        .as_deref()
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|| "pendiente".to_string());
    if !is_valid_status(&status) {
        return Err("El estado del pedido no es válido.".into());
    }

    let due_date = clean(&input.due_date);
    if parse_date(&due_date).is_none() {
        return Err("La fecha de entrega no es válida.".into());
    }

    if !input.estimated_cost.is_finite() || input.estimated_cost < 0.0 {
        return Err("El costo no puede ser negativo.".into());
    }
    if !input.deposit_paid.is_finite() || input.deposit_paid < 0.0 {
        return Err("El anticipo no puede ser negativo.".into());
    }
    if input.estimated_cost > 10_000_000.0 || input.deposit_paid > 10_000_000.0 {
        return Err("El monto es demasiado alto.".into());
    }

    for (key, value) in &input.measurements.values {
        if !value.is_finite() || *value < 0.0 || *value > 400.0 {
            return Err(format!(
                "La medida '{}' debe estar entre 0 y 400 cm.",
                key
            ));
        }
    }
    if input.measurements.notes.chars().count() > 1000 {
        return Err("Las notas de medidas son demasiado largas.".into());
    }

    let fabric_details = clean(&input.fabric_details);
    if fabric_details.chars().count() > 1500 {
        return Err("Los detalles de la tela son demasiado largos.".into());
    }
    let style_name = clean(&input.style_name);
    if style_name.chars().count() > 80 {
        return Err("El estilo es demasiado largo.".into());
    }

    for usage in &input.supplies {
        let supply = data
            .supplies
            .iter()
            .find(|s| s.id == usage.supply_id)
            .ok_or_else(|| "Un insumo del pedido ya no existe. Actualiza la lista.".to_string())?;
        if !usage.quantity.is_finite() || usage.quantity <= 0.0 || usage.quantity > 1_000_000.0 {
            return Err(format!(
                "La cantidad de «{}» debe ser mayor que cero.",
                supply.name
            ));
        }
        if !is_valid_cut(&usage.cut) {
            return Err("La dirección de corte no es válida.".into());
        }
        if usage.note.chars().count() > 500 {
            return Err("La nota del insumo es demasiado larga.".into());
        }
    }

    let mut measurements = input.measurements.clone();
    measurements.kind = garment_type.clone();
    measurements.notes = clean(&measurements.notes);
    measurements
        .options
        .retain(|key, value| !key.trim().is_empty() && !value.trim().is_empty());

    let custom_type = clean(&input.custom_type);
    if custom_type.chars().count() > 80 {
        return Err("El subtipo personalizado es demasiado largo.".into());
    }

    // Límite ISO/Storage v2: máximo 4 imágenes de referencia por pedido
    let images = input
        .images
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(4)
        .collect::<Vec<_>>();

    Ok(Order {
        id: input.id.clone().unwrap_or_default(),
        customer_id: input.customer_id.clone(),
        piece_name,
        garment_type,
        custom_type,
        style_name,
        status,
        priority,
        measurements,
        fabric_details,
        estimated_cost: round_money(input.estimated_cost),
        deposit_paid: round_money(input.deposit_paid),
        due_date,
        delivery_date: None,
        revision: 1,
        created_at: String::new(),
        updated_at: now_iso(),
        notes: Vec::new(),
        supplies: input.supplies.clone(),
        images,
        storage_images: Vec::new(),
    })
}

pub fn normalize_supply(input: &SupplyInput) -> Result<Supply, String> {
    let name = clean(&input.name);
    if name.is_empty() {
        return Err("Escribe el nombre del insumo.".into());
    }
    if name.chars().count() > 120 {
        return Err("El nombre del insumo es demasiado largo.".into());
    }

    let category = clean(&input.category).to_lowercase();
    if !is_valid_category(&category) {
        return Err("La categoría del insumo no es válida.".into());
    }
    let unit = clean(&input.unit).to_lowercase();
    if !is_valid_unit(&unit) {
        return Err("La unidad del insumo no es válida.".into());
    }

    if !input.stock.is_finite() || input.stock < -1_000_000.0 || input.stock > 1_000_000.0 {
        return Err("La existencia del insumo no es válida.".into());
    }
    if !input.min_stock.is_finite() || input.min_stock < 0.0 || input.min_stock > 1_000_000.0 {
        return Err("El mínimo de existencias no es válido.".into());
    }
    if !input.unit_cost.is_finite() || input.unit_cost < 0.0 || input.unit_cost > 1_000_000.0 {
        return Err("El costo unitario no es válido.".into());
    }

    let supplier = clean(&input.supplier);
    if supplier.chars().count() > 160 {
        return Err("El proveedor es demasiado largo.".into());
    }
    let notes = clean(&input.notes);
    if notes.chars().count() > 1000 {
        return Err("Las notas del insumo son demasiado largas.".into());
    }

    Ok(Supply {
        id: input.id.clone().unwrap_or_default(),
        name,
        category,
        unit,
        stock: round_money(input.stock),
        min_stock: round_money(input.min_stock),
        unit_cost: round_money(input.unit_cost),
        supplier,
        notes,
        created_at: String::new(),
        updated_at: now_iso(),
    })
}

pub fn normalize_reminder(input: &ReminderInput) -> Result<Reminder, String> {
    let title = clean(&input.title);
    if title.is_empty() {
        return Err("Escribe el título del recordatorio.".into());
    }
    if title.chars().count() > 140 {
        return Err("El título del recordatorio es demasiado largo.".into());
    }
    let due_date = clean(&input.due_date);
    if parse_date(&due_date).is_none() {
        return Err("La fecha del recordatorio debe tener formato YYYY-MM-DD.".into());
    }
    let due_time = match input.due_time.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            if parse_time(t).is_none() {
                return Err("La hora del recordatorio debe tener formato HH:mm (24h).".into());
            }
            Some(t.to_string())
        }
        None => None,
    };
    let priority = clean(&input.priority).to_lowercase();
    if !is_valid_priority(&priority) {
        return Err("La prioridad no es válida.".into());
    }
    let description = clean(&input.description);
    if description.chars().count() > 1500 {
        return Err("La descripción del recordatorio es demasiado larga.".into());
    }

    Ok(Reminder {
        id: input.id.clone().unwrap_or_default(),
        title,
        description,
        due_date,
        due_time,
        priority,
        completed: false,
        customer_id: input.customer_id.clone().filter(|id| !id.is_empty()),
        order_id: input.order_id.clone().filter(|id| !id.is_empty()),
        created_at: String::new(),
        updated_at: now_iso(),
    })
}

pub fn round_money(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_data() -> AppData {
        let customer = Customer {
            id: "c1".into(),
            full_name: "Ana".into(),
            ..Default::default()
        };
        AppData {
            customers: vec![customer],
            ..Default::default()
        }
    }

    #[test]
    fn order_requires_existing_customer() {
        let data = AppData::default();
        let input = OrderInput {
            customer_id: "nope".into(),
            piece_name: "Blusa".into(),
            garment_type: "camisa".into(),
            priority: "normal".into(),
            due_date: today(),
            ..Default::default()
        };
        assert!(normalize_order(&input, &data).is_err());
    }

    #[test]
    fn order_rejects_negative_cost() {
        let data = base_data();
        let input = OrderInput {
            customer_id: "c1".into(),
            piece_name: "Blusa".into(),
            garment_type: "camisa".into(),
            priority: "normal".into(),
            due_date: today(),
            estimated_cost: -5.0,
            ..Default::default()
        };
        assert!(normalize_order(&input, &data).is_err());
    }

    #[test]
    fn order_normalizes_measurements_kind() {
        let data = base_data();
        let mut values = BTreeMap::new();
        values.insert("pecho".to_string(), 96.0);
        let input = OrderInput {
            customer_id: "c1".into(),
            piece_name: "Blusa".into(),
            garment_type: "camisa".into(),
            priority: "normal".into(),
            due_date: today(),
            measurements: Measurements {
                kind: "pantalon".into(),
                values,
                ..Default::default()
            },
            ..Default::default()
        };
        let order = normalize_order(&input, &data).unwrap();
        assert_eq!(order.measurements.kind, "camisa");
        assert_eq!(order.balance(), 0.0);
        assert_eq!(order.revision, 1);
    }

    #[test]
    fn deterministic_movement_id_is_stable() {
        let id = deterministic_movement_id("ord-123", 2, "sup-456");
        assert_eq!(id, "mov_ord-123_r2_sup-456");
    }

    #[test]
    fn customer_aggregates_reflect_active_orders() {
        let mut data = base_data();
        data.orders.push(Order {
            id: "o1".into(),
            customer_id: "c1".into(),
            piece_name: "Camisa".into(),
            status: "en_corte".into(),
            estimated_cost: 50.0,
            deposit_paid: 20.0,
            ..Default::default()
        });
        data.orders.push(Order {
            id: "o2".into(),
            customer_id: "c1".into(),
            piece_name: "Pantalón".into(),
            status: "entregado".into(),
            estimated_cost: 40.0,
            deposit_paid: 10.0,
            ..Default::default()
        });
        data.refresh_customer_aggregates();
        assert_eq!(data.customers[0].pending_balance, 30.0);
        assert_eq!(data.customers[0].active_orders_count, 1);
    }

    #[test]
    fn reminder_validates_strict_date_and_optional_time() {
        let input = ReminderInput {
            title: "Prueba".into(),
            due_date: "2026-11-12".into(),
            due_time: Some("16:00".into()),
            priority: "normal".into(),
            ..Default::default()
        };
        let normalized = normalize_reminder(&input).unwrap();
        assert_eq!(normalized.due_date, "2026-11-12");
        assert_eq!(normalized.due_time.as_deref(), Some("16:00"));

        // Sin hora también es válido (dueTime opcional: None)
        let date_only = ReminderInput {
            due_time: None,
            ..input.clone()
        };
        assert_eq!(normalize_reminder(&date_only).unwrap().due_time, None);

        // Rechaza formatos combinados tipo YYYY-MM-DDTHH:mm en due_date
        let combined = ReminderInput {
            due_date: "2026-11-12T16:00".into(),
            ..input.clone()
        };
        assert!(normalize_reminder(&combined).is_err());

        // Rechaza hora inválida
        let bad_time = ReminderInput {
            due_time: Some("25:99".into()),
            ..input
        };
        assert!(normalize_reminder(&bad_time).is_err());
    }

    #[test]
    fn dotenv_parser_populates_firebase_slots_and_redacts_disk_secret() {
        let mut slots = FirebaseConnectionSlots::default();
        assert!(!slots.is_cloud_configured());

        let env_content = r#"
            # Configuración Firebase
            FIREBASE_API_KEY="AIzaTestKey1234567890"
            FIREBASE_AUTH_DOMAIN="tallercostura-9f69d.firebaseapp.com"
            FIREBASE_PROJECT_ID="tallercostura-9f69d"
            FIREBASE_STORAGE_BUCKET="tallercostura-9f69d.firebasestorage.app"
            FIREBASE_MESSAGING_SENDER_ID="581229678751"
            FIREBASE_APP_ID="1:581229678751:web:22fe1c03de7c884fcfd2aa"
        "#;

        assert!(slots.apply_dotenv_str(env_content));
        assert!(slots.is_cloud_configured());
        assert_eq!(slots.project_id, "tallercostura-9f69d");
        assert_eq!(slots.api_key, "AIzaTestKey1234567890");
        assert_eq!(slots.mode, "cloud");
        assert!(!slots.use_emulators);

        let redacted = slots.redacted_for_disk();
        assert_eq!(redacted.project_id, "tallercostura-9f69d");
        assert_eq!(redacted.api_key, "REEMPLAZAR_CON_FIREBASE_API_KEY");
    }

    #[test]
    fn materialized_firestore_snapshot_satisfies_all_collection_rules() {
        let data = AppData::default();
        let snap = data.to_materialized_firestore_snapshot("uid_test_123");
        assert_eq!(snap.root_path, "/talleres/uid_test_123");
        assert_eq!(snap.workshop_doc.version, DATA_VERSION);
        assert!(!snap.workshop_doc.workshop_name.is_empty());
        assert!((0.0..=100.0).contains(&snap.workshop_doc.tax_rate_percent));
        assert!(matches!(snap.workshop_doc.language.as_str(), "es" | "en" | "pt"));

        // Verifica que las 6 subcolecciones contengan al menos 1 documento válido
        assert!(!snap.collections.clientes.is_empty());
        assert!(!snap.collections.pedidos.is_empty());
        assert!(!snap.collections.insumos.is_empty());
        assert!(!snap.collections.movimientos.is_empty());
        assert!(!snap.collections.agenda.is_empty());
        assert!(!snap.collections.auditoria.is_empty());

        // Verifica agenda: dueDate YYYY-MM-DD (10 chars) y dueTime HH:mm (5 chars)
        let rem = &snap.collections.agenda[0];
        assert_eq!(rem.due_date.len(), 10);
        assert!(parse_date(&rem.due_date).is_some());
        if let Some(t) = &rem.due_time {
            assert_eq!(t.len(), 5);
            assert!(parse_time(t).is_some());
        }

        // Verifica movimiento determinista
        let mov = &snap.collections.movimientos[0];
        assert!(mov.id.starts_with("mov_"));
        assert!(matches!(mov.kind.as_str(), "entrada" | "salida" | "devolucion" | "ajuste"));
    }
}
