use crate::model::*;
use crate::store::BackupInfo;
use crate::AppState;
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::State;

type Db<'a> = State<'a, Mutex<AppState>>;

fn lock<'a>(state: &'a Db<'_>) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    state
        .lock()
        .map_err(|_| "El estado de la aplicación está bloqueado. Reinicia Costura.".to_string())
}

fn mutate<R>(
    state: &Db<'_>,
    change: impl FnOnce(&mut AppState) -> Result<R, String>,
) -> Result<(R, AppData), String> {
    let mut guard = lock(state)?;
    let result = change(&mut guard)?;
    guard
        .store
        .save(&guard.data)
        .map_err(|e| format!("No se pudieron guardar los cambios: {e}"))?;
    let snapshot = guard.data.clone();
    Ok((result, snapshot))
}

fn save_only(state: &Db<'_>, data: &AppData) -> Result<AppData, String> {
    let mut guard = lock(state)?;
    guard.data = data.clone();
    guard
        .store
        .save(&guard.data)
        .map_err(|e| format!("No se pudieron guardar los cambios: {e}"))?;
    Ok(guard.data.clone())
}

// ────────────────────────────── Consulta ──────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    data: AppData,
    data_dir: String,
    data_file: String,
    backups_dir: String,
    app_version: String,
    warning: Option<String>,
}

#[tauri::command]
pub fn get_state(state: Db<'_>) -> Result<AppStateDto, String> {
    let guard = lock(&state)?;
    Ok(AppStateDto {
        data: guard.data.clone(),
        data_dir: guard.store.dir().to_string_lossy().to_string(),
        data_file: guard.store.data_file().to_string_lossy().to_string(),
        backups_dir: guard
            .store
            .resolve_backups_dir(&guard.data)
            .to_string_lossy()
            .to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        warning: guard.warning.clone(),
    })
}

#[tauri::command]
pub fn dismiss_warning(state: Db<'_>) -> Result<(), String> {
    let mut guard = lock(&state)?;
    guard.warning = None;
    Ok(())
}

// ────────────────────────────── Clientes ──────────────────────────────

#[tauri::command]
pub fn save_customer(state: Db<'_>, input: CustomerInput) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let customer = normalize_customer(&input)?;
        match input.id.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => {
                let existing = app
                    .data
                    .customers
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or_else(|| "El cliente ya no existe.".to_string())?;
                existing.full_name = customer.full_name;
                existing.phone = customer.phone;
                existing.email = customer.email;
                existing.address = customer.address;
                existing.notes = customer.notes;
            }
            None => {
                app.data.customers.push(Customer {
                    id: new_id(),
                    created_at: now_iso(),
                    ..customer
                });
            }
        }
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn delete_customer(state: Db<'_>, id: String) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        if !app.data.customers.iter().any(|c| c.id == id) {
            return Err("El cliente ya no existe.".to_string());
        }
        let order_ids: Vec<String> = app
            .data
            .orders
            .iter()
            .filter(|o| o.customer_id == id)
            .map(|o| o.id.clone())
            .collect();

        app.data
            .orders
            .retain(|o| !order_ids.contains(&o.id));
        app.data.reminders.retain(|r| {
            r.customer_id.as_deref() != Some(id.as_str())
                && !r
                    .order_id
                    .as_deref()
                    .map(|oid| order_ids.iter().any(|x| x == oid))
                    .unwrap_or(false)
        });
        app.data.customers.retain(|c| c.id != id);
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Pedidos ──────────────────────────────

fn trim_movements(movements: &mut Vec<SupplyMovement>) {
    if movements.len() > 3000 {
        let excess = movements.len() - 3000;
        movements.drain(0..excess);
    }
}

