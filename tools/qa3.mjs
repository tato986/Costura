// Prueba del primer uso: app vacía -> crear cliente -> pedido.
// Uso: node tools/qa3.mjs <wsUrl>

import fs from "node:fs";
import path from "node:path";

const [, , wsUrl] = process.argv;
const root = path.resolve(import.meta.dirname, "..");
const shots = path.join(root, "shots");

const ws = new WebSocket(wsUrl);
let nextId = 1;
const pending = new Map();
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = nextId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });

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
  ws.addEventListener("error", () => reject(new Error("sin websocket")), { once: true });
});

async function evaluate(expression) {
  const r = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
  });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result?.value;
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const shot = async (name) => {
  const { data } = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
  fs.writeFileSync(path.join(shots, name), Buffer.from(data, "base64"));
  console.log("· captura " + name);
};

async function click(selector) {
  const outcome = await evaluate(
    `(() => { const el = document.querySelector(${JSON.stringify(selector)}); if (!el) return "NO"; el.click(); return "OK"; })()`
  );
  if (outcome !== "OK") throw new Error("No se encontró " + selector);
  await wait(300);
}

async function fill(selector, value) {
  await evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    el.focus(); el.value = ${JSON.stringify(String(value))};
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    return "OK";
  })()`);
}

try {
  await send("Page.enable");
  await wait(500);

  const inicial = await evaluate("window.__TAURI__.core.invoke('get_state').then(s => s.data)");
  console.log("· estado inicial:", inicial.customers.length, "clientes,", inicial.orders.length, "pedidos");
  await shot("21-primer-uso.png");

  // Sin clientes, "Nuevo pedido" debe pedir crear el cliente primero.
  await click('#viewRoot [data-action="new-order"]');
  const hayFormularioCliente = await evaluate("!!document.querySelector('#cName')");
  if (!hayFormularioCliente) throw new Error("No se abrió el formulario de cliente");
  console.log("· sin clientes, abre el formulario de cliente");
  await shot("22-primer-cliente.png");

  await fill("#cName", "Primera Clienta");
  await fill("#cPhone", "+503 7000-1234");
  await evaluate("document.querySelector('form[data-form=\"customer\"]').requestSubmit()");
  await wait(900);

  const enPedido = await evaluate(
    "!!document.querySelector('form[data-form=\"order\"]') && document.querySelector('#orderCustomer')?.value"
  );
  if (!enPedido) throw new Error("No continuó al formulario de pedido");
  const clienteId = await evaluate("document.querySelector('#orderCustomer').value");
  console.log("· continúa al pedido con el cliente seleccionado");
  await shot("23-primer-pedido.png");

  await fill("#orderPiece", "Vestido de primera prueba");
  await click('[data-action="order-type"][data-type="vestido"]');
  await fill("#orderCost", "60");
  await fill("#orderDeposit", "25");
  await evaluate("document.querySelector('form[data-form=\"order\"]').requestSubmit()");
  await wait(900);

  const final = await evaluate("window.__TAURI__.core.invoke('get_state').then(s => s.data)");
  if (final.customers.length !== 1) throw new Error("El cliente no quedó guardado");
  if (final.orders.length !== 1) throw new Error("El pedido no quedó guardado");
  if (final.orders[0].customerId !== clienteId) throw new Error("El pedido no quedó asociado al cliente");
  console.log("· primer pedido guardado:", final.orders[0].pieceName, "| medidas:", Object.keys(final.orders[0].measurements.values).length);
  await shot("24-primer-pedido-guardado.png");

  console.log("· error JS:", await evaluate("window.__lastError || 'ninguno'"));
} catch (err) {
  console.error("FALLO: " + err.message);
  await shot("error3.png").catch(() => {});
  process.exitCode = 1;
} finally {
  ws.close();
}
