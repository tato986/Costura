use crate::image_cache::{CachedImageEntry, ImageCacheStats, ImageDiskCache};
use crate::model::*;
use crate::store::BackupInfo;
use crate::xlsx::{write_accounting_xlsx, AccountingMonthSummary};
use crate::AppState;
use std::collections::BTreeMap;
use std::sync::Mutex;
use tauri::State;

type Db<'a> = State<'a, Mutex<AppState>>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionAuthState {
    Unauthenticated,
    OfflineLocalAuthorized,
    Authenticated { uid: String, email: Option<String> },
}

impl Default for SessionAuthState {
    fn default() -> Self {
        Self::Unauthenticated
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusDto {
    pub authorized: bool,
    pub mode: String, // "unauthenticated", "offline_local", "authenticated"
    pub active_uid: Option<String>,
    pub email: Option<String>,
    pub cloud_configured: bool,
}

pub fn resolve_initial_session_auth(data: &mut AppData) -> SessionAuthState {
    if let Ok(Some(raw)) = read_keyring_secret(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(uid) = parsed
                .get("uid")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|u| !u.is_empty())
            {
                let email = parsed
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .map(str::to_string);
                data.firebase.active_uid = Some(uid.to_string());
                return SessionAuthState::Authenticated {
                    uid: uid.to_string(),
                    email,
                };
            }
        }
    }
    data.firebase.active_uid = None;
    SessionAuthState::Unauthenticated
}

pub fn ensure_session_authorized(app: &AppState) -> Result<(), String> {
    match app.session_auth {
        SessionAuthState::Authenticated { ref uid, .. } if !uid.trim().is_empty() => Ok(()),
        SessionAuthState::OfflineLocalAuthorized => Ok(()),
        _ => Err(
            "No autorizado: debes iniciar sesión o activar el modo local sin conexión para acceder a los datos del taller."
                .to_string(),
        ),
    }
}

pub fn build_session_status(app: &AppState) -> SessionStatusDto {
    let cloud_configured = app.data.firebase.is_cloud_configured();
    match &app.session_auth {
        SessionAuthState::Authenticated { uid, email } => SessionStatusDto {
            authorized: true,
            mode: "authenticated".to_string(),
            active_uid: Some(uid.clone()),
            email: email.clone(),
            cloud_configured,
        },
        SessionAuthState::OfflineLocalAuthorized => SessionStatusDto {
            authorized: true,
            mode: "offline_local".to_string(),
            active_uid: None,
            email: None,
            cloud_configured,
        },
        SessionAuthState::Unauthenticated => SessionStatusDto {
            authorized: false,
            mode: "unauthenticated".to_string(),
            active_uid: None,
            email: None,
            cloud_configured,
        },
    }
}

fn lock<'a>(state: &'a Db<'_>) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    state
        .lock()
        .map_err(|_| "El estado de la aplicación está bloqueado. Reinicia Costura.".to_string())
}

fn lock_authorized<'a>(state: &'a Db<'_>) -> Result<std::sync::MutexGuard<'a, AppState>, String> {
    let guard = lock(state)?;
    ensure_session_authorized(&guard)?;
    Ok(guard)
}

pub fn mutate_app_authorized<R>(
    app: &mut AppState,
    change: impl FnOnce(&mut AppState) -> Result<R, String>,
) -> Result<(R, AppData), String> {
    ensure_session_authorized(app)?;
    let result = change(app)?;
    app.data.refresh_customer_aggregates();
    app.store
        .save(&app.data)
        .map_err(|e| format!("No se pudieron guardar los cambios: {e}"))?;
    let snapshot = app.data.clone();
    Ok((result, snapshot))
}

fn mutate<R>(
    state: &Db<'_>,
    change: impl FnOnce(&mut AppState) -> Result<R, String>,
) -> Result<(R, AppData), String> {
    let mut guard = lock(state)?;
    mutate_app_authorized(&mut guard, change)
}

fn save_only(state: &Db<'_>, data: &AppData) -> Result<AppData, String> {
    let mut guard = lock_authorized(state)?;
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
    session: SessionStatusDto,
}

#[tauri::command]
pub fn get_session_status(state: Db<'_>) -> Result<SessionStatusDto, String> {
    let guard = lock(&state)?;
    Ok(build_session_status(&guard))
}

#[tauri::command]
pub fn authorize_offline_local_session(state: Db<'_>) -> Result<SessionStatusDto, String> {
    let mut guard = lock(&state)?;
    guard.session_auth = SessionAuthState::OfflineLocalAuthorized;
    guard.data.firebase.active_uid = None;
    let _ = guard.store.save(&guard.data);
    Ok(build_session_status(&guard))
}

#[tauri::command]
pub fn get_state(state: Db<'_>) -> Result<AppStateDto, String> {
    let guard = lock_authorized(&state)?;
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
        session: build_session_status(&guard),
    })
}