fn apply_supply_deltas(
    app: &mut AppState,
    previous: Option<&Order>,
    next: &[SupplyUsage],
    order_id: &str,
    piece_name: &str,
    prefix: &str,
) -> Result<(), String> {
    let mut deltas: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    if let Some(prev) = previous {
        for usage in &prev.supplies {
            *deltas.entry(usage.supply_id.clone()).or_insert(0.0) -= usage.quantity;
        }
    }
    for usage in next {
        *deltas.entry(usage.supply_id.clone()).or_insert(0.0) += usage.quantity;
    }
    deltas.retain(|_, value| value.abs() > 1e-9);
    if deltas.is_empty() {
        return Ok(());
    }

    for supply_id in deltas.keys() {
        if !app.data.supplies.iter().any(|s| &s.id == supply_id) {
            return Err("Uno de los insumos del pedido ya no existe.".into());
        }
    }

    for (supply_id, delta) in deltas {
        if let Some(supply) = app.data.supplies.iter_mut().find(|s| s.id == supply_id) {
            supply.stock = round_money(supply.stock - delta);
        }
        app.data.movements.push(SupplyMovement {
            id: new_id(),
            supply_id,
            timestamp: now_iso(),
            delta: round_money(-delta),
            kind: if delta > 0.0 { "salida" } else { "devolucion" }.to_string(),
            reason: format!("{prefix}: {piece_name}"),
            order_id: Some(order_id.to_string()).filter(|id| !id.is_empty()),
        });
    }
    trim_movements(&mut app.data.movements);
    Ok(())
}

