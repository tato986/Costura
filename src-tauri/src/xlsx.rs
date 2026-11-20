// ─────────────────────────────────────────────────────────────────────────────
// Costura v2.0 — Exportación Contable `.xlsx` de 3 Pestañas (`rust_xlsxwriter`)
// ─────────────────────────────────────────────────────────────────────────────
// Estructura del libro contable:
// 1. `Resumen_Mes` (Hacienda / Contabilidad):
//    - Periodo consultado y moneda del taller.
//    - Total Ingresos Brutos, Total Anticipos Cobrados y Saldo Pendiente por Cobrar.
//    - Desglose Fiscal Dinámico según `taxRatePercent` y `taxLabel`:
//      Si `taxRatePercent > 0`, calcula Subtotal Base Gravable, Impuesto Trasladado y Total Neto.
//      Si `taxRatePercent == 0`, muestra la fila de impuesto en `0.00 (Exento / No aplica)`.
//    - Costo Total de Insumos Consumidos, Margen Bruto Operativo y desglose mensual por categoría
//      (`camisa`, `pantalon`, `vestido`, `bolso`, `otro`).
// 2. `Detalle_Pedidos`:
//    - Folio/ID, Fecha Creación, Fecha Compromiso, Fecha Entrega, Cliente, Teléfono, Categoría,
//      Subtipo (`Otros`), Prenda/Trabajo, Estado, Costo Total, Impuesto Estimado, Anticipo, Saldo Pendiente.
// 3. `Movimientos_Insumos`:
//    - Fecha/Hora, ID Determinista, Insumo, Categoría, Unidad, Tipo de Movimiento,
//      Cantidad (Δ), Costo Unitario, Impacto Monetario, Motivo / Pedido vinculado (`orderId` + `revision`).

use crate::model::{round_money, AppData, Order, SupplyMovement};
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdownRow {
    pub category_key: String,
    pub category_label: String,
    pub order_count: usize,
    pub gross_income: f64,
    pub deposits_collected: f64,
    pub pending_balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountingMonthSummary {
    pub period: String,
    pub workshop_name: String,
    pub currency: String,
    pub tax_rate_percent: f64,
    pub tax_label: String,
    pub is_tax_exempt: bool,
    pub orders_count: usize,
    pub gross_income: f64,
    pub deposits_collected: f64,
    pub pending_balance: f64,
    pub taxable_subtotal: f64,
    pub tax_amount: f64,
    pub net_total: f64,
    pub supplies_consumed_cost: f64,
    pub gross_operating_margin: f64,
    pub categories: Vec<CategoryBreakdownRow>,
}

fn matches_year_month(date_or_iso: &str, year: i32, month: u32) -> bool {
    if year <= 0 || !(1..=12).contains(&month) {
        return true;
    }
    let prefix = format!("{year:04}-{month:02}");
    date_or_iso.trim().starts_with(&prefix)
}

fn order_matches_period(order: &Order, year: i32, month: u32) -> bool {
    if year <= 0 || !(1..=12).contains(&month) {
        return true;
    }
    matches_year_month(&order.due_date, year, month)
        || matches_year_month(&order.created_at, year, month)
        || order
            .delivery_date
            .as_deref()
            .map(|d| matches_year_month(d, year, month))
            .unwrap_or(false)
}

fn movement_matches_period(mov: &SupplyMovement, year: i32, month: u32) -> bool {
    if year <= 0 || !(1..=12).contains(&month) {
        return true;
    }
    matches_year_month(&mov.timestamp, year, month)
}

fn category_label(key: &str) -> &'static str {
    match key {
        "camisa" => "Camisas",
        "pantalon" => "Pantalones",
        "vestido" => "Vestidos",
        "bolso" => "Bolsos",
        _ => "Otros",
    }
}