#[tauri::command]
pub fn dismiss_warning(state: Db<'_>) -> Result<(), String> {
    let mut guard = lock_authorized(&state)?;
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
                existing.updated_at = now_iso();
            }
            None => {
                let now = now_iso();
                app.data.customers.push(Customer {
                    id: new_id(),
                    created_at: now.clone(),
                    updated_at: now,
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
        let customer_name = app
            .data
            .customers
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.full_name.clone())
            .ok_or_else(|| "El cliente ya no existe.".to_string())?;
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
        record_audit(
            &mut app.data.audit_log,
            "customer.delete",
            "clientes",
            &id,
            format!("Cliente eliminado: {customer_name}"),
        );
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Pedidos ──────────────────────────────

fn record_audit(
    audit_log: &mut Vec<AuditEvent>,
    action: &str,
    entity_type: &str,
    entity_id: &str,
    summary: String,
) {
    audit_log.push(AuditEvent {
        id: new_id(),
        timestamp: now_iso(),
        action: action.to_string(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        summary,
    });
    if audit_log.len() > 2000 {
        let excess = audit_log.len() - 2000;
        audit_log.drain(0..excess);
    }
}

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
    order_revision: u64,
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
        let det_id = deterministic_movement_id(order_id, order_revision, &supply_id);
        if app.data.movements.iter().any(|m| m.id == det_id) {
            continue;
        }
        if let Some(supply) = app.data.supplies.iter_mut().find(|s| s.id == supply_id) {
            supply.stock = round_money(supply.stock - delta);
            supply.updated_at = now_iso();
        }
        app.data.movements.push(SupplyMovement {
            id: det_id,
            supply_id,
            timestamp: now_iso(),
            delta: round_money(-delta),
            kind: if delta > 0.0 { "salida" } else { "devolucion" }.to_string(),
            reason: format!("{prefix}: {piece_name}"),
            order_id: Some(order_id.to_string()).filter(|id| !id.is_empty()),
            order_revision: Some(order_revision),
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

                let next_revision = previous.revision.saturating_add(1);
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
                    existing.revision = next_revision;
                    existing.updated_at = now_iso();

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
                    next_revision,
                    &order.piece_name,
                    "Pedido editado",
                )?;
            }
            None => {
                let now = now_iso();
                let mut created = Order {
                    id: new_id(),
                    revision: 1,
                    created_at: now.clone(),
                    updated_at: now,
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
                apply_supply_deltas(app, None, &supplies, &order_id, 1, &piece_name, "Pedido creado")?;
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

        let del_revision = order.revision.saturating_add(1);
        apply_supply_deltas(
            app,
            Some(&order),
            &[],
            &order.id,
            del_revision,
            &order.piece_name,
            "Pedido eliminado",
        )?;

        app.data.orders.retain(|o| o.id != id);
        app.data
            .reminders
            .retain(|r| r.order_id.as_deref() != Some(id.as_str()));
        record_audit(
            &mut app.data.audit_log,
            "order.delete",
            "pedidos",
            &id,
            format!("Pedido eliminado: {}", order.piece_name),
        );
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
                existing.due_time = reminder.due_time;
                existing.priority = reminder.priority;
                existing.customer_id = reminder.customer_id;
                existing.order_id = reminder.order_id;
                existing.updated_at = now_iso();
            }
            None => {
                let now = now_iso();
                app.data.reminders.push(Reminder {
                    id: new_id(),
                    created_at: now.clone(),
                    updated_at: now,
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
                        supply_id: supply_id.clone(),
                        timestamp: now_iso(),
                        delta: round_money(delta),
                        kind: "ajuste".into(),
                        reason: "Ajuste manual de existencias".into(),
                        order_id: None,
                        order_revision: None,
                    });
                    record_audit(
                        &mut app.data.audit_log,
                        "supply.adjust",
                        "insumos",
                        &supply_id,
                        format!("Ajuste manual de stock: delta {:+.2}", round_money(delta)),
                    );
                }
            }
            None => {
                let now = now_iso();
                let created = Supply {
                    id: new_id(),
                    created_at: now.clone(),
                    updated_at: now,
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
                        order_revision: None,
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
            order_revision: None,
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
    tax_rate_percent: Option<f64>,
    tax_label: Option<String>,
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

    if let Some(rate) = tax_rate_percent {
        if !rate.is_finite() || !(0.0..=100.0).contains(&rate) {
            return Err("La tasa de impuesto debe estar entre 0 % y 100 %.".to_string());
        }
    }

    let (_, data) = mutate(&state, |app| {
        app.data.settings.workshop_name = name;
        app.data.settings.owner_name = owner_name.trim().to_string();
        app.data.settings.currency = currency;
        app.data.settings.default_due_days = default_due_days;
        app.data.settings.language = lang;
        if let Some(rate) = tax_rate_percent {
            app.data.settings.tax_rate_percent = round_money(rate);
        }
        if let Some(lbl) = tax_label {
            let trimmed = lbl.trim();
            if trimmed.chars().count() <= 20 {
                app.data.settings.tax_label = trimmed.to_string();
            }
        }
        app.data.settings.updated_at = now_iso();
        Ok(())
    })?;
    Ok(data)
}

#[tauri::command]
pub fn get_firestore_export_preview(
    state: Db<'_>,
    uid: Option<String>,
) -> Result<FirestoreWorkshopSnapshot, String> {
    let guard = lock_authorized(&state)?;
    let target_uid = uid
        .or_else(|| guard.data.firebase.active_uid.clone())
        .unwrap_or_else(|| "PENDIENTE_UID_FIREBASE".to_string());
    Ok(guard.data.to_firestore_snapshot(&target_uid))
}

#[tauri::command]
pub fn get_firestore_materialized_snapshot(
    state: Db<'_>,
    uid: Option<String>,
) -> Result<FirestoreWorkshopSnapshot, String> {
    let guard = lock_authorized(&state)?;
    let target_uid = uid
        .filter(|u| !u.trim().is_empty())
        .or_else(|| guard.data.firebase.active_uid.clone())
        .unwrap_or_else(|| "PENDIENTE_UID_FIREBASE".to_string());
    Ok(guard.data.to_materialized_firestore_snapshot(&target_uid))
}

#[tauri::command]
pub fn get_firebase_env_config(state: Db<'_>) -> Result<FirebaseConnectionSlots, String> {
    let mut guard = lock(&state)?;
    let AppState {
        ref store,
        ref mut data,
        ..
    } = *guard;
    store.hydrate_firebase_from_env(data);
    Ok(data.firebase.clone())
}

const GOOGLE_OAUTH_REDIRECT_PORT: u16 = 43821;

fn percent_encode_oauth(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn open_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).spawn();

    status
        .map(|_| ())
        .map_err(|e| format!("No se pudo abrir el navegador del sistema: {e}"))
}

fn parse_form_param(raw: &str, key: &str) -> Option<String> {
    let cleaned = raw.trim().trim_start_matches('#').trim_start_matches('?');
    for pair in cleaned.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[tauri::command]
pub async fn google_oauth_desktop_login(state: Db<'_>) -> Result<String, String> {
    let client_id = {
        let mut guard = lock(&state)?;
        let AppState {
            ref store,
            ref mut data,
            ..
        } = *guard;
        store.hydrate_firebase_from_env(data);
        data.firebase.google_web_client_id.trim().to_string()
    };

    if client_id.is_empty() || client_id.starts_with("REEMPLAZAR_") {
        return Err(
            "Falta configurar COSTURA_GOOGLE_WEB_CLIENT_ID en tu archivo .env (ID de cliente Web OAuth 2.0 de Google Cloud Console)."
                .to_string(),
        );
    }

    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};

        let addr = format!("127.0.0.1:{GOOGLE_OAUTH_REDIRECT_PORT}");
        let listener = TcpListener::bind(&addr).map_err(|e| {
            format!("No se pudo abrir el puerto local {GOOGLE_OAUTH_REDIRECT_PORT} para Google Sign-In: {e}")
        })?;
        let _ = listener.set_nonblocking(true);

        let redirect_uri = format!("http://127.0.0.1:{GOOGLE_OAUTH_REDIRECT_PORT}/callback");
        let nonce = new_id();
        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=id_token&scope=openid%20email%20profile&nonce={}&prompt=select_account",
            percent_encode_oauth(&client_id),
            percent_encode_oauth(&redirect_uri),
            percent_encode_oauth(&nonce),
        );

        open_system_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(180);
        let callback_html = r#"<!doctype html>
<html lang="es"><head><meta charset="utf-8"><title>Taller Costura — Google</title>
<style>body{font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0;background:#f7f5f2;color:#222}
.box{background:#fff;padding:28px 32px;border-radius:12px;box-shadow:0 4px 20px rgba(0,0,0,.08);text-align:center;max-width:420px}</style>
</head><body><div class="box"><h2 id="t">Completando inicio de sesión…</h2><p id="m">Conectando con Taller Costura.</p></div>
<script>
(function(){
  var payload = window.location.hash || window.location.search || "";
  fetch("/token", { method: "POST", headers: { "Content-Type": "text/plain" }, body: payload })
    .then(function(){
      if (payload.indexOf("id_token=") !== -1) {
        document.getElementById("t").textContent = "¡Sesión iniciada!";
        document.getElementById("m").textContent = "Ya puedes cerrar esta pestaña y volver a Taller Costura.";
      } else {
        document.getElementById("t").textContent = "No se completó el acceso";
        document.getElementById("m").textContent = "Vuelve a Taller Costura para ver el detalle.";
      }
    })
    .catch(function(){
      document.getElementById("t").textContent = "Error de conexión";
      document.getElementById("m").textContent = "Vuelve a intentarlo desde Taller Costura.";
    });
})();
</script></body></html>"#;

        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut buf = [0u8; 16384];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        continue;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let first_line = req.lines().next().unwrap_or("");

                    if first_line.starts_with("GET /callback") {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            callback_html.len(),
                            callback_html
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();
                    } else if first_line.starts_with("POST /token") {
                        let body = req.split("\r\n\r\n").nth(1).unwrap_or("");
                        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.flush();

                        if let Some(id_token) = parse_form_param(body, "id_token") {
                            if !id_token.is_empty() {
                                return Ok(id_token);
                            }
                        }
                        if let Some(err) = parse_form_param(body, "error") {
                            return Err(format!("Google canceló o rechazó el acceso ({err})."));
                        }
                        return Err("Google no devolvió un token de sesión válido.".to_string());
                    } else {
                        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(80));
                }
                Err(e) => return Err(format!("Error escuchando respuesta de Google: {e}")),
            }
        }

        Err("Se agotó el tiempo de espera para iniciar sesión con Google. Intenta de nuevo.".to_string())
    })
    .await
    .map_err(|e| format!("Error interno al esperar respuesta de Google: {e}"))?
}

