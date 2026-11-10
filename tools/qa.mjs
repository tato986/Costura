// Recorrido de control de calidad: navega la app, crea/edita/borra datos y captura pantallas.
// Uso: node tools/qa.mjs <wsUrl>

import fs from "node:fs";
import path from "node:path";

const [, , wsUrl] = process.argv;
if (!wsUrl) {
  console.error("Falta el WebSocket de CDP");
  process.exit(2);
}

const root = path.resolve(import.meta.dirname, "..");
const shots = path.join(root, "shots");
fs.mkdirSync(shots, { recursive: true });

const ws = new WebSocket(wsUrl);
let nextId = 1;
const pending = new Map();

function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

ws.addEventListener("message", (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  }
});

await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", () => reject(new Error("WebSocket no disponible")), { once: true });
});

await send("Page.enable");

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
  }
  return result.result?.value;
}

async function shot(name) {
  const { data } = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
  fs.writeFileSync(path.join(shots, name), Buffer.from(data, "base64"));
  console.log("· captura " + name);
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

async function click(selector, index = 0) {
  const outcome = await evaluate(`(() => {
    const list = document.querySelectorAll(${JSON.stringify(selector)});
    if (!list[${index}]) return "NO_ENCONTRADO";
    list[${index}].click();
    return "OK";
  })()`);
  if (outcome !== "OK") throw new Error("No se encontró: " + selector + " #" + index);
  await wait(230);
}

async function fill(selector, value) {
  const outcome = await evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return "NO_ENCONTRADO";
    el.focus();
    el.value = ${JSON.stringify(String(value))};
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    return "OK";
  })()`);
  if (outcome !== "OK") throw new Error("No se encontró el campo: " + selector);
}

async function submit(formSelector) {
  const outcome = await evaluate(
    `(() => { const f = document.querySelector(${JSON.stringify(formSelector)}); if (!f) return "NO_ENCONTRADO"; f.requestSubmit(); return "OK"; })()`
  );
  if (outcome !== "OK") throw new Error("No se encontró el formulario: " + formSelector);
  await wait(700);
}

async function modalScroll(top) {
  await evaluate(
    `(() => { const b = document.querySelector('#modal .modal-body'); if (b) b.scrollTop = ${Number(top)}; return 'OK'; })()`
  );
  await wait(200);
}

async function clicEnFila(texto) {
  const outcome = await evaluate(`(() => {
    const rows = Array.from(document.querySelectorAll('#viewRoot tbody tr, #viewRoot .list-row, #viewRoot .client-card, .note-list .note'));
    const row = rows.find((r) => r.textContent.includes(${JSON.stringify(texto)}));
    if (!row) return "NO_ENCONTRADO";
    row.click();
    return "OK";
  })()`);
  if (outcome !== "OK") throw new Error("No se encontró una fila con: " + texto);
  await wait(450);
}

async function snapshot() {
  return evaluate("window.__TAURI__.core.invoke('get_state').then(s => ({ data: s.data, file: s.dataFile }))");
}

async function confirmarSi() {
  await click('[data-confirm="yes"]');
  await wait(650);
}

try {
  console.log("Errores JS previos:", await evaluate("window.__lastError || 'ninguno'"));

  await wait(400);
  await shot("01-inicio.png");

  // ── Pedidos ────────────────────────────────────────────────────
  await click('[data-view="pedidos"]');
  await shot("02-pedidos.png");

  await click('#viewRoot [data-action="new-order"]');
  await wait(350);
  await shot("03-formulario.png");

  await fill("#orderPiece", "Pantalón de prueba QA");
  await click('[data-action="order-type"][data-type="pantalon"]');
  await click('[data-action="apply-preset"][data-preset="Baggy"]');
  await fill("#orderCost", "55");
  await fill("#orderDeposit", "20");
  await fill("#orderFabric", "Drill azul, prueba automática");
  await modalScroll(250);
  await shot("04-formulario-pantalon.png");

  const before = (await snapshot()).data.orders.length;
  await submit('form[data-form="order"]');
  await wait(450);
  const afterCreate = await snapshot();
  console.log(`· pedidos: ${before} -> ${afterCreate.data.orders.length}`);
  if (afterCreate.data.orders.length !== before + 1) throw new Error("El pedido no se guardó");
  const creado = afterCreate.data.orders.find((o) => o.pieceName === "Pantalón de prueba QA");
  if (!creado) throw new Error("No se encontró el pedido creado");
  console.log("· medidas guardadas:", JSON.stringify(creado.measurements.values));
  await shot("05-pedido-guardado.png");

  // Detalle, cambio de estado y nota
  await clicEnFila("Pantalón de prueba QA");
  await shot("06-detalle-pedido.png");
  await click('#modal [data-action="set-status"][data-status="en_corte"]');
  await wait(400);
  await fill('[data-form="note"] [name="content"]', "Corte realizado y piezas marcadas (QA).");
  await submit('form[data-form="note"]');
  await modalScroll(2000);
  await shot("07-bitacora.png");

  // Editar el pedido
  await click('#modal [data-action="edit-order"]');
  await wait(350);
  const piezaActual = await evaluate("document.querySelector('#orderPiece')?.value");
  if (piezaActual !== "Pantalón de prueba QA") throw new Error("El formulario de edición no cargó la prenda");
  await fill("#orderPiece", "Pantalón QA editado");
  await submit('form[data-form="order"]');
  await wait(500);
  const editado = (await snapshot()).data.orders.find((o) => o.id === creado.id);
  if (!editado || editado.pieceName !== "Pantalón QA editado") throw new Error("La edición no se guardó");
  console.log("· pedido editado correctamente");

  // Borrar el pedido (con confirmación)
  await clicEnFila("Pantalón QA editado");
  await click('#modal [data-action="delete-order"]');
  await wait(300);
  await shot("08-confirmar-borrado.png");
  await confirmarSi();
  const trasBorrar = await snapshot();
  if (trasBorrar.data.orders.some((o) => o.id === creado.id)) throw new Error("El pedido no se eliminó");
  console.log("· pedido eliminado correctamente");

  // ── Clientes ───────────────────────────────────────────────────
  await click('[data-view="clientes"]');
  await shot("09-clientes.png");
  await click('#viewRoot [data-action="new-client"]');
  await wait(300);
  await fill("#cName", "Cliente de prueba QA");
  await fill("#cPhone", "+503 7000-0000");
  await fill("#cNotes", "Cliente temporal creado por el control de calidad.");
  await shot("10-cliente-formulario.png");
  await submit('form[data-form="customer"]');
  await wait(400);
  const conCliente = await snapshot();
  const clienteQa = conCliente.data.customers.find((c) => c.fullName === "Cliente de prueba QA");
  if (!clienteQa) throw new Error("El cliente no se creó");
  console.log("· cliente creado:", clienteQa.id.slice(0, 8));

  await clicEnFila("Cliente de prueba QA");
  await shot("11-cliente-detalle.png");
  await click('#modal [data-action="edit-client"]');
  await wait(300);
  await fill("#cName", "Cliente QA editado");
  await submit('form[data-form="customer"]');
  await wait(400);
  const editadoCliente = (await snapshot()).data.customers.find((c) => c.id === clienteQa.id);
  if (!editadoCliente || editadoCliente.fullName !== "Cliente QA editado") throw new Error("El cliente no se editó");

  await clicEnFila("Cliente QA editado");
  await click('#modal [data-action="delete-client"]');
  await wait(300);
  await confirmarSi();
  const sinCliente = await snapshot();
  if (sinCliente.data.customers.some((c) => c.id === clienteQa.id)) throw new Error("El cliente no se eliminó");
  console.log("· cliente editado y eliminado correctamente");

  // ── Agenda ─────────────────────────────────────────────────────
  await click('[data-view="agenda"]');
  await shot("12-agenda.png");
  await click('#viewRoot [data-action="new-reminder"]');
  await wait(300);
  await fill("#rTitle", "Recordatorio de prueba QA");
  await fill("#rDesc", "Verificar que la agenda guarda y ordena.");
  await shot("13-recordatorio-formulario.png");
  await submit('form[data-form="reminder"]');
  await wait(450);
  await shot("14-recordatorio-creado.png");

  const marcado = await evaluate(`(() => {
    const rows = Array.from(document.querySelectorAll('.reminder'));
    const row = rows.find((r) => r.textContent.includes('Recordatorio de prueba QA'));
    if (!row) return 'NO_ENCONTRADO';
    row.querySelector('[data-action="toggle-reminder"]').click();
    return 'OK';
  })()`);
  if (marcado !== "OK") throw new Error("No se encontró el recordatorio para marcar");
  await wait(400);
  const trasToggle = await snapshot();
  const qaReminder = trasToggle.data.reminders.find((r) => r.title === "Recordatorio de prueba QA");
  if (!qaReminder || !qaReminder.completed) throw new Error("El recordatorio no se marcó");
  console.log("· recordatorio marcado como completado");
  await click('[data-view="agenda"]');
  await wait(250);
  await click('[data-action="agenda-filter"][data-filter="todos"]');
  await wait(250);
  const filaRecordatorio = await evaluate(`(() => {
    const rows = Array.from(document.querySelectorAll('.reminder'));
    const row = rows.find((r) => r.textContent.includes('Recordatorio de prueba QA'));
    if (!row) return 'NO_ENCONTRADO';
    row.querySelector('[data-action="delete-reminder"]').click();
    return 'OK';
  })()`);
  if (filaRecordatorio !== "OK") throw new Error("No se encontró el recordatorio para borrar");
  await wait(300);
  await confirmarSi();
  const sinRecordatorio = await snapshot();
  if (sinRecordatorio.data.reminders.some((r) => r.title === "Recordatorio de prueba QA")) throw new Error("El recordatorio no se eliminó");
  console.log("· recordatorio eliminado correctamente");

  // ── Ajustes, respaldos y tema ──────────────────────────────────
  await click('[data-view="ajustes"]');
  await wait(400);
  await shot("15-ajustes.png");

  await fill("#wsName", "Taller Aurora QA");
  await submit('form[data-form="settings"]');
  await wait(400);
  let ajustes = await snapshot();
  if (ajustes.data.settings.workshopName !== "Taller Aurora QA") throw new Error("Los ajustes no se guardaron");
  await fill("#wsName", "Taller Aurora");
  await submit('form[data-form="settings"]');
  await wait(300);
  console.log("· ajustes guardados y restaurados");

  await click('[data-action="backup-create"]');
  await wait(900);
  await shot("16-respaldo-creado.png");
  const backups = await evaluate("window.__TAURI__.core.invoke('list_backups')");
  console.log("· respaldos disponibles:", backups.length, backups.map((b) => b.file).join(", "));

  // Restaurar el respaldo automático (el más antiguo) al final
  await click('[data-action="backup-restore"]', backups.length - 1);
  await wait(300);
  await confirmarSi();
  await wait(700);
  const restaurado = await snapshot();
  console.log(
    `· tras restaurar: ${restaurado.data.customers.length} clientes, ${restaurado.data.orders.length} pedidos, ${restaurado.data.reminders.length} recordatorios`
  );
  await shot("17-tras-restaurar.png");

  // Tema claro
  await click("#themeToggle");
  await wait(300);
  await shot("18-ajustes-claro.png");
  await click('[data-view="inicio"]');
  await wait(350);
  await shot("19-inicio-claro.png");

  const summary = await snapshot();
  console.log(
    "Resumen final:",
    JSON.stringify({
      clientes: summary.data.customers.length,
      pedidos: summary.data.orders.length,
      recordatorios: summary.data.reminders.length,
      archivo: summary.file,
      error: await evaluate("window.__lastError || null"),
    })
  );
} catch (err) {
  console.error("FALLO: " + err.message);
  await shot("error.png").catch(() => {});
  process.exitCode = 1;
} finally {
  ws.close();
}