/// Calcula el resumen contable y fiscal del periodo (`year`, `month`).
/// Si `year == 0` o `month == 0`, incluye todos los registros históricos.
pub fn compute_accounting_summary(
    data: &AppData,
    year: i32,
    month: u32,
) -> AccountingMonthSummary {
    let period = if year > 0 && (1..=12).contains(&month) {
        format!("{year:04}-{month:02}")
    } else {
        "TODOS".to_string()
    };

    let tax_rate = if data.settings.tax_rate_percent.is_finite()
        && (0.0..=100.0).contains(&data.settings.tax_rate_percent)
    {
        round_money(data.settings.tax_rate_percent)
    } else {
        0.0
    };
    let tax_label = if data.settings.tax_label.trim().is_empty() {
        "IVA".to_string()
    } else {
        data.settings.tax_label.trim().to_string()
    };
    let is_tax_exempt = tax_rate <= 1e-9;

    let filtered_orders: Vec<&Order> = data
        .orders
        .iter()
        .filter(|o| order_matches_period(o, year, month))
        .collect();

    let mut gross_income = 0.0;
    let mut deposits_collected = 0.0;
    let mut pending_balance = 0.0;

    let mut cat_map: BTreeMap<&str, (usize, f64, f64, f64)> = BTreeMap::from([
        ("camisa", (0, 0.0, 0.0, 0.0)),
        ("pantalon", (0, 0.0, 0.0, 0.0)),
        ("vestido", (0, 0.0, 0.0, 0.0)),
        ("bolso", (0, 0.0, 0.0, 0.0)),
        ("otro", (0, 0.0, 0.0, 0.0)),
    ]);

    for o in &filtered_orders {
        let cost = o.estimated_cost.max(0.0);
        let dep = o.deposit_paid.max(0.0);
        let bal = (cost - dep).max(0.0);
        gross_income += cost;
        deposits_collected += dep;
        pending_balance += bal;

        let key = match o.garment_type.as_str() {
            "camisa" => "camisa",
            "pantalon" => "pantalon",
            "vestido" => "vestido",
            "bolso" => "bolso",
            _ => "otro",
        };
        if let Some(entry) = cat_map.get_mut(key) {
            entry.0 += 1;
            entry.1 += cost;
            entry.2 += dep;
            entry.3 += bal;
        }
    }

    gross_income = round_money(gross_income);
    deposits_collected = round_money(deposits_collected);
    pending_balance = round_money(pending_balance);

    let (taxable_subtotal, tax_amount, net_total) = if is_tax_exempt {
        (gross_income, 0.0, gross_income)
    } else {
        // Desglose fiscal: Base gravable + Impuesto trasladado = Total
        let factor = 1.0 + (tax_rate / 100.0);
        let base = round_money(gross_income / factor);
        let tax = round_money(gross_income - base);
        (base, tax, gross_income)
    };

    // Costo de insumos consumidos en el periodo (movimientos de salida o insumos asignados a pedidos del periodo)
    let supply_costs: BTreeMap<&str, f64> = data
        .supplies
        .iter()
        .map(|s| (s.id.as_str(), s.unit_cost.max(0.0)))
        .collect();

    let mut supplies_consumed_cost = 0.0;
    let filtered_movements: Vec<&SupplyMovement> = data
        .movements
        .iter()
        .filter(|m| movement_matches_period(m, year, month))
        .collect();

    if !filtered_movements.is_empty() {
        for m in &filtered_movements {
            let unit_cost = supply_costs.get(m.supply_id.as_str()).copied().unwrap_or(0.0);
            if m.kind == "salida" || m.delta < 0.0 {
                supplies_consumed_cost += m.delta.abs() * unit_cost;
            } else if m.kind == "devolucion" && m.delta > 0.0 {
                supplies_consumed_cost -= m.delta * unit_cost;
            }
        }
        supplies_consumed_cost = round_money(supplies_consumed_cost.max(0.0));
    } else {
        for o in &filtered_orders {
            for u in &o.supplies {
                let unit_cost = supply_costs.get(u.supply_id.as_str()).copied().unwrap_or(0.0);
                supplies_consumed_cost += u.quantity.max(0.0) * unit_cost;
            }
        }
        supplies_consumed_cost = round_money(supplies_consumed_cost);
    }

    let gross_operating_margin = round_money(taxable_subtotal - supplies_consumed_cost);

    let ordered_keys = ["camisa", "pantalon", "vestido", "bolso", "otro"];
    let categories = ordered_keys
        .into_iter()
        .map(|k| {
            let (count, inc, dep, bal) = cat_map.get(k).copied().unwrap_or((0, 0.0, 0.0, 0.0));
            CategoryBreakdownRow {
                category_key: k.to_string(),
                category_label: category_label(k).to_string(),
                order_count: count,
                gross_income: round_money(inc),
                deposits_collected: round_money(dep),
                pending_balance: round_money(bal),
            }
        })
        .collect();

    AccountingMonthSummary {
        period,
        workshop_name: data.settings.workshop_name.clone(),
        currency: data.settings.currency.clone(),
        tax_rate_percent: tax_rate,
        tax_label,
        is_tax_exempt,
        orders_count: filtered_orders.len(),
        gross_income,
        deposits_collected,
        pending_balance,
        taxable_subtotal,
        tax_amount,
        net_total,
        supplies_consumed_cost,
        gross_operating_margin,
        categories,
    }
}