// ────────────────────────────── Custodia de Sesión en SO (ISO/IEC 27002 5.17 — keyring) ──────────────────────────────

pub const KEYRING_SERVICE: &str = "JLRDev.TallerCostura";
pub const KEYRING_USER: &str = "firebase_session_token";

pub fn save_keyring_secret(service: &str, user: &str, secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("No se pudo acceder al llavero del sistema ({service}): {e}"))?;
    entry
        .set_password(secret)
        .map_err(|e| format!("No se pudo guardar la credencial en el llavero del sistema: {e}"))
}

pub fn read_keyring_secret(service: &str, user: &str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("No se pudo acceder al llavero del sistema ({service}): {e}"))?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("No se pudo leer la credencial del llavero: {e}")),
    }
}

pub fn delete_keyring_secret(service: &str, user: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("No se pudo acceder al llavero del sistema ({service}): {e}"))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("No se pudo eliminar la credencial del llavero: {e}")),
    }
}

pub fn apply_save_secure_session_token(
    app: &mut AppState,
    uid: &str,
    refresh_token: &str,
    email: Option<&str>,
    keyring_service: &str,
    keyring_user: &str,
) -> Result<AppData, String> {
    let clean_uid = uid.trim().to_string();
    if clean_uid.is_empty() {
        return Err("El UID de sesión no puede estar vacío.".into());
    }
    let clean_token = refresh_token.trim();
    if clean_token.is_empty() {
        return Err("El token de sesión no puede estar vacío.".into());
    }
    let clean_email = email
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string);

    let payload = serde_json::json!({
        "uid": clean_uid,
        "refreshToken": clean_token,
        "email": clean_email,
        "savedAt": now_iso(),
    })
    .to_string();

    let _ = save_keyring_secret(keyring_service, keyring_user, &payload);

    app.session_auth = SessionAuthState::Authenticated {
        uid: clean_uid.clone(),
        email: clean_email,
    };
    app.data.firebase.active_uid = Some(clean_uid);
    app.data.refresh_customer_aggregates();
    app.store
        .save(&app.data)
        .map_err(|e| format!("No se pudieron guardar los cambios: {e}"))?;
    Ok(app.data.clone())
}

