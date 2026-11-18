use chrono::{NaiveDate, NaiveDateTime, SecondsFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DATA_VERSION: u32 = 1;

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

// ────────────────────────────── Ajustes ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub workshop_name: String,
    pub owner_name: String,
    pub currency: String,
    pub default_due_days: i64,
    pub backup_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            workshop_name: "Mi taller de costura".to_string(),
            owner_name: String::new(),
            currency: "$".to_string(),
            default_due_days: 7,
            backup_dir: String::new(),
        }
    }
}

// ────────────────────────────── Datos ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppData {
    pub version: u32,
    pub settings: Settings,
    pub customers: Vec<Customer>,
    pub orders: Vec<Order>,
    pub reminders: Vec<Reminder>,
    pub supplies: Vec<Supply>,
    pub movements: Vec<SupplyMovement>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            settings: Settings::default(),
            customers: Vec::new(),
            orders: Vec::new(),
            reminders: Vec::new(),
            supplies: Vec::new(),
            movements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Customer {
    pub id: String,
    pub full_name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub notes: String,
    pub created_at: String,
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
            created_at: String::new(),
        }
    }
}

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
    pub created_at: String,
    pub notes: Vec<OrderNote>,
    pub supplies: Vec<SupplyUsage>,
    pub images: Vec<String>,
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
            created_at: String::new(),
            notes: Vec::new(),
            supplies: Vec::new(),
            images: Vec::new(),
        }
    }
}

impl Order {
    pub fn balance(&self) -> f64 {
        (self.estimated_cost - self.deposit_paid).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub description: String,
    pub due_date: String,
    pub priority: String,
    pub completed: bool,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub created_at: String,
}

impl Default for Reminder {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            due_date: String::new(),
            priority: "normal".to_string(),
            completed: false,
            customer_id: None,
            order_id: None,
            created_at: String::new(),
        }
    }
}

// ────────────────────────────── Insumos ──────────────────────────────

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
        }
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
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()
}

pub fn parse_datetime(value: &str) -> Option<NaiveDateTime> {
    let value = value.trim();
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M"))
        .ok()
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
        created_at: String::new(),
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

    if input.measurements.kind != garment_type {
        // el tipo manda; se corrige abajo
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
    let images = input
        .images
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .take(30)
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
        created_at: String::new(),
        notes: Vec::new(),
        supplies: input.supplies.clone(),
        images,
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
    if parse_datetime(&due_date).is_none() {
        return Err("La fecha y hora del recordatorio no son válidas.".into());
    }
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
        priority,
        completed: false,
        customer_id: input.customer_id.clone().filter(|id| !id.is_empty()),
        order_id: input.order_id.clone().filter(|id| !id.is_empty()),
        created_at: String::new(),
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
    }

    #[test]
    fn reminder_validates_datetime() {
        let input = ReminderInput {
            title: "Prueba".into(),
            due_date: "2026-11-12T16:00".into(),
            priority: "normal".into(),
            ..Default::default()
        };
        assert!(normalize_reminder(&input).is_ok());
        let bad = ReminderInput {
            due_date: "mañana".into(),
            ..input.clone()
        };
        assert!(normalize_reminder(&bad).is_err());
    }
}
