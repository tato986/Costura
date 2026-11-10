// Pruebas complementarias: impresión de ficha y persistencia tras recargar.
// Uso: node tools/qa2.mjs <wsUrl>

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

try {
  await send("Page.enable");

  // 1) Impresión: interceptamos window.print para revisar la ficha sin abrir el diálogo.
  await evaluate(
    "window.__printed = null; window.print = () => { window.__printed = document.getElementById('printRoot') ? document.getElementById('printRoot').innerText : null; }; 'ok'"
  );
  await evaluate("document.querySelector('[data-view=\"pedidos\"]').click()");
  await wait(300);
  await evaluate("document.querySelectorAll('#viewRoot tbody tr')[0].click()");
  await wait(400);
  await evaluate("document.querySelector('#modal [data-action=\"print-order\"]').click()");
  await wait(400);

  const ficha = await evaluate("window.__printed || ''");
  if (!ficha.includes("Ficha de pedido")) throw new Error("La ficha no se generó");
  if (!ficha.includes("Medidas")) throw new Error("La ficha no incluye medidas");
  console.log("· ficha generada:", ficha.split("\n").slice(0, 3).join(" / "));
  console.log("· longitud de la ficha:", ficha.length, "caracteres");
  await shot("20-ficha-impresa.png");

  await evaluate("document.getElementById('modal').close()");
  await wait(200);

  // 2) Persistencia en memoria tras recargar la página
  const antes = await evaluate("window.__TAURI__.core.invoke('get_state').then(s => s.data.orders.length)");
  await send("Page.reload", { ignoreCache: true });
  await wait(2500);
  const despues = await evaluate("window.__TAURI__.core.invoke('get_state').then(s => s.data.orders.length)");
  console.log(`· pedidos antes/después de recargar: ${antes}/${despues}`);
  if (antes !== despues) throw new Error("Los datos no sobrevivieron a la recarga");

  const domOk = await evaluate("document.querySelectorAll('.stat').length + document.querySelectorAll('#viewRoot .hero').length");
  if (!domOk) throw new Error("La interfaz no se volvió a montar");
  console.log("· interfaz remontada correctamente");
} catch (err) {
  console.error("FALLO: " + err.message);
  await shot("error2.png").catch(() => {});
  process.exitCode = 1;
} finally {
  ws.close();
}
