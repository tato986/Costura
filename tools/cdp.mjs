// Pequeño cliente CDP para inspeccionar y capturar la app durante el desarrollo.
// Uso:
//   node tools/cdp.mjs <wsUrl> eval "<expresión js>"
//   node tools/cdp.mjs <wsUrl> click "<selector css>"
//   node tools/cdp.mjs <wsUrl> wait 800
//   node tools/cdp.mjs <wsUrl> shot salida.png
//   node tools/cdp.mjs <wsUrl> run tools/guion.js

import fs from "node:fs";

const [, , wsUrl, action, ...rest] = process.argv;
if (!wsUrl || !action) {
  console.error("Faltan argumentos: <wsUrl> <acción> [...]");
  process.exit(2);
}

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
  let msg;
  try {
    msg = JSON.parse(event.data);
  } catch {
    return;
  }
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.error) reject(new Error(JSON.stringify(msg.error)));
    else resolve(msg.result);
  }
});

await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", (e) => reject(new Error("No se pudo conectar al WebSocket: " + e.message)), { once: true });
});

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
    includeCommandLineAPI: true,
  });
  if (result.exceptionDetails) {
    const details = result.exceptionDetails;
    throw new Error(details.exception?.description || details.text || "Error al evaluar JS");
  }
  return result.result?.value;
}

try {
  await send("Page.enable");

  if (action === "eval") {
    const value = await evaluate(rest.join(" "));
    console.log(typeof value === "string" ? value : JSON.stringify(value, null, 2));
  } else if (action === "click") {
    const selector = JSON.stringify(rest[0]);
    const value = await evaluate(
      `(() => { const el = document.querySelector(${selector}); if (!el) return "NO_ENCONTRADO"; el.click(); return "OK"; })()`
    );
    console.log(value);
  } else if (action === "wait") {
    await new Promise((r) => setTimeout(r, Number(rest[0]) || 500));
    console.log("ok");
  } else if (action === "shot") {
    const { data } = await send("Page.captureScreenshot", { format: "png", fromSurface: true });
    fs.writeFileSync(rest[0], Buffer.from(data, "base64"));
    console.log("guardado " + rest[0]);
  } else if (action === "run") {
    const code = fs.readFileSync(rest[0], "utf8");
    const value = await evaluate(`(async () => { ${code} })()`);
    console.log(typeof value === "string" ? value : JSON.stringify(value, null, 2));
  } else {
    throw new Error("Acción desconocida: " + action);
  }
} catch (err) {
  console.error("ERROR: " + err.message);
  process.exitCode = 1;
} finally {
  ws.close();
}