#[tauri::command]
pub fn save_order(state: Db<'_>, input: OrderInput) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let order = normalize_order(&input, &app.data)?;
        let editing_id = input
            .id
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let initial_note = input.initial_note.trim().to_string();

        match editing_id {
            Some(id) => {
                let index = app
                    .data
                    .orders
                    .iter()
                    .position(|o| o.id == id)
                    .ok_or_else(|| "El pedido ya no existe.".to_string())?;
                let previous = app.data.orders[index].clone();

                {
                    let existing = &mut app.data.orders[index];
                    let was_delivered = existing.status == "entregado";
                    existing.customer_id = order.customer_id;
                    existing.piece_name = order.piece_name.clone();
                    existing.garment_type = order.garment_type;
                    existing.custom_type = order.custom_type;
                    existing.style_name = order.style_name;
                    existing.status = order.status.clone();
                    existing.priority = order.priority;
                    existing.measurements = order.measurements;
                    existing.fabric_details = order.fabric_details;
                    existing.estimated_cost = order.estimated_cost;
                    existing.deposit_paid = order.deposit_paid;
                    existing.due_date = order.due_date;
                    existing.supplies = order.supplies.clone();
                    existing.images = order.images;

                    if existing.status == "entregado" && !was_delivered {
                        existing.delivery_date = Some(today());
                    }

                    if !initial_note.is_empty() {
                        existing.notes.push(OrderNote {
                            id: new_id(),
                            timestamp: now_iso(),
                            stage: "seguimiento".into(),
                            content: initial_note.clone(),
                        });
                    }
                }

                apply_supply_deltas(
                    app,
                    Some(&previous),
                    &order.supplies,
                    &id,
                    &order.piece_name,
                    "Pedido editado",
                )?;
            }
            None => {
                let mut created = Order {
                    id: new_id(),
                    created_at: now_iso(),
                    ..order
                };
                if !initial_note.is_empty() {
                    created.notes.push(OrderNote {
                        id: new_id(),
                        timestamp: now_iso(),
                        stage: "recepcion".into(),
                        content: initial_note,
                    });
                }
                let order_id = created.id.clone();
                let piece_name = created.piece_name.clone();
                let supplies = created.supplies.clone();
                apply_supply_deltas(app, None, &supplies, &order_id, &piece_name, "Pedido creado")?;
                app.data.orders.push(created);
            }
        }
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn delete_order(state: Db<'_>, id: String) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let order = app
            .data
            .orders
            .iter()
            .find(|o| o.id == id)
            .cloned()
            .ok_or_else(|| "El pedido ya no existe.".to_string())?;

        apply_supply_deltas(
            app,
            Some(&order),
            &[],
            &order.id,
            &order.piece_name,
            "Pedido eliminado",
        )?;

        app.data.orders.retain(|o| o.id != id);
        app.data
            .reminders
            .retain(|r| r.order_id.as_deref() != Some(id.as_str()));
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn set_order_status(
    state: Db<'_>,
    id: String,
    status: String,
    note: Option<String>,
) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let status = status.trim().to_lowercase();
        if !is_valid_status(&status) {
            return Err("Estado no válido.".to_string());
        }
        let order = app
            .data
            .orders
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or_else(|| "El pedido ya no existe.".to_string())?;

        if order.status != status {
            order.status = status.clone();
            if status == "entregado" && order.delivery_date.is_none() {
                order.delivery_date = Some(today());
            }
            let note_text = note.unwrap_or_default();
            if !note_text.trim().is_empty() {
                order.notes.push(OrderNote {
                    id: new_id(),
                    timestamp: now_iso(),
                    stage: "estado".into(),
                    content: note_text.trim().to_string(),
                });
            }
        }
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn add_order_note(
    state: Db<'_>,
    order_id: String,
    stage: String,
    content: String,
) -> Result<AppData, String> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("Escribe el contenido de la nota.".to_string());
    }
    if content.chars().count() > 2000 {
        return Err("La nota es demasiado larga.".to_string());
    }
    let stage = if stage.trim().is_empty() {
        "seguimiento".to_string()
    } else {
        stage.trim().to_lowercase()
    };

    let (_, data) = mutate(&state, |app| {
        let order = app
            .data
            .orders
            .iter_mut()
            .find(|o| o.id == order_id)
            .ok_or_else(|| "El pedido ya no existe.".to_string())?;
        order.notes.push(OrderNote {
            id: new_id(),
            timestamp: now_iso(),
            stage,
            content,
        });
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn delete_order_note(
    state: Db<'_>,
    order_id: String,
    note_id: String,
) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let order = app
            .data
            .orders
            .iter_mut()
            .find(|o| o.id == order_id)
            .ok_or_else(|| "El pedido ya no existe.".to_string())?;
        order.notes.retain(|n| n.id != note_id);
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Recordatorios ──────────────────────────────

#[tauri::command]
pub fn save_reminder(state: Db<'_>, input: ReminderInput) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let reminder = normalize_reminder(&input)?;
        let customer_ok = reminder
            .customer_id
            .as_deref()
            .map(|id| app.data.customers.iter().any(|c| c.id == id))
            .unwrap_or(true);
        let order_ok = reminder
            .order_id
            .as_deref()
            .map(|id| app.data.orders.iter().any(|o| o.id == id))
            .unwrap_or(true);
        if !customer_ok || !order_ok {
            return Err("El cliente o pedido asociado ya no existe.".to_string());
        }

        match input.id.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => {
                let existing = app
                    .data
                    .reminders
                    .iter_mut()
                    .find(|r| r.id == id)
                    .ok_or_else(|| "El recordatorio ya no existe.".to_string())?;
                existing.title = reminder.title;
                existing.description = reminder.description;
                existing.due_date = reminder.due_date;
                existing.priority = reminder.priority;
                existing.customer_id = reminder.customer_id;
                existing.order_id = reminder.order_id;
            }
            None => {
                app.data.reminders.push(Reminder {
                    id: new_id(),
                    created_at: now_iso(),
                    ..reminder
                });
            }
        }
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn toggle_reminder(state: Db<'_>, id: String) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let reminder = app
            .data
            .reminders
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| "El recordatorio ya no existe.".to_string())?;
        reminder.completed = !reminder.completed;
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn delete_reminder(state: Db<'_>, id: String) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        if !app.data.reminders.iter().any(|r| r.id == id) {
            return Err("El recordatorio ya no existe.".to_string());
        }
        app.data.reminders.retain(|r| r.id != id);
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Insumos ──────────────────────────────

#[tauri::command]
pub fn save_supply(state: Db<'_>, input: SupplyInput) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let supply = normalize_supply(&input)?;
        match input.id.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => {
                let index = app
                    .data
                    .supplies
                    .iter()
                    .position(|s| s.id == id)
                    .ok_or_else(|| "El insumo ya no existe.".to_string())?;

                let (delta, supply_id) = {
                    let existing = &mut app.data.supplies[index];
                    let delta = supply.stock - existing.stock;
                    let supply_id = existing.id.clone();
                    existing.name = supply.name;
                    existing.category = supply.category;
                    existing.unit = supply.unit;
                    existing.stock = supply.stock;
                    existing.min_stock = supply.min_stock;
                    existing.unit_cost = supply.unit_cost;
                    existing.supplier = supply.supplier;
                    existing.notes = supply.notes;
                    (delta, supply_id)
                };

                if delta.abs() > 1e-9 {
                    app.data.movements.push(SupplyMovement {
                        id: new_id(),
                        supply_id,
                        timestamp: now_iso(),
                        delta: round_money(delta),
                        kind: "ajuste".into(),
                        reason: "Ajuste manual de existencias".into(),
                        order_id: None,
                    });
                }
            }
            None => {
                let created = Supply {
                    id: new_id(),
                    created_at: now_iso(),
                    ..supply
                };
                let initial = created.stock;
                let supply_id = created.id.clone();
                app.data.supplies.push(created);
                if initial.abs() > 1e-9 {
                    app.data.movements.push(SupplyMovement {
                        id: new_id(),
                        supply_id,
                        timestamp: now_iso(),
                        delta: initial,
                        kind: "entrada".into(),
                        reason: "Existencia inicial".into(),
                        order_id: None,
                    });
                }
            }
        }
        trim_movements(&mut app.data.movements);
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn delete_supply(state: Db<'_>, id: String) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        if !app.data.supplies.iter().any(|s| s.id == id) {
            return Err("El insumo ya no existe.".to_string());
        }
        let used = app
            .data
            .orders
            .iter()
            .filter(|o| o.supplies.iter().any(|u| u.supply_id == id))
            .count();
        if used > 0 {
            return Err(format!(
                "Este insumo se usa en {used} pedido{}. Quita el consumo de esos pedidos antes de eliminarlo.",
                if used == 1 { "" } else { "s" }
            ));
        }
        app.data.supplies.retain(|s| s.id != id);
        app.data.movements.retain(|m| m.supply_id != id);
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn add_supply_stock(
    state: Db<'_>,
    id: String,
    quantity: f64,
    reason: Option<String>,
) -> Result<AppData, String> {
    if !quantity.is_finite() || quantity <= 0.0 || quantity > 1_000_000.0 {
        return Err("La cantidad de entrada debe ser mayor que cero.".to_string());
    }
    let (_, data) = mutate(&state, |app| {
        let supply = app
            .data
            .supplies
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| "El insumo ya no existe.".to_string())?;
        supply.stock = round_money(supply.stock + quantity);
        let reason = reason
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| "Entrada de mercancía".to_string());
        let supply_id = supply.id.clone();
        app.data.movements.push(SupplyMovement {
            id: new_id(),
            supply_id,
            timestamp: now_iso(),
            delta: round_money(quantity),
            kind: "entrada".into(),
            reason,
            order_id: None,
        });
        trim_movements(&mut app.data.movements);
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Ajustes ──────────────────────────────

#[tauri::command]
pub fn save_settings(
    state: Db<'_>,
    workshop_name: String,
    owner_name: String,
    currency: String,
    default_due_days: i64,
    language: Option<String>,
) -> Result<AppData, String> {
    let name = workshop_name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("El nombre del taller debe tener entre 1 y 80 caracteres.".to_string());
    }
    if !(1..=365).contains(&default_due_days) {
        return Err("Los días de entrega deben estar entre 1 y 365.".to_string());
    }
    let currency = currency.trim().to_string();
    if currency.chars().count() > 6 {
        return Err("El símbolo de moneda es demasiado largo.".to_string());
    }
    let lang = language
        .map(|l| l.trim().to_lowercase())
        .filter(|l| matches!(l.as_str(), "es" | "en" | "pt"))
        .unwrap_or_else(|| "es".to_string());

    let (_, data) = mutate(&state, |app| {
        app.data.settings.workshop_name = name;
        app.data.settings.owner_name = owner_name.trim().to_string();
        app.data.settings.currency = currency;
        app.data.settings.default_due_days = default_due_days;
        app.data.settings.language = lang;
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Respaldos ──────────────────────────────

#[tauri::command]
pub fn create_backup(state: Db<'_>) -> Result<BackupInfo, String> {
    let guard = lock(&state)?;
    guard.store.create_backup(&guard.data, false)
}

#[tauri::command]
pub fn list_backups(state: Db<'_>) -> Result<Vec<BackupInfo>, String> {
    let guard = lock(&state)?;
    Ok(guard.store.list_backups(&guard.data))
}

#[tauri::command]
pub fn restore_backup(state: Db<'_>, file: String) -> Result<AppData, String> {
    let restored = {
        let guard = lock(&state)?;
        let restored = guard.store.read_backup(&guard.data, &file)?;
        guard
            .store
            .create_backup(&guard.data, false)
            .map_err(|e| format!("No se pudo respaldar la versión actual antes de restaurar: {e}"))?;
        restored
    };
    save_only(&state, &restored)
}

fn set_backup_dir_impl(state: &Db<'_>, path: &str) -> Result<AppData, String> {
    let trimmed = path.trim().to_string();
    let (_, data) = mutate(state, |app| {
        if trimmed.is_empty() {
            app.data.settings.backup_dir.clear();
            return Ok(());
        }
        let folder = std::path::PathBuf::from(&trimmed);
        std::fs::create_dir_all(&folder)
            .map_err(|e| format!("No se pudo crear la carpeta de respaldos: {e}"))?;
        let probe = folder.join(".costura-escritura.tmp");
        std::fs::write(&probe, b"ok")
            .map_err(|e| format!("La carpeta no permite escribir: {e}"))?;
        let _ = std::fs::remove_file(&probe);
        app.data.settings.backup_dir = folder.to_string_lossy().to_string();
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn set_backup_dir(state: Db<'_>, path: String) -> Result<AppData, String> {
    set_backup_dir_impl(&state, &path)
}

#[tauri::command]
pub fn choose_folder(title: Option<String>) -> Result<Option<String>, String> {
    let title = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Elegir carpeta".to_string());
    Ok(crate::dialog::pick_folder(&title))
}

#[tauri::command]
pub fn open_folder(state: Db<'_>, which: String) -> Result<(), String> {
    let guard = lock(&state)?;
    let path = match which.as_str() {
        "backups" => guard.store.resolve_backups_dir(&guard.data),
        _ => guard.store.dir().to_path_buf(),
    };
    drop(guard);
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|e| format!("No se pudo abrir la carpeta: {e}"))?;
    Ok(())
}

// ────────────────────────────── Mantenimiento ──────────────────────────────

#[tauri::command]
pub fn reset_all(state: Db<'_>) -> Result<AppData, String> {
    let mut guard = lock(&state)?;
    guard
        .store
        .create_backup(&guard.data, false)
        .map_err(|e| format!("No se pudo respaldar antes de borrar: {e}"))?;
    let settings = guard.data.settings.clone();
    let fresh = AppData {
        settings,
        ..Default::default()
    };
    guard.data = fresh;
    guard
        .store
        .save(&guard.data)
        .map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(guard.data.clone())
}

#[tauri::command]
pub fn load_demo(state: Db<'_>) -> Result<AppData, String> {
    let mut guard = lock(&state)?;
    guard
        .store
        .create_backup(&guard.data, false)
        .map_err(|e| format!("No se pudo respaldar antes de cargar el ejemplo: {e}"))?;
    let settings = guard.data.settings.clone();
    let mut demo = demo_data();
    demo.settings = settings;
    guard.data = demo;
    guard
        .store
        .save(&guard.data)
        .map_err(|e| format!("No se pudo guardar: {e}"))?;
    Ok(guard.data.clone())
}

// ────────────────────────────── Datos de ejemplo ──────────────────────────────

pub fn demo_data() -> AppData {
    use chrono::{Duration, Local};

    let base = Local::now().date_naive();
    let day = |offset: i64| (base + Duration::days(offset)).format("%Y-%m-%d").to_string();
    let created = |offset: i64| {
        (Local::now() + Duration::days(offset))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    };

    let maria = Customer {
        id: new_id(),
        full_name: "María José Rivera".into(),
        phone: "+503 7788-1122".into(),
        email: "maria.rivera@example.com".into(),
        address: "Col. Escalón, San Salvador".into(),
        notes: "Prefiere telas naturales. Ajuste holgado en hombros.".into(),
        created_at: created(-24),
    };
    let carlos = Customer {
        id: new_id(),
        full_name: "Carlos Ernesto Mejía".into(),
        phone: "+503 6123-4455".into(),
        email: "carlos.mejia@example.com".into(),
        address: "Col. Layco, San Salvador".into(),
        notes: "Camisas formales y pantalones rectos. Prefiere tonos oscuros.".into(),
        created_at: created(-18),
    };
    let ana = Customer {
        id: new_id(),
        full_name: "Ana Lucía Hernández".into(),
        phone: "+503 7011-9090".into(),
        email: "ana.hernandez@example.com".into(),
        address: "Santa Tecla, La Libertad".into(),
        notes: "Vestidos para eventos. Le gusta el acabado a mano en el ruedo.".into(),
        created_at: created(-11),
    };
    let colegio = Customer {
        id: new_id(),
        full_name: "Colegio San José (uniformes)".into(),
        phone: "+503 2222-3030".into(),
        email: "compras@colegiosanjose.example".into(),
        address: "Av. Olímpica, San Salvador".into(),
        notes: "Pedidos por lote de uniformes. Facturar a nombre de la institución.".into(),
        created_at: created(-40),
    };

    let mut camisa = Measurements {
        kind: "camisa".into(),
        ..Default::default()
    };
    for (k, v) in [
        ("pecho", 98.0),
        ("cintura", 84.0),
        ("cadera", 99.0),
        ("hombros", 45.0),
        ("largoManga", 63.0),
        ("anchoManga", 35.0),
        ("largoTotal", 74.0),
        ("cuello", 41.0),
    ] {
        camisa.values.insert(k.into(), v);
    }
    camisa.options.insert("cuello".into(), "Clásico".into());
    camisa.options.insert("manga".into(), "Larga".into());
    camisa.notes = "Holgura de 4 cm en pecho.".into();

    let mut baggy = Measurements {
        kind: "pantalon".into(),
        ..Default::default()
    };
    for (k, v) in [
        ("cintura", 84.0),
        ("cadera", 108.0),
        ("muslo", 64.0),
        ("rodilla", 52.0),
        ("bota", 50.0),
        ("tiro", 32.0),
        ("largoTotal", 106.0),
    ] {
        baggy.values.insert(k.into(), v);
    }
    baggy.options.insert("corte".into(), "Baggy".into());
    baggy.options.insert("cargo".into(), "No".into());
    baggy.notes = "Caída ancha y doble pinza frontal.".into();

    let mut vestido = Measurements {
        kind: "vestido".into(),
        ..Default::default()
    };
    for (k, v) in [
        ("busto", 90.0),
        ("cintura", 72.0),
        ("cadera", 96.0),
        ("hombros", 38.0),
        ("largoTalle", 39.0),
        ("largoFalda", 70.0),
    ] {
        vestido.values.insert(k.into(), v);
    }
    vestido.options.insert("escote".into(), "En V".into());
    vestido.options.insert("mangas".into(), "Sin mangas".into());
    vestido.notes = "Cierre invisible en la espalda y forro de satén.".into();

    let mut tote = Measurements {
        kind: "bolso".into(),
        ..Default::default()
    };
    for (k, v) in [
        ("ancho", 40.0),
        ("alto", 32.0),
        ("profundidad", 14.0),
        ("largoCorrea", 95.0),
    ] {
        tote.values.insert(k.into(), v);
    }
    tote.options.insert("tipo".into(), "Tote".into());
    tote.options.insert("bolsillo".into(), "Sí".into());
    tote.options.insert("cierre".into(), "Sí".into());

    let ordenes = vec![
        Order {
            id: new_id(),
            customer_id: carlos.id.clone(),
            piece_name: "Camisa de lino blanca".into(),
            garment_type: "camisa".into(),
            style_name: "Clásico elegante".into(),
            status: "en_corte".into(),
            priority: "normal".into(),
            measurements: camisa,
            fabric_details: "Lino italiano color marfil, botones de nácar.".into(),
            estimated_cost: 45.0,
            deposit_paid: 20.0,
            due_date: day(4),
            delivery_date: None,
            created_at: created(-5),
            notes: vec![
                OrderNote {
                    id: new_id(),
                    timestamp: created(-5),
                    stage: "recepcion".into(),
                    content: "Se tomaron medidas completas; cuello de 41 cm.".into(),
                },
                OrderNote {
                    id: new_id(),
                    timestamp: created(-2),
                    stage: "corte".into(),
                    content: "Patrón trazado y piezas delanteras cortadas.".into(),
                },
            ],
            ..Default::default()
        },
        Order {
            id: new_id(),
            customer_id: carlos.id.clone(),
            piece_name: "Pantalón baggy de drill negro".into(),
            garment_type: "pantalon".into(),
            style_name: "Baggy".into(),
            status: "en_confeccion".into(),
            priority: "urgente".into(),
            measurements: baggy,
            fabric_details: "Drill pesado 12 oz, tiro alto con pinzas.".into(),
            estimated_cost: 38.0,
            deposit_paid: 15.0,
            due_date: day(2),
            delivery_date: None,
            created_at: created(-7),
            notes: vec![OrderNote {
                id: new_id(),
                timestamp: created(-3),
                stage: "costura".into(),
                content: "Cremallera instalada y unión de entrepierna lista.".into(),
            }],
            ..Default::default()
        },
        Order {
            id: new_id(),
            customer_id: ana.id.clone(),
            piece_name: "Vestido de cóctel línea A".into(),
            garment_type: "vestido".into(),
            style_name: "Línea A".into(),
            status: "prueba_ajuste".into(),
            priority: "prioritaria".into(),
            measurements: vestido,
            fabric_details: "Terciopelo color ciruela con forro al tono.".into(),
            estimated_cost: 85.0,
            deposit_paid: 40.0,
            due_date: day(6),
            delivery_date: None,
            created_at: created(-9),
            notes: vec![OrderNote {
                id: new_id(),
                timestamp: created(-1),
                stage: "prueba".into(),
                content: "Primera prueba con tacones; ajustar 2 cm en cintura.".into(),
            }],
            ..Default::default()
        },
        Order {
            id: new_id(),
            customer_id: maria.id.clone(),
            piece_name: "Bolso tote de lona y cuero".into(),
            garment_type: "bolso".into(),
            style_name: "Tote".into(),
            status: "pendiente".into(),
            priority: "normal".into(),
            measurements: tote,
            fabric_details: "Lona encerada cruda con asas de cuero tostado.".into(),
            estimated_cost: 30.0,
            deposit_paid: 10.0,
            due_date: day(12),
            delivery_date: None,
            created_at: created(-2),
            notes: vec![],
            ..Default::default()
        },
        Order {
            id: new_id(),
            customer_id: colegio.id.clone(),
            piece_name: "Camisa escolar manga corta".into(),
            garment_type: "camisa".into(),
            style_name: "Clásico".into(),
            status: "terminado".into(),
            priority: "normal".into(),
            measurements: Measurements {
                kind: "camisa".into(),
                values: BTreeMap::from([
                    ("pecho".to_string(), 102.0),
                    ("cintura".to_string(), 90.0),
                    ("cadera".to_string(), 104.0),
                    ("hombros".to_string(), 47.0),
                    ("largoManga".to_string(), 26.0),
                    ("anchoManga".to_string(), 36.0),
                    ("largoTotal".to_string(), 72.0),
                    ("cuello".to_string(), 42.0),
                ]),
                options: BTreeMap::from([
                    ("cuello".to_string(), "Clásico".to_string()),
                    ("manga".to_string(), "Corta".to_string()),
                ]),
                notes: String::new(),
            },
            fabric_details: "Popelina azul marino, lote de 12 unidades.".into(),
            estimated_cost: 18.0,
            deposit_paid: 18.0,
            due_date: day(-1),
            delivery_date: None,
            created_at: created(-20),
            notes: vec![OrderNote {
                id: new_id(),
                timestamp: created(-2),
                stage: "terminado".into(),
                content: "Lote planchado y empacado para entrega.".into(),
            }],
            ..Default::default()
        },
        Order {
            id: new_id(),
            customer_id: carlos.id.clone(),
            piece_name: "Pantalón recto azul marino".into(),
            garment_type: "pantalon".into(),
            style_name: "Recto".into(),
            status: "entregado".into(),
            priority: "normal".into(),
            measurements: Measurements {
                kind: "pantalon".into(),
                values: BTreeMap::from([
                    ("cintura".to_string(), 86.0),
                    ("cadera".to_string(), 100.0),
                    ("muslo".to_string(), 58.0),
                    ("rodilla".to_string(), 44.0),
                    ("bota".to_string(), 38.0),
                    ("tiro".to_string(), 28.0),
                    ("largoTotal".to_string(), 104.0),
                ]),
                options: BTreeMap::from([("corte".to_string(), "Recto".to_string())]),
                notes: String::new(),
            },
            fabric_details: "Casimir azul marino.".into(),
            estimated_cost: 42.0,
            deposit_paid: 42.0,
            due_date: day(-6),
            delivery_date: Some(day(-5)),
            created_at: created(-30),
            notes: vec![OrderNote {
                id: new_id(),
                timestamp: created(-5),
                stage: "entrega".into(),
                content: "Entregado y probado sin ajustes.".into(),
            }],
            ..Default::default()
        },
    ];

    let reminders = vec![
        Reminder {
            id: new_id(),
            title: "Prueba del vestido con Ana Lucía".into(),
            description: "Cita a las 4:30 p. m. para revisar cintura y ruedo.".into(),
            due_date: format!("{}T16:30", day(1)),
            priority: "prioritaria".into(),
            completed: false,
            customer_id: Some(ana.id.clone()),
            order_id: Some(ordenes[2].id.clone()),
            created_at: created(-2),
        },
        Reminder {
            id: new_id(),
            title: "Comprar cierre invisible color ciruela".into(),
            description: "Mercería del centro; también forro de satén.".into(),
            due_date: format!("{}T10:00", day(2)),
            priority: "normal".into(),
            completed: false,
            customer_id: Some(ana.id.clone()),
            order_id: None,
            created_at: created(-2),
        },
        Reminder {
            id: new_id(),
            title: "Entregar pantalón baggy".into(),
            description: "Revisar largo de bota antes de empacar.".into(),
            due_date: format!("{}T17:00", day(2)),
            priority: "urgente".into(),
            completed: false,
            customer_id: Some(carlos.id.clone()),
            order_id: Some(ordenes[1].id.clone()),
            created_at: created(-1),
        },
        Reminder {
            id: new_id(),
            title: "Llamar a María José por medidas del bolso".into(),
            description: "Confirmar largo de correa y tipo de cierre.".into(),
            due_date: format!("{}T15:00", day(0)),
            priority: "normal".into(),
            completed: false,
            customer_id: Some(maria.id.clone()),
            order_id: None,
            created_at: created(-1),
        },
        Reminder {
            id: new_id(),
            title: "Mantenimiento de la overlock".into(),
            description: "Limpiar pelusa, lubricar y cambiar aguja n.º 90.".into(),
            due_date: format!("{}T18:00", day(7)),
            priority: "normal".into(),
            completed: false,
            customer_id: None,
            order_id: None,
            created_at: created(-3),
        },
        Reminder {
            id: new_id(),
            title: "Cortar tela de camisas escolares".into(),
            description: "Lote de 12 camisas para el Colegio San José.".into(),
            due_date: format!("{}T09:00", day(-2)),
            priority: "normal".into(),
            completed: true,
            customer_id: Some(colegio.id.clone()),
            order_id: Some(ordenes[4].id.clone()),
            created_at: created(-8),
        },
    ];

    AppData {
        version: DATA_VERSION,
        settings: Settings {
            workshop_name: "Taller Aurora".into(),
            owner_name: "Elena".into(),
            currency: "$".into(),
            default_due_days: 7,
            backup_dir: String::new(),
            language: "es".into(),
        },
        customers: vec![maria, carlos, ana, colegio],
        orders: ordenes,
        reminders,
        supplies: vec![
            Supply {
                id: "s1".into(),
                name: "Tela de algodón".into(),
                category: "tela".into(),
                unit: "yarda".into(),
                stock: 15.0,
                min_stock: 5.0,
                unit_cost: 8.5,
                supplier: "Mercería del centro".into(),
                notes: "Tela blanca 100% algodón".into(),
                created_at: now_iso(),
            },
            Supply {
                id: "s2".into(),
                name: "Hilo de polyester".into(),
                category: "hilo".into(),
                unit: "cono".into(),
                stock: 10.0,
                min_stock: 3.0,
                unit_cost: 2.0,
                supplier: "Proveedor textil".into(),
                notes: "Cono negro 40/3".into(),
                created_at: now_iso(),
            },
        ],
        movements: vec![
            SupplyMovement {
                id: new_id(),
                supply_id: "s1".into(),
                timestamp: now_iso(),
                delta: 15.0,
                kind: "entrada".into(),
                reason: "Existencia inicial".into(),
                order_id: None,
            },
        ],
    }
}