#[tauri::command]
pub fn save_secure_session_token(
    state: Db<'_>,
    uid: String,
    refresh_token: String,
    email: Option<String>,
) -> Result<AppData, String> {
    let mut guard = lock(&state)?;
    apply_save_secure_session_token(
        &mut guard,
        &uid,
        &refresh_token,
        email.as_deref(),
        KEYRING_SERVICE,
        KEYRING_USER,
    )
}

#[tauri::command]
pub fn get_secure_session_token() -> Result<Option<String>, String> {
    Ok(read_keyring_secret(KEYRING_SERVICE, KEYRING_USER).unwrap_or(None))
}

pub fn apply_clear_secure_session_token(
    app: &mut AppState,
    keyring_service: &str,
    keyring_user: &str,
) -> Result<SessionStatusDto, String> {
    let _ = delete_keyring_secret(keyring_service, keyring_user);
    app.session_auth = SessionAuthState::Unauthenticated;
    app.data.firebase.active_uid = None;
    app.store
        .save(&app.data)
        .map_err(|e| format!("No se pudieron guardar los cambios: {e}"))?;
    Ok(build_session_status(app))
}

#[tauri::command]
pub fn clear_secure_session_token(state: Db<'_>) -> Result<SessionStatusDto, String> {
    let mut guard = lock(&state)?;
    apply_clear_secure_session_token(&mut guard, KEYRING_SERVICE, KEYRING_USER)
}

/// Sincroniza el espejo local (`costura.json` + respaldo diario) cuando Cloud Firestore
/// emite un snapshot remoto actualizado.
#[tauri::command]
pub fn sync_mirror_state(state: Db<'_>, incoming: AppData) -> Result<AppData, String> {
    let (_, data) = mutate(&state, |app| {
        let current_fb = app.data.firebase.clone();
        let current_backup_dir = app.data.settings.backup_dir.clone();
        app.data = incoming;
        app.data.version = DATA_VERSION;
        app.data.firebase = current_fb;
        if app.data.settings.backup_dir.is_empty() && !current_backup_dir.is_empty() {
            app.data.settings.backup_dir = current_backup_dir;
        }
        Ok(())
    })?;
    let guard = lock(&state)?;
    let _ = guard.store.ensure_daily_backup(&guard.data);
    Ok(data)
}