/// Genera y escribe en disco el archivo `.xlsx` con las 3 pestañas contables:
/// `Resumen_Mes`, `Detalle_Pedidos` y `Movimientos_Insumos`.
pub fn write_accounting_xlsx(
    data: &AppData,
    year: i32,
    month: u32,
    output_path: &Path,
) -> Result<AccountingMonthSummary, String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear la carpeta del reporte Excel: {e}"))?;
    }

    let summary = compute_accounting_summary(data, year, month);
    let mut workbook = Workbook::new();

    // Formatos sobrios (Paleta neutra + Morado acento #5B21B6 en cabeceras principales)
    let title_fmt = Format::new()
        .set_bold()
        .set_font_size(14)
        .set_font_color(Color::RGB(0x18181B));

    let section_fmt = Format::new()
        .set_bold()
        .set_font_size(11)
        .set_background_color(Color::RGB(0xF3EEFC))
        .set_font_color(Color::RGB(0x5B21B6))
        .set_border(FormatBorder::Thin);

    let header_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x5B21B6))
        .set_font_color(Color::White)
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);

    let label_fmt = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0x18181B))
        .set_border(FormatBorder::Thin);

    let text_fmt = Format::new().set_border(FormatBorder::Thin);

    let money_fmt = Format::new()
        .set_num_format("#,##0.00")
        .set_border(FormatBorder::Thin);

    let money_bold_fmt = Format::new()
        .set_bold()
        .set_num_format("#,##0.00")
        .set_border(FormatBorder::Thin);

    let qty_fmt = Format::new()
        .set_num_format("#,##0.00")
        .set_border(FormatBorder::Thin);

    // ─────────────────────────────────────────────────────────────────────────
    // PESTAÑA 1: Resumen_Mes
    // ─────────────────────────────────────────────────────────────────────────
    {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Resumen_Mes")
            .map_err(|e| format!("Error al nombrar hoja Resumen_Mes: {e}"))?;

        sheet.set_column_width(0, 38).ok();
        sheet.set_column_width(1, 22).ok();
        sheet.set_column_width(2, 20).ok();
        sheet.set_column_width(3, 20).ok();
        sheet.set_column_width(4, 20).ok();

        sheet
            .write_string_with_format(
                0,
                0,
                &format!("Reporte Contable — {}", summary.workshop_name),
                &title_fmt,
            )
            .ok();

        sheet
            .write_string_with_format(2, 0, "Periodo Consultado", &label_fmt)
            .ok();
        sheet
            .write_string_with_format(2, 1, &summary.period, &text_fmt)
            .ok();

        sheet
            .write_string_with_format(3, 0, "Moneda del Taller", &label_fmt)
            .ok();
        sheet
            .write_string_with_format(3, 1, &summary.currency, &text_fmt)
            .ok();

        sheet
            .write_string_with_format(4, 0, "Pedidos Contabilizados en el Periodo", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(4, 1, summary.orders_count as f64, &text_fmt)
            .ok();

        // Bloque de Ingresos y Desglose Fiscal Dinámico
        sheet
            .write_string_with_format(6, 0, "RESUMEN DE INGRESOS Y DESGLOSE FISCAL", &section_fmt)
            .ok();
        sheet
            .write_string_with_format(6, 1, "Importe", &section_fmt)
            .ok();

        sheet
            .write_string_with_format(7, 0, "Total Ingresos Brutos (Pedidos)", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(7, 1, summary.gross_income, &money_bold_fmt)
            .ok();

        sheet
            .write_string_with_format(8, 0, "Total Anticipos Cobrados", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(8, 1, summary.deposits_collected, &money_fmt)
            .ok();

        sheet
            .write_string_with_format(9, 0, "Saldo Pendiente por Cobrar", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(9, 1, summary.pending_balance, &money_fmt)
            .ok();

        sheet
            .write_string_with_format(10, 0, "Subtotal Base Gravable", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(10, 1, summary.taxable_subtotal, &money_fmt)
            .ok();

        let tax_row_title = if summary.is_tax_exempt {
            format!(
                "Impuesto Trasladado ({} 0.00% — Exento / No aplica)",
                summary.tax_label
            )
        } else {
            format!(
                "Impuesto Trasladado ({} {:.2}%)",
                summary.tax_label, summary.tax_rate_percent
            )
        };
        sheet
            .write_string_with_format(11, 0, &tax_row_title, &label_fmt)
            .ok();
        sheet
            .write_number_with_format(11, 1, summary.tax_amount, &money_fmt)
            .ok();

        sheet
            .write_string_with_format(12, 0, "Total Neto Facturable", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(12, 1, summary.net_total, &money_bold_fmt)
            .ok();

        sheet
            .write_string_with_format(13, 0, "Costo Total de Insumos Consumidos", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(13, 1, summary.supplies_consumed_cost, &money_fmt)
            .ok();

        sheet
            .write_string_with_format(14, 0, "Margen Bruto Operativo", &label_fmt)
            .ok();
        sheet
            .write_number_with_format(14, 1, summary.gross_operating_margin, &money_bold_fmt)
            .ok();

        // Tabla de Desglose por Categoría
        sheet
            .write_string_with_format(16, 0, "Categoría", &header_fmt)
            .ok();
        sheet
            .write_string_with_format(16, 1, "Cantidad Pedidos", &header_fmt)
            .ok();
        sheet
            .write_string_with_format(16, 2, "Ingresos Brutos", &header_fmt)
            .ok();
        sheet
            .write_string_with_format(16, 3, "Anticipos", &header_fmt)
            .ok();
        sheet
            .write_string_with_format(16, 4, "Saldo Pendiente", &header_fmt)
            .ok();

        for (idx, cat) in summary.categories.iter().enumerate() {
            let row = (17 + idx) as u32;
            sheet
                .write_string_with_format(row, 0, &cat.category_label, &text_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 1, cat.order_count as f64, &text_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 2, cat.gross_income, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 3, cat.deposits_collected, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 4, cat.pending_balance, &money_fmt)
                .ok();
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // PESTAÑA 2: Detalle_Pedidos
    // ─────────────────────────────────────────────────────────────────────────
    {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Detalle_Pedidos")
            .map_err(|e| format!("Error al nombrar hoja Detalle_Pedidos: {e}"))?;

        let headers = [
            ("Folio / ID", 22),
            ("Fecha Creación", 16),
            ("Fecha Compromiso", 16),
            ("Fecha Entrega", 16),
            ("Cliente", 26),
            ("Teléfono", 16),
            ("Categoría", 16),
            ("Subtipo (Otros)", 20),
            ("Prenda / Trabajo", 28),
            ("Estado", 16),
            ("Costo Total", 16),
            ("Impuesto Estimado", 18),
            ("Anticipo", 16),
            ("Saldo Pendiente", 16),
        ];

        for (col, (title, width)) in headers.iter().enumerate() {
            sheet.set_column_width(col as u16, *width).ok();
            sheet
                .write_string_with_format(0, col as u16, *title, &header_fmt)
                .ok();
        }

        let customers_by_id: BTreeMap<&str, (&str, &str)> = data
            .customers
            .iter()
            .map(|c| (c.id.as_str(), (c.full_name.as_str(), c.phone.as_str())))
            .collect();

        let filtered_orders: Vec<&Order> = data
            .orders
            .iter()
            .filter(|o| order_matches_period(o, year, month))
            .collect();

        for (idx, o) in filtered_orders.iter().enumerate() {
            let row = (idx + 1) as u32;
            let (client_name, client_phone) = customers_by_id
                .get(o.customer_id.as_str())
                .copied()
                .unwrap_or(("Cliente eliminado", ""));

            let cost = round_money(o.estimated_cost.max(0.0));
            let dep = round_money(o.deposit_paid.max(0.0));
            let bal = round_money((cost - dep).max(0.0));
            let est_tax = if summary.is_tax_exempt {
                0.0
            } else {
                let factor = 1.0 + (summary.tax_rate_percent / 100.0);
                let base = round_money(cost / factor);
                round_money(cost - base)
            };

            let created_date = o.created_at.chars().take(10).collect::<String>();
            let delivery_date = o.delivery_date.clone().unwrap_or_default();

            sheet.write_string_with_format(row, 0, &o.id, &text_fmt).ok();
            sheet
                .write_string_with_format(row, 1, &created_date, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 2, &o.due_date, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 3, &delivery_date, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 4, client_name, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 5, client_phone, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 6, category_label(&o.garment_type), &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 7, &o.custom_type, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 8, &o.piece_name, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 9, &o.status, &text_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 10, cost, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 11, est_tax, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 12, dep, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 13, bal, &money_fmt)
                .ok();
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // PESTAÑA 3: Movimientos_Insumos
    // ─────────────────────────────────────────────────────────────────────────
    {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Movimientos_Insumos")
            .map_err(|e| format!("Error al nombrar hoja Movimientos_Insumos: {e}"))?;

        let headers = [
            ("Fecha / Hora", 22),
            ("ID Movimiento", 28),
            ("Insumo", 24),
            ("Categoría", 16),
            ("Unidad", 14),
            ("Tipo de Movimiento", 18),
            ("Cantidad (Δ)", 16),
            ("Costo Unitario", 16),
            ("Impacto Monetario", 18),
            ("Motivo / Pedido Vinculado", 34),
        ];

        for (col, (title, width)) in headers.iter().enumerate() {
            sheet.set_column_width(col as u16, *width).ok();
            sheet
                .write_string_with_format(0, col as u16, *title, &header_fmt)
                .ok();
        }

        let supplies_by_id: BTreeMap<&str, (&str, &str, &str, f64)> = data
            .supplies
            .iter()
            .map(|s| {
                (
                    s.id.as_str(),
                    (
                        s.name.as_str(),
                        s.category.as_str(),
                        s.unit.as_str(),
                        s.unit_cost.max(0.0),
                    ),
                )
            })
            .collect();

        let filtered_movements: Vec<&SupplyMovement> = data
            .movements
            .iter()
            .filter(|m| movement_matches_period(m, year, month))
            .collect();

        for (idx, m) in filtered_movements.iter().enumerate() {
            let row = (idx + 1) as u32;
            let (sup_name, sup_cat, sup_unit, unit_cost) = supplies_by_id
                .get(m.supply_id.as_str())
                .copied()
                .unwrap_or(("Insumo eliminado", "otro", "unidad", 0.0));

            let impact = round_money(m.delta * unit_cost);
            let linked_info = match (&m.order_id, m.order_revision) {
                (Some(oid), Some(rev)) => format!("{} (orderId={} r{})", m.reason, oid, rev),
                (Some(oid), None) => format!("{} (orderId={})", m.reason, oid),
                _ => m.reason.clone(),
            };

            sheet
                .write_string_with_format(row, 0, &m.timestamp, &text_fmt)
                .ok();
            sheet.write_string_with_format(row, 1, &m.id, &text_fmt).ok();
            sheet
                .write_string_with_format(row, 2, sup_name, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 3, sup_cat, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 4, sup_unit, &text_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 5, &m.kind, &text_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 6, m.delta, &qty_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 7, unit_cost, &money_fmt)
                .ok();
            sheet
                .write_number_with_format(row, 8, impact, &money_fmt)
                .ok();
            sheet
                .write_string_with_format(row, 9, &linked_info, &text_fmt)
                .ok();
        }
    }

    workbook
        .save(output_path)
        .map_err(|e| format!("No se pudo guardar el archivo Excel (.xlsx): {e}"))?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        deterministic_movement_id, now_iso, Customer, Order, Settings, Supply, SupplyMovement,
        SupplyUsage,
    };
    use std::fs;
    use std::path::PathBuf;

    fn sample_accounting_data(tax_rate_percent: f64) -> AppData {
        let customer = Customer {
            id: "cli_1".into(),
            full_name: "Elena Martínez".into(),
            phone: "+503 7000-1111".into(),
            created_at: "2026-11-01T10:00:00Z".into(),
            updated_at: "2026-11-01T10:00:00Z".into(),
            ..Default::default()
        };

        let supply = Supply {
            id: "sup_lino".into(),
            name: "Lino Italiano".into(),
            category: "tela".into(),
            unit: "metro".into(),
            stock: 18.0,
            min_stock: 5.0,
            unit_cost: 10.0,
            created_at: now_iso(),
            updated_at: now_iso(),
            ..Default::default()
        };

        let order_camisa = Order {
            id: "ord_100".into(),
            customer_id: customer.id.clone(),
            piece_name: "Camisa de lino".into(),
            garment_type: "camisa".into(),
            status: "en_confeccion".into(),
            estimated_cost: 116.0,
            deposit_paid: 50.0,
            due_date: "2026-11-20".into(),
            created_at: "2026-11-05T12:00:00Z".into(),
            updated_at: "2026-11-05T12:00:00Z".into(),
            revision: 1,
            supplies: vec![SupplyUsage {
                supply_id: supply.id.clone(),
                quantity: 2.0,
                cut: String::new(),
                note: String::new(),
            }],
            ..Default::default()
        };

        let order_otro = Order {
            id: "ord_101".into(),
            customer_id: customer.id.clone(),
            piece_name: "Ajuste de cortina".into(),
            garment_type: "otro".into(),
            custom_type: "Cortina".into(),
            status: "entregado".into(),
            estimated_cost: 58.0,
            deposit_paid: 58.0,
            due_date: "2026-11-22".into(),
            delivery_date: Some("2026-11-22".into()),
            created_at: "2026-11-10T15:00:00Z".into(),
            updated_at: "2026-11-22T15:00:00Z".into(),
            revision: 1,
            ..Default::default()
        };

        let mov = SupplyMovement {
            id: deterministic_movement_id("ord_100", 1, "sup_lino"),
            supply_id: supply.id.clone(),
            timestamp: "2026-11-05T12:00:00Z".into(),
            delta: -2.0,
            kind: "salida".into(),
            reason: "Pedido creado: Camisa de lino".into(),
            order_id: Some("ord_100".into()),
            order_revision: Some(1),
        };

        AppData {
            settings: Settings {
                workshop_name: "Taller Aurora".into(),
                owner_name: "Elena".into(),
                currency: "$".into(),
                tax_rate_percent,
                tax_label: "IVA".into(),
                default_due_days: 7,
                backup_dir: String::new(),
                language: "es".into(),
                updated_at: now_iso(),
            },
            customers: vec![customer],
            orders: vec![order_camisa, order_otro],
            supplies: vec![supply],
            movements: vec![mov],
            ..Default::default()
        }
    }

    #[test]
    fn calcula_desglose_fiscal_con_tasa_16_por_ciento_y_modo_exento() {
        // 1. Con IVA 16%: Bruto = 116 + 58 = 174.00 -> Base = 150.00, IVA = 24.00
        let data_with_tax = sample_accounting_data(16.0);
        let sum_tax = compute_accounting_summary(&data_with_tax, 2026, 11);
        assert_eq!(sum_tax.orders_count, 2);
        assert_eq!(sum_tax.gross_income, 174.0);
        assert_eq!(sum_tax.deposits_collected, 108.0);
        assert_eq!(sum_tax.pending_balance, 66.0);
        assert!(!sum_tax.is_tax_exempt);
        assert_eq!(sum_tax.taxable_subtotal, 150.0);
        assert_eq!(sum_tax.tax_amount, 24.0);
        assert_eq!(sum_tax.net_total, 174.0);
        assert_eq!(sum_tax.supplies_consumed_cost, 20.0); // 2m * $10
        assert_eq!(sum_tax.gross_operating_margin, 130.0); // 150 - 20

        // 2. Modo Exento (0%): Impuesto = 0.00, Base = 174.00
        let data_exempt = sample_accounting_data(0.0);
        let sum_exempt = compute_accounting_summary(&data_exempt, 2026, 11);
        assert!(sum_exempt.is_tax_exempt);
        assert_eq!(sum_exempt.tax_amount, 0.0);
        assert_eq!(sum_exempt.taxable_subtotal, 174.0);
        assert_eq!(sum_exempt.gross_operating_margin, 154.0); // 174 - 20
    }

    #[test]
    fn genera_archivo_xlsx_valido_con_las_3_pestanas() {
        let data = sample_accounting_data(13.0);
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data")
            .join(format!("xlsx-{}", uuid::Uuid::new_v4()));
        let file_path = out_dir.join("Costura_2026_11.xlsx");

        let summary = write_accounting_xlsx(&data, 2026, 11, &file_path).unwrap();
        assert_eq!(summary.period, "2026-11");
        assert!(file_path.exists());

        let bytes = fs::read(&file_path).unwrap();
        // Todo archivo OpenXML .xlsx válido es un contenedor ZIP que inicia con 'PK\x03\x04'
        assert!(bytes.len() > 2000);
        assert_eq!(&bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);

        // Verificamos entradas OpenXML estándar de las 3 hojas dentro del contenedor ZIP
        let as_lossy = String::from_utf8_lossy(&bytes);
        assert!(as_lossy.contains("xl/workbook.xml"));
        assert!(as_lossy.contains("xl/worksheets/sheet1.xml"));
        assert!(as_lossy.contains("xl/worksheets/sheet2.xml"));
        assert!(as_lossy.contains("xl/worksheets/sheet3.xml"));
        assert!(as_lossy.contains("xl/sharedStrings.xml"));

        let _ = fs::remove_dir_all(out_dir);
    }
}