// ────────────────────────────── Exportación Contable .XLSX (rust_xlsxwriter) ──────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportXlsxResult {
    pub path: String,
    pub summary: AccountingMonthSummary,
}

#[tauri::command]
pub fn export_accounting_xlsx(
    state: Db<'_>,
    year: i32,
    month: u32,
    path: Option<String>,
) -> Result<ExportXlsxResult, String> {
    let guard = lock_authorized(&state)?;
    let out_path = match path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        Some(custom) => std::path::PathBuf::from(custom),
        None => {
            let period_tag = if year > 0 && (1..=12).contains(&month) {
                format!("{year:04}_{month:02}")
            } else {
                "HISTORICO".to_string()
            };
            guard
                .store
                .dir()
                .join("reportes")
                .join(format!("Costura_Contabilidad_{period_tag}.xlsx"))
        }
    };
    let summary = write_accounting_xlsx(&guard.data, year, month, &out_path)?;
    Ok(ExportXlsxResult {
        path: out_path.to_string_lossy().to_string(),
        summary,
    })
}

// ────────────────────────────── Caché Local de Imágenes LRU (150 MB / 85 % = 127.5 MB) ──────────────────────────────

#[tauri::command]
pub fn cache_order_image(
    state: Db<'_>,
    order_id: String,
    image_id: String,
    payload: String,
    pending_upload: bool,
) -> Result<(CachedImageEntry, ImageCacheStats), String> {
    let guard = lock_authorized(&state)?;
    let cache = ImageDiskCache::new(guard.store.dir())?;
    cache.put_image(&order_id, &image_id, payload.as_bytes(), pending_upload, None)
}

#[tauri::command]
pub fn mark_cached_image_uploaded(
    state: Db<'_>,
    order_id: String,
    image_id: String,
) -> Result<ImageCacheStats, String> {
    let guard = lock_authorized(&state)?;
    let cache = ImageDiskCache::new(guard.store.dir())?;
    let _ = cache.touch_image(&order_id, &image_id, None)?;
    let _ = cache.mark_uploaded(&order_id, &image_id)?;
    Ok(cache.stats())
}

#[tauri::command]
pub fn get_image_cache_stats(state: Db<'_>) -> Result<ImageCacheStats, String> {
    let guard = lock_authorized(&state)?;
    let cache = ImageDiskCache::new(guard.store.dir())?;
    Ok(cache.stats())
}

#[tauri::command]
pub fn purge_image_cache_lru(state: Db<'_>) -> Result<ImageCacheStats, String> {
    let guard = lock_authorized(&state)?;
    let cache = ImageDiskCache::new(guard.store.dir())?;
    cache.purge_if_needed()
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FirebaseSlotsInput {
    pub mode: String,
    pub use_emulators: bool,
    pub project_id: String,
    pub api_key: String,
    pub auth_domain: String,
    pub storage_bucket: String,
    pub messaging_sender_id: String,
    pub app_id: String,
    pub google_web_client_id: String,
    pub active_uid: Option<String>,
    pub emulator_auth_host: Option<String>,
    pub emulator_firestore_host: Option<String>,
    pub emulator_storage_host: Option<String>,
}

impl Default for FirebaseSlotsInput {
    fn default() -> Self {
        let def = FirebaseConnectionSlots::default();
        Self {
            mode: def.mode,
            use_emulators: def.use_emulators,
            project_id: def.project_id,
            api_key: def.api_key,
            auth_domain: def.auth_domain,
            storage_bucket: def.storage_bucket,
            messaging_sender_id: def.messaging_sender_id,
            app_id: def.app_id,
            google_web_client_id: def.google_web_client_id,
            active_uid: def.active_uid,
            emulator_auth_host: Some(def.emulator_auth_host),
            emulator_firestore_host: Some(def.emulator_firestore_host),
            emulator_storage_host: Some(def.emulator_storage_host),
        }
    }
}

#[tauri::command]
pub fn save_firebase_slots(state: Db<'_>, input: FirebaseSlotsInput) -> Result<AppData, String> {
    let mode = match input.mode.trim() {
        "emulator" | "cloud" => input.mode.trim().to_string(),
        _ => "offline_local".to_string(),
    };
    let (_, data) = mutate(&state, |app| {
        app.data.firebase.mode = mode;
        app.data.firebase.use_emulators = input.use_emulators;
        app.data.firebase.project_id = input.project_id.trim().to_string();
        app.data.firebase.api_key = input.api_key.trim().to_string();
        app.data.firebase.auth_domain = input.auth_domain.trim().to_string();
        app.data.firebase.storage_bucket = input.storage_bucket.trim().to_string();
        app.data.firebase.messaging_sender_id = input.messaging_sender_id.trim().to_string();
        app.data.firebase.app_id = input.app_id.trim().to_string();
        app.data.firebase.google_web_client_id = input.google_web_client_id.trim().to_string();
        app.data.firebase.active_uid = input
            .active_uid
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        if let Some(h) = input.emulator_auth_host.filter(|s| !s.trim().is_empty()) {
            app.data.firebase.emulator_auth_host = h.trim().to_string();
        }
        if let Some(h) = input.emulator_firestore_host.filter(|s| !s.trim().is_empty()) {
            app.data.firebase.emulator_firestore_host = h.trim().to_string();
        }
        if let Some(h) = input.emulator_storage_host.filter(|s| !s.trim().is_empty()) {
            app.data.firebase.emulator_storage_host = h.trim().to_string();
        }
        Ok(())
    })?;
    Ok(data)
}

// ────────────────────────────── Respaldos ──────────────────────────────

#[tauri::command]
pub fn create_backup(state: Db<'_>) -> Result<BackupInfo, String> {
    let guard = lock_authorized(&state)?;
    guard.store.create_backup(&guard.data, false)
}

#[tauri::command]
pub fn list_backups(state: Db<'_>) -> Result<Vec<BackupInfo>, String> {
    let guard = lock_authorized(&state)?;
    Ok(guard.store.list_backups(&guard.data))
}

#[tauri::command]
pub fn restore_backup(state: Db<'_>, file: String) -> Result<AppData, String> {
    let restored = {
        let guard = lock_authorized(&state)?;
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
    let guard = lock_authorized(&state)?;
    let path = match which.as_str() {
        "backups" => guard.store.resolve_backups_dir(&guard.data),
        "reports" => {
            let reports_dir = guard.store.dir().join("reportes");
            let _ = std::fs::create_dir_all(&reports_dir);
            reports_dir
        }
        _ => guard.store.dir().to_path_buf(),
    };
    drop(guard);

    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");

    cmd.arg(path)
        .spawn()
        .map_err(|e| format!("No se pudo abrir la carpeta: {e}"))?;
    Ok(())
}

// ────────────────────────────── Mantenimiento ──────────────────────────────

#[tauri::command]
pub fn reset_all(state: Db<'_>) -> Result<AppData, String> {
    let mut guard = lock_authorized(&state)?;
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
    let mut guard = lock_authorized(&state)?;
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
        updated_at: created(-24),
        ..Default::default()
    };
    let carlos = Customer {
        id: new_id(),
        full_name: "Carlos Ernesto Mejía".into(),
        phone: "+503 6123-4455".into(),
        email: "carlos.mejia@example.com".into(),
        address: "Col. Layco, San Salvador".into(),
        notes: "Camisas formales y pantalones rectos. Prefiere tonos oscuros.".into(),
        created_at: created(-18),
        updated_at: created(-18),
        ..Default::default()
    };
    let ana = Customer {
        id: new_id(),
        full_name: "Ana Lucía Hernández".into(),
        phone: "+503 7011-9090".into(),
        email: "ana.hernandez@example.com".into(),
        address: "Santa Tecla, La Libertad".into(),
        notes: "Vestidos para eventos. Le gusta el acabado a mano en el ruedo.".into(),
        created_at: created(-11),
        updated_at: created(-11),
        ..Default::default()
    };
    let colegio = Customer {
        id: new_id(),
        full_name: "Colegio San José (uniformes)".into(),
        phone: "+503 2222-3030".into(),
        email: "compras@colegiosanjose.example".into(),
        address: "Av. Olímpica, San Salvador".into(),
        notes: "Pedidos por lote de uniformes. Facturar a nombre de la institución.".into(),
        created_at: created(-40),
        updated_at: created(-40),
        ..Default::default()
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
            due_date: day(1),
            due_time: Some("16:30".into()),
            priority: "prioritaria".into(),
            completed: false,
            customer_id: Some(ana.id.clone()),
            order_id: Some(ordenes[2].id.clone()),
            created_at: created(-2),
            updated_at: created(-2),
        },
        Reminder {
            id: new_id(),
            title: "Comprar cierre invisible color ciruela".into(),
            description: "Mercería del centro; también forro de satén.".into(),
            due_date: day(2),
            due_time: Some("10:00".into()),
            priority: "normal".into(),
            completed: false,
            customer_id: Some(ana.id.clone()),
            order_id: None,
            created_at: created(-2),
            updated_at: created(-2),
        },
        Reminder {
            id: new_id(),
            title: "Entregar pantalón baggy".into(),
            description: "Revisar largo de bota antes de empacar.".into(),
            due_date: day(2),
            due_time: Some("17:00".into()),
            priority: "urgente".into(),
            completed: false,
            customer_id: Some(carlos.id.clone()),
            order_id: Some(ordenes[1].id.clone()),
            created_at: created(-1),
            updated_at: created(-1),
        },
        Reminder {
            id: new_id(),
            title: "Llamar a María José por medidas del bolso".into(),
            description: "Confirmar largo de correa y tipo de cierre.".into(),
            due_date: day(0),
            due_time: Some("15:00".into()),
            priority: "normal".into(),
            completed: false,
            customer_id: Some(maria.id.clone()),
            order_id: None,
            created_at: created(-1),
            updated_at: created(-1),
        },
        Reminder {
            id: new_id(),
            title: "Mantenimiento de la overlock".into(),
            description: "Limpiar pelusa, lubricar y cambiar aguja n.º 90.".into(),
            due_date: day(7),
            due_time: Some("18:00".into()),
            priority: "normal".into(),
            completed: false,
            customer_id: None,
            order_id: None,
            created_at: created(-3),
            updated_at: created(-3),
        },
        Reminder {
            id: new_id(),
            title: "Cortar tela de camisas escolares".into(),
            description: "Lote de 12 camisas para el Colegio San José.".into(),
            due_date: day(-2),
            due_time: Some("09:00".into()),
            priority: "normal".into(),
            completed: true,
            customer_id: Some(colegio.id.clone()),
            order_id: Some(ordenes[4].id.clone()),
            created_at: created(-8),
            updated_at: created(-8),
        },
    ];

    let mut demo = AppData {
        version: DATA_VERSION,
        settings: Settings {
            workshop_name: "Taller Aurora".into(),
            owner_name: "Elena".into(),
            currency: "$".into(),
            tax_rate_percent: 0.0,
            tax_label: "IVA".into(),
            default_due_days: 7,
            backup_dir: String::new(),
            language: "es".into(),
            updated_at: now_iso(),
        },
        firebase: FirebaseConnectionSlots::default(),
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
                updated_at: now_iso(),
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
                updated_at: now_iso(),
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
                order_revision: None,
            },
        ],
        audit_log: Vec::new(),
    };
    demo.refresh_customer_aggregates();
    demo
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use std::path::PathBuf;

    #[test]
    fn keyring_ciclo_guardar_leer_y_limpiar_bajo_servicio_jlrdev_tallercostura() {
        // Usamos una cuenta de prueba aislada bajo el mismo servicio `JLRDev.TallerCostura`
        let test_user = "test_firebase_session_token_ci";
        let _ = delete_keyring_secret(KEYRING_SERVICE, test_user);

        let payload = serde_json::json!({
            "uid": "uid_taller_9f69d",
            "refreshToken": "rt_test_123456",
        })
        .to_string();

        save_keyring_secret(KEYRING_SERVICE, test_user, &payload)
            .expect("Debe guardar en Windows Credential Manager / macOS Keychain");

        let read_back = read_keyring_secret(KEYRING_SERVICE, test_user)
            .expect("Debe consultar el llavero del SO sin error");
        assert_eq!(read_back.as_deref(), Some(payload.as_str()));

        delete_keyring_secret(KEYRING_SERVICE, test_user)
            .expect("Debe limpiar la credencial del llavero del SO");

        let after_clear = read_keyring_secret(KEYRING_SERVICE, test_user)
            .expect("Consulta tras borrado no debe fallar");
        assert!(after_clear.is_none());
    }

    #[test]
    fn hidrata_env_local_real_y_materializa_tablas_con_agenda_estricta() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("cmd-env-{}", uuid::Uuid::new_v4()));
        let store = Store::at(dir.clone()).unwrap();

        let mut data = demo_data();
        store.hydrate_firebase_from_env(&mut data);

        // Si existe el archivo `.env` local en la raíz del proyecto, verifica que se haya hidratado
        let root_env = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(".env");
        if root_env.exists() {
            assert_eq!(data.firebase.project_id, "tallercostura-9f69d");
            assert!(!data.firebase.api_key.starts_with("REEMPLAZAR_"));
            assert!(!data.firebase.app_id.starts_with("REEMPLAZAR_"));
        }

        let snap = data.to_materialized_firestore_snapshot("uid_verificacion_pc");
        assert_eq!(snap.root_path, "/talleres/uid_verificacion_pc");
        assert_eq!(snap.workshop_doc.version, 2);
        assert!(!snap.collections.clientes.is_empty());
        assert!(!snap.collections.pedidos.is_empty());
        assert!(!snap.collections.insumos.is_empty());
        assert!(!snap.collections.movimientos.is_empty());
        assert!(!snap.collections.agenda.is_empty());
        assert!(!snap.collections.auditoria.is_empty());

        for rem in &snap.collections.agenda {
            assert_eq!(rem.due_date.len(), 10, "dueDate debe ser YYYY-MM-DD (10 chars)");
            assert!(!rem.due_date.contains('T'));
            if let Some(ref t) = rem.due_time {
                assert_eq!(t.len(), 5, "dueTime opcional debe ser HH:mm (5 chars)");
                assert_eq!(&t[2..3], ":");
            }
        }

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn descuento_idempotente_de_insumos_evita_duplicados_por_revision() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("cmd-supply-{}", uuid::Uuid::new_v4()));
        let store = Store::at(dir.clone()).unwrap();
        let data = demo_data();
        let mut app = AppState {
            store,
            data,
            warning: None,
            session_auth: SessionAuthState::OfflineLocalAuthorized,
        };

        let initial_stock = app
            .data
            .supplies
            .iter()
            .find(|s| s.id == "s1")
            .unwrap()
            .stock;

        let usage = vec![SupplyUsage {
            supply_id: "s1".into(),
            quantity: 3.5,
            cut: String::new(),
            note: String::new(),
        }];

        // Primera aplicación para (ord_test, rev 1)
        apply_supply_deltas(
            &mut app,
            None,
            &usage,
            "ord_test",
            1,
            "Camisa lino",
            "Pedido creado",
        )
        .unwrap();

        let stock_after_first = app
            .data
            .supplies
            .iter()
            .find(|s| s.id == "s1")
            .unwrap()
            .stock;
        assert_eq!(stock_after_first, round_money(initial_stock - 3.5));

        // Segunda aplicación con el mismo (ord_test, rev 1) -> Idempotente (no vuelve a descontar)
        apply_supply_deltas(
            &mut app,
            None,
            &usage,
            "ord_test",
            1,
            "Camisa lino",
            "Pedido creado",
        )
        .unwrap();

        let stock_after_second = app
            .data
            .supplies
            .iter()
            .find(|s| s.id == "s1")
            .unwrap()
            .stock;
        assert_eq!(stock_after_second, stock_after_first);

        let det_id = deterministic_movement_id("ord_test", 1, "s1");
        let count_det = app.data.movements.iter().filter(|m| m.id == det_id).count();
        assert_eq!(count_det, 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn guardia_sesion_rechaza_operaciones_sin_autenticacion_y_permite_con_sesion_o_modo_local() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("cmd-auth-guard-{}", uuid::Uuid::new_v4()));
        let store = Store::at(dir.clone()).unwrap();
        let data = demo_data();
        let mut app = AppState {
            store,
            data,
            warning: None,
            session_auth: SessionAuthState::Unauthenticated,
        };

        // 1. Sin sesión ni modo local autorizado -> debe rechazar lectura/mutación protegida
        let status_unauth = build_session_status(&app);
        assert!(!status_unauth.authorized);
        assert_eq!(status_unauth.mode, "unauthenticated");
        assert!(ensure_session_authorized(&app).is_err());

        let unauthorized_mutation = mutate_app_authorized(&mut app, |a| {
            a.data.settings.workshop_name = "Intento No Autorizado".into();
            Ok(())
        });
        assert!(unauthorized_mutation.is_err());
        assert_ne!(app.data.settings.workshop_name, "Intento No Autorizado");

        // 2. Autorizando explícitamente el modo local sin conexión -> debe permitir operaciones
        app.session_auth = SessionAuthState::OfflineLocalAuthorized;
        let status_offline = build_session_status(&app);
        assert!(status_offline.authorized);
        assert_eq!(status_offline.mode, "offline_local");
        assert!(ensure_session_authorized(&app).is_ok());

        let offline_mutation = mutate_app_authorized(&mut app, |a| {
            a.data.settings.workshop_name = "Taller Modo Local".into();
            Ok(())
        });
        assert!(offline_mutation.is_ok());
        assert_eq!(app.data.settings.workshop_name, "Taller Modo Local");

        // 3. Autenticando con token en keyring (JLRDev.TallerCostura) -> pasa a Authenticated y persiste UID
        let test_user = "test_firebase_guard_session_ci";
        let _ = delete_keyring_secret(KEYRING_SERVICE, test_user);

        let saved_data = apply_save_secure_session_token(
            &mut app,
            "uid_taller_autenticado",
            "refresh_token_seguro_abc",
            Some("taller@costura.sv"),
            KEYRING_SERVICE,
            test_user,
        )
        .expect("Debe registrar sesión autenticada y guardar en keyring");
        assert_eq!(
            saved_data.firebase.active_uid.as_deref(),
            Some("uid_taller_autenticado")
        );

        let status_auth = build_session_status(&app);
        assert!(status_auth.authorized);
        assert_eq!(status_auth.mode, "authenticated");
        assert_eq!(
            status_auth.active_uid.as_deref(),
            Some("uid_taller_autenticado")
        );
        assert_eq!(status_auth.email.as_deref(), Some("taller@costura.sv"));

        // 4. Al cerrar sesión (clear_secure_session_token) -> revoca estado en Rust y limpia keyring
        let status_cleared =
            apply_clear_secure_session_token(&mut app, KEYRING_SERVICE, test_user)
                .expect("Debe revocar sesión y limpiar keyring");
        assert!(!status_cleared.authorized);
        assert_eq!(status_cleared.mode, "unauthenticated");
        assert!(app.data.firebase.active_uid.is_none());
        assert!(ensure_session_authorized(&app).is_err());

        let keyring_after = read_keyring_secret(KEYRING_SERVICE, test_user).unwrap();
        assert!(keyring_after.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
