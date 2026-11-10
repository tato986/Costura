/* Costura — interfaz local (sin frameworks). */
(function () {
  "use strict";

  const G = window.Costura && window.Costura.garments;
  const tauri = window.__TAURI__;
  const invoke = tauri && tauri.core ? tauri.core.invoke.bind(tauri.core) : null;

  if (!G) {
    document.body.innerHTML = "<p style='padding:40px;font-family:Segoe UI'>No se pudo cargar el catálogo de prendas.</p>";
    return;
  }

  // ─────────────────────────── Constantes ───────────────────────────

  const STATUS = {
    pendiente: { label: "Pendiente", cls: "pendiente" },
    en_corte: { label: "En corte", cls: "en_corte" },
    en_confeccion: { label: "En confección", cls: "en_confeccion" },
    prueba_ajuste: { label: "Prueba y ajuste", cls: "prueba_ajuste" },
    terminado: { label: "Terminado", cls: "terminado" },
    entregado: { label: "Entregado", cls: "entregado" },
  };
  const STATUS_KEYS = ["pendiente", "en_corte", "en_confeccion", "prueba_ajuste", "terminado", "entregado"];

  const PRIORITY = {
    normal: { label: "Normal" },
    urgente: { label: "Urgente" },
    prioritaria: { label: "Prioritaria" },
  };

  const STAGES = [
    ["recepcion", "Recepción"],
    ["corte", "Corte"],
    ["costura", "Costura"],
    ["prueba", "Prueba"],
    ["ajuste", "Ajuste"],
    ["entrega", "Entrega"],
    ["seguimiento", "Seguimiento"],
    ["estado", "Cambio de estado"],
  ];

  const state = {
    ready: false,
    data: null,
    meta: null,
    view: "inicio",
    orderQuery: "",
    orderStatus: "todos",
    clientQuery: "",
    agendaFilter: "pendientes",
    backups: [],
    currentOrderId: null,
    lastView: null,
  };

  let orderForm = null;
  let pendingOrderAfterClient = false;

  // ─────────────────────────── Utilidades ───────────────────────────

  const $ = (sel, root) => (root || document).querySelector(sel);
  const $$ = (sel, root) => Array.from((root || document).querySelectorAll(sel));

  function esc(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  const pad = (n) => String(n).padStart(2, "0");

  function todayStr() {
    const d = new Date();
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }

  function dateOnly(value) {
    const parts = String(value || "").slice(0, 10).split("-").map(Number);
    if (parts.length !== 3 || parts.some((p) => !isFinite(p))) return null;
    return new Date(parts[0], parts[1] - 1, parts[2]);
  }

  function parseDateTime(value) {
    const raw = String(value || "");
    const d = dateOnly(raw);
    if (!d) return null;
    const time = raw.slice(11, 16).split(":").map(Number);
    d.setHours(isFinite(time[0]) ? time[0] : 0, isFinite(time[1]) ? time[1] : 0, 0, 0);
    return d;
  }

  function startOfToday() {
    const d = new Date();
    return new Date(d.getFullYear(), d.getMonth(), d.getDate());
  }

  function daysUntil(value) {
    const d = dateOnly(value);
    if (!d) return null;
    return Math.round((d - startOfToday()) / 86400000);
  }

  const MONTHS = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];

  function fmtDate(value) {
    const d = dateOnly(value);
    if (!d) return "—";
    const sameYear = d.getFullYear() === new Date().getFullYear();
    return `${d.getDate()} ${MONTHS[d.getMonth()]}${sameYear ? "" : " " + d.getFullYear()}`;
  }

  function fmtDateLong(value) {
    const d = dateOnly(value);
    if (!d) return "—";
    return d.toLocaleDateString("es-SV", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  }

  function fmtTime(value) {
    const d = parseDateTime(value);
    if (!d) return "";
    return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  function fmtDateTime(value) {
    const d = String(value || "");
    if (!d) return "—";
    const day = dateOnly(d);
    if (!day) return "—";
    const hasTime = d.length > 10;
    return `${fmtDate(d)}${hasTime ? " · " + fmtTime(d) : ""}`;
  }

  function relDue(value) {
    const diff = daysUntil(value);
    if (diff == null) return "";
    if (diff === 0) return "Hoy";
    if (diff === 1) return "Mañana";
    if (diff === -1) return "Ayer";
    if (diff < 0) return `Atrasado ${Math.abs(diff)} d`;
    if (diff <= 7) return `En ${diff} días`;
    return fmtDate(value);
  }

  function reminderWhen(value) {
    const d = parseDateTime(value);
    if (!d) return "—";
    const diff = daysUntil(String(value).slice(0, 10));
    const time = fmtTime(value);
    if (diff === 0) return `Hoy ${time}`;
    if (diff === 1) return `Mañana ${time}`;
    if (diff === -1) return `Ayer ${time}`;
    if (diff < 0) return `${fmtDate(value)} ${time}`;
    return `${fmtDate(value)} ${time}`;
  }

  function num(value) {
    if (typeof value === "number") return isFinite(value) ? value : 0;
    const clean = String(value == null ? "" : value).replace(/\s/g, "").replace(",", ".");
    const n = parseFloat(clean);
    return isFinite(n) ? n : 0;
  }

  function money(value) {
    const n = num(value);
    const cur = (state.data && state.data.settings && state.data.settings.currency) || "$";
    const formatted = n.toLocaleString("es-SV", { minimumFractionDigits: 0, maximumFractionDigits: 2 });
    return `${cur} ${formatted}`;
  }

  function initials(name) {
    const parts = String(name || "")
      .trim()
      .split(/\s+/)
      .filter(Boolean);
    if (!parts.length) return "?";
    return (parts[0][0] + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase();
  }

  function plural(n, singular, pluralForm) {
    return n === 1 ? singular : pluralForm || singular + "s";
  }

  function statusLabel(key) {
    return (STATUS[key] && STATUS[key].label) || key;
  }

  function stageLabel(key) {
    const found = STAGES.find((s) => s[0] === key);
    if (found) return found[1];
    const label = (STATUS[key] && STATUS[key].label) || key;
    return label;
  }

  function statusPill(key) {
    return `<span class="pill pill-${esc(key)}">${esc(statusLabel(key))}</span>`;
  }

  function prioTag(key) {
    const label = (PRIORITY[key] && PRIORITY[key].label) || key;
    return `<span class="prio prio-${esc(key)}">${esc(label)}</span>`;
  }

  function data() {
    return (
      state.data || {
        customers: [],
        orders: [],
        reminders: [],
        settings: { workshopName: "Mi taller", ownerName: "", currency: "$", defaultDueDays: 7 },
      }
    );
  }

  function customers() {
    return data().customers || [];
  }
  function orders() {
    return data().orders || [];
  }
  function reminders() {
    return data().reminders || [];
  }

  function customerById(id) {
    return customers().find((c) => c.id === id) || null;
  }
  function orderById(id) {
    return orders().find((o) => o.id === id) || null;
  }
  function customerName(id) {
    const c = customerById(id);
    return c ? c.fullName : "Cliente eliminado";
  }

  function balance(order) {
    return Math.max(0, num(order.estimatedCost) - num(order.depositPaid));
  }

  function activeOrders() {
    return orders().filter((o) => o.status !== "entregado");
  }

  function dueBadge(order) {
    const diff = daysUntil(order.dueDate);
    let cls = "ok";
    if (order.status === "entregado") cls = "";
    else if (diff != null && diff < 0) cls = "late";
    else if (diff != null && diff <= 2) cls = "soon";
    const d = dateOnly(order.dueDate);
    const day = d ? d.getDate() : "—";
    const month = d ? MONTHS[d.getMonth()] : "";
    return `<div class="day-badge ${cls}"><div class="d">${day}</div><div class="m">${month}</div></div>`;
  }

  // ─────────────────────────── Llamadas al backend ───────────────────────────

  async function call(command, args) {
    if (!invoke) throw new Error("Esta ventana debe abrirse desde la aplicación Costura.");
    try {
      return await invoke(command, args || {});
    } catch (err) {
      const message = typeof err === "string" ? err : err && err.message ? err.message : String(err);
      toast(message || "Ocurrió un error.", "error", 6000);
      throw err;
    }
  }

  // ─────────────────────────── Avisos ───────────────────────────

  function toast(message, type, ms) {
    const wrap = $("#toastWrap");
    if (!wrap) return;
    const kind = type || "info";
    const icons = { info: "i-bell", success: "i-check", error: "i-alert", warn: "i-alert" };
    const el = document.createElement("div");
    el.className = `toast is-${kind}`;
    el.innerHTML = `<svg><use href="#${icons[kind] || "i-bell"}"/></svg>
      <div class="toast-text">${esc(message)}</div>
      <button class="toast-close" type="button" aria-label="Cerrar aviso"><svg><use href="#i-x"/></svg></button>`;
    el.querySelector(".toast-close").addEventListener("click", () => el.remove());
    wrap.appendChild(el);
    setTimeout(() => el.remove(), ms || 4200);
  }

  // ─────────────────────────── Tema ───────────────────────────

  function applyTheme(theme) {
    document.documentElement.setAttribute("data-theme", theme);
    const icon = $("#themeIcon");
    const label = $("#themeLabel");
    if (icon) icon.innerHTML = `<use href="#${theme === "dark" ? "i-sun" : "i-moon"}"/>`;
    if (label) label.textContent = theme === "dark" ? "Modo claro" : "Modo oscuro";
    try {
      localStorage.setItem("costura-theme", theme);
    } catch (e) {
      /* sin almacenamiento */
    }
  }

  function initTheme() {
    let saved = null;
    try {
      saved = localStorage.getItem("costura-theme");
    } catch (e) {
      saved = null;
    }
    const prefersDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
    applyTheme(saved || (prefersDark ? "dark" : "light"));
  }

  // ─────────────────────────── Arranque ───────────────────────────

  async function boot() {
    initTheme();
    bindGlobalEvents();

    if (!invoke) {
      $("#viewRoot").innerHTML = `<div class="page"><div class="card card-pad">
        <h2 class="page-title">Abre Costura desde su aplicación</h2>
        <p class="page-sub">Esta página necesita el programa de escritorio para leer y guardar tus datos.</p>
        <p class="muted small">Ejecuta <span class="mono">costura.exe</span> (o <span class="mono">cargo run</span> dentro de src-tauri).</p>
      </div></div>`;
      return;
    }

    try {
      const st = await invoke("get_state");
      state.data = st.data;
      state.meta = st;
      state.ready = true;
      $("#appVersion").textContent = "v" + (st.appVersion || "0.1.0");
      render();
      if (st.warning) {
        toast(st.warning, "warn", 11000);
        invoke("dismiss_warning").catch(() => {});
      }
      if (!state.data.customers.length && !state.data.orders.length) {
        toast("Bienvenido. Puedes empezar creando un cliente o cargar datos de ejemplo en Ajustes.", "info", 7000);
      }
    } catch (err) {
      $("#viewRoot").innerHTML = `<div class="page"><div class="card card-pad">
        <h2 class="page-title">No se pudieron cargar los datos</h2>
        <p class="page-sub">${esc(err && err.message ? err.message : err)}</p>
        <button class="btn btn-ghost" onclick="location.reload()">Reintentar</button>
      </div></div>`;
    }
  }

  // ─────────────────────────── Navegación y render ───────────────────────────

  function setView(view) {
    if (!["inicio", "pedidos", "clientes", "agenda", "ajustes"].includes(view)) return;
    state.view = view;
    render();
    const root = $("#viewRoot");
    if (root) root.scrollTop = 0;
  }

  function updateChrome() {
    const s = data().settings;
    const brand = $("#brandName");
    if (brand) brand.textContent = s.workshopName || "Costura";
    const sub = $("#brandSub");
    if (sub) sub.textContent = "Gestión de taller";

    const active = activeOrders().length;
    const badge = $("#navOrdersBadge");
    if (badge) {
      badge.textContent = String(active);
      badge.hidden = active === 0;
    }

    const today = todayStr();
    const pendingToday = reminders().filter((r) => !r.completed && String(r.dueDate).slice(0, 10) <= today).length;
    const agendaBadge = $("#navAgendaBadge");
    if (agendaBadge) {
      agendaBadge.textContent = String(pendingToday);
      agendaBadge.hidden = pendingToday === 0;
    }

    $$(".nav-item").forEach((btn) => {
      btn.classList.toggle("is-active", btn.dataset.view === state.view);
    });
  }

  function render() {
    if (!state.ready) return;
    updateChrome();
    const root = $("#viewRoot");
    if (!root) return;
    const views = {
      inicio: viewHome,
      pedidos: viewOrders,
      clientes: viewClients,
      agenda: viewAgenda,
      ajustes: viewSettings,
    };
    const html = (views[state.view] || viewHome)();
    root.innerHTML = html;

    if (state.view === "ajustes") loadBackups();
    if (state.lastView !== state.view) {
      root.scrollTop = 0;
      state.lastView = state.view;
    }
  }

  // ─────────────────────────── Vista: inicio ───────────────────────────

  function pageHead(eyebrow, title, sub, actions) {
    return `<div class="page-head">
      <div>
        <div class="eyebrow">${esc(eyebrow)}</div>
        <h1 class="page-title">${esc(title)}</h1>
        ${sub ? `<p class="page-sub">${sub}</p>` : ""}
      </div>
      <div class="head-actions">${actions || ""}</div>
    </div>`;
  }

  function stats() {
    const list = orders();
    const active = list.filter((o) => o.status !== "entregado");
    const late = active.filter((o) => (daysUntil(o.dueDate) || 0) < 0).length;
    const week = active.filter((o) => {
      const d = daysUntil(o.dueDate);
      return d != null && d >= 0 && d <= 7;
    }).length;
    const pending = reminders().filter((r) => !r.completed);
    const today = todayStr();
    const todayCount = pending.filter((r) => String(r.dueDate).slice(0, 10) === today).length;
    const overdue = pending.filter((r) => String(r.dueDate).slice(0, 10) < today).length;
    const clientsWithActive = new Set(active.map((o) => o.customerId)).size;
    return { active, late, week, pending, todayCount, overdue, clientsWithActive };
  }

  function emptyBox(icon, title, text, action) {
    return `<div class="empty">
      <div class="empty-icon"><svg><use href="#${icon}"/></svg></div>
      <div class="empty-title">${esc(title)}</div>
      <p class="empty-text">${esc(text)}</p>
      ${action || ""}
    </div>`;
  }

  function viewHome() {
    const s = data().settings;
    const st = stats();
    const now = new Date();
    const hour = now.getHours();
    const greeting = hour < 12 ? "Buenos días" : hour < 19 ? "Buenas tardes" : "Buenas noches";
    const who = (s.ownerName || "").trim();

    const upcoming = activeOrders()
      .filter((o) => {
        const d = daysUntil(o.dueDate);
        return d != null && d <= 7;
      })
      .sort((a, b) => String(a.dueDate).localeCompare(String(b.dueDate)))
      .slice(0, 6);
    const atrasada = upcoming.find((o) => (daysUntil(o.dueDate) || 0) < 0);
    const upcomingFoot = atrasada
      ? `<span class="late">${Math.abs(daysUntil(atrasada.dueDate))} ${plural(Math.abs(daysUntil(atrasada.dueDate)), "día", "días")} de atraso</span>`
      : upcoming.length
        ? "Próxima: " + relDue(upcoming[0].dueDate)
        : "Sin entregas próximas";

    const todayReminders = reminders()
      .filter((r) => !r.completed && String(r.dueDate).slice(0, 10) === todayStr())
      .sort((a, b) => String(a.dueDate).localeCompare(String(b.dueDate)))
      .slice(0, 5);

    const urgent = activeOrders()
      .filter((o) => o.priority !== "normal")
      .sort((a, b) => {
        const rank = { urgente: 0, prioritaria: 1, normal: 2 };
        return (rank[a.priority] || 2) - (rank[b.priority] || 2) || String(a.dueDate).localeCompare(String(b.dueDate));
      })
      .slice(0, 5);

    const heroActions = `<button class="btn btn-primary" data-action="new-order"><svg><use href="#i-plus"/></svg>Nuevo pedido</button>
      <button class="btn" data-action="new-client"><svg><use href="#i-users"/></svg>Nuevo cliente</button>
      <button class="btn" data-action="new-reminder"><svg><use href="#i-bell"/></svg>Recordatorio</button>`;

    const statsHtml = `<div class="stat-grid">
      <div class="stat">
        <div class="stat-top"><span class="stat-label">Pedidos activos</span><span class="stat-icon"><svg><use href="#i-shirt"/></svg></span></div>
        <div class="stat-value">${st.active.length}</div>
        <div class="stat-foot">${st.late ? `<span class="late">${st.late} ${plural(st.late, "atrasado", "atrasados")}</span>` : "Todo al día"}</div>
      </div>
      <div class="stat">
        <div class="stat-top"><span class="stat-label">Entregas esta semana</span><span class="stat-icon is-warn"><svg><use href="#i-clock"/></svg></span></div>
        <div class="stat-value">${st.week}</div>
        <div class="stat-foot">${upcomingFoot}</div>
      </div>
      <div class="stat">
        <div class="stat-top"><span class="stat-label">Clientes</span><span class="stat-icon is-good"><svg><use href="#i-users"/></svg></span></div>
        <div class="stat-value">${customers().length}</div>
        <div class="stat-foot">${st.clientsWithActive} ${plural(st.clientsWithActive, "cliente", "clientes")} con pedidos activos</div>
      </div>
      <div class="stat">
        <div class="stat-top"><span class="stat-label">Recordatorios pendientes</span><span class="stat-icon is-bad"><svg><use href="#i-bell"/></svg></span></div>
        <div class="stat-value">${st.pending.length}</div>
        <div class="stat-foot">${st.todayCount} para hoy${st.overdue ? ` · <span class="late">${st.overdue} vencidos</span>` : ""}</div>
      </div>
    </div>`;

    const upcomingHtml = upcoming.length
      ? `<div class="list">${upcoming
          .map(
            (o) => `<button class="list-row" type="button" data-action="open-order" data-id="${esc(o.id)}">
              ${dueBadge(o)}
              <div class="list-main">
                <div class="list-title">${esc(o.pieceName)}</div>
                <div class="list-sub">${esc(customerName(o.customerId))} · ${esc(statusLabel(o.status))}</div>
              </div>
              <div class="list-side">
                ${o.priority !== "normal" ? prioTag(o.priority) : ""}
                <span class="muted small nowrap">${esc(relDue(o.dueDate))}</span>
              </div>
            </button>`
          )
          .join("")}</div>`
      : emptyBox("i-calendar", "Sin entregas próximas", "Cuando registres pedidos con fecha de entrega aparecerán aquí.", `<button class="btn btn-sm btn-ghost" data-action="new-order">Crear pedido</button>`);

    const todayHtml = todayReminders.length
      ? `<div class="list">${todayReminders
          .map(
            (r) => `<button class="list-row" type="button" data-action="edit-reminder" data-id="${esc(r.id)}">
              <span class="time-chip"><span class="t">${esc(fmtTime(r.dueDate))}</span><span class="l">hoy</span></span>
              <div class="list-main">
                <div class="list-title">${esc(r.title)}</div>
                <div class="list-sub">${r.customerId ? esc(customerName(r.customerId)) : esc(r.description || "Sin cliente")}${r.priority !== "normal" ? " · " + esc((PRIORITY[r.priority] || {}).label || r.priority) : ""}</div>
              </div>
              <div class="list-side"><svg style="width:15px;height:15px;color:var(--muted)"><use href="#i-arrow"/></svg></div>
            </button>`
          )
          .join("")}</div>`
      : emptyBox("i-check", "Nada pendiente para hoy", "Tu agenda de hoy está despejada.");

    const urgentHtml = urgent.length
      ? `<div class="list">${urgent
          .map(
            (o) => `<button class="list-row" type="button" data-action="open-order" data-id="${esc(o.id)}">
              <div class="list-main">
                <div class="list-title">${esc(o.pieceName)}</div>
                <div class="list-sub">${esc(customerName(o.customerId))} · vence ${esc(fmtDate(o.dueDate))}</div>
              </div>
              <div class="list-side">${prioTag(o.priority)}</div>
            </button>`
          )
          .join("")}</div>`
      : emptyBox("i-shirt", "Sin pedidos urgentes", "Los pedidos marcados como urgentes o prioritarios se verán aquí.");

    const onboarding =
      customers().length === 0 && orders().length === 0
        ? `<div class="card" style="margin-bottom:20px">
            <div class="card-pad">
              <h2 class="card-title" style="font-size:16px;margin-bottom:6px"><svg><use href="#i-scissors"/></svg>Empecemos con tu taller</h2>
              <p class="page-sub" style="margin-top:0">Todo se guarda en este equipo. No hay cuentas, ni nube, ni pagos: solo tus clientes, pedidos y agenda.</p>
              <div class="row-wrap" style="margin-top:14px">
                <button class="btn btn-primary" data-action="new-client">1. Crear mi primer cliente</button>
                <button class="btn btn-ghost" data-action="seed-demo">Ver con datos de ejemplo</button>
              </div>
            </div>
          </div>`
        : "";

    return `<div class="page">
      <div class="hero">
        <div class="hero-text">
          <div class="hero-kicker">${esc(fmtDateLong(todayStr()))}</div>
          <h1 class="hero-title">${greeting}${who ? ", " + esc(who) : ""}</h1>
          <p class="hero-sub">${esc(s.workshopName || "Tu taller")} · ${st.active.length} ${plural(st.active.length, "pedido activo", "pedidos activos")} · ${st.pending.length} ${plural(st.pending.length, "recordatorio pendiente", "recordatorios pendientes")}</p>
        </div>
        <div class="hero-actions">${heroActions}</div>
      </div>
      ${onboarding}
      ${statsHtml}
      <div class="grid-2">
        <div class="card">
          <div class="card-head"><div class="card-title"><svg><use href="#i-clock"/></svg>Próximas entregas</div><button class="card-link" data-view="pedidos">Ver todos</button></div>
          ${upcomingHtml}
        </div>
        <div class="stack">
          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-calendar"/></svg>Agenda de hoy</div><button class="card-link" data-view="agenda">Abrir agenda</button></div>
            ${todayHtml}
          </div>
          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-alert"/></svg>Pedidos urgentes</div><button class="card-link" data-view="pedidos">Ver pedidos</button></div>
            ${urgentHtml}
          </div>
        </div>
      </div>
    </div>`;
  }

  // ─────────────────────────── Vista: pedidos ───────────────────────────

  function filteredOrders() {
    const q = state.orderQuery.trim().toLowerCase();
    let list = orders().slice();
    if (state.orderStatus !== "todos") list = list.filter((o) => o.status === state.orderStatus);
    if (q) {
      list = list.filter((o) => {
        const hay = `${o.pieceName} ${o.styleName} ${o.fabricDetails} ${customerName(o.customerId)} ${statusLabel(o.status)}`.toLowerCase();
        return hay.includes(q);
      });
    }
    list.sort((a, b) => {
      const aDone = a.status === "entregado" ? 1 : 0;
      const bDone = b.status === "entregado" ? 1 : 0;
      if (aDone !== bDone) return aDone - bDone;
      return String(a.dueDate).localeCompare(String(b.dueDate));
    });
    return list;
  }

  function viewOrders() {
    const list = filteredOrders();
    const active = activeOrders().length;
    const statusOptions = [["todos", "Todos"], ...STATUS_KEYS.map((k) => [k, STATUS[k].label])]
      .map(([v, l]) => `<option value="${esc(v)}" ${state.orderStatus === v ? "selected" : ""}>${esc(l)}</option>`)
      .join("");

    const rows = list
      .map((o) => {
        const diff = daysUntil(o.dueDate);
        const dueCls = o.status === "entregado" ? "" : diff != null && diff < 0 ? "color:var(--bad);font-weight:600" : "";
        return `<tr data-action="open-order" data-id="${esc(o.id)}">
          <td>
            <div class="cell-main">${esc(o.pieceName)}</div>
            <div class="cell-sub">${esc(G.label(o.garmentType))}${o.styleName ? " · " + esc(o.styleName) : ""}</div>
          </td>
          <td>${esc(customerName(o.customerId))}</td>
          <td>${statusPill(o.status)}</td>
          <td>${prioTag(o.priority)}</td>
          <td class="nowrap" style="${dueCls}">${esc(fmtDate(o.dueDate))}${o.status !== "entregado" && diff != null ? `<div class="cell-sub">${esc(relDue(o.dueDate))}</div>` : ""}</td>
          <td class="num">${esc(money(o.estimatedCost))}</td>
          <td class="num">${balance(o) > 0 ? `<span style="color:var(--warn);font-weight:600">${esc(money(balance(o)))}</span>` : `<span class="muted">—</span>`}</td>
          <td>
            <div class="row-actions">
              <button class="icon-btn" type="button" title="Editar" data-action="edit-order" data-id="${esc(o.id)}"><svg><use href="#i-pencil"/></svg></button>
              <button class="icon-btn danger" type="button" title="Eliminar" data-action="delete-order" data-id="${esc(o.id)}"><svg><use href="#i-trash"/></svg></button>
            </div>
          </td>
        </tr>`;
      })
      .join("");

    return `<div class="page">
      ${pageHead(
        "Pedidos",
        "Pedidos y proyectos",
        `${orders().length} en total · ${active} ${plural(active, "activo", "activos")} · ${orders().length - active} entregados`,
        `<button class="btn btn-primary" data-action="new-order"><svg><use href="#i-plus"/></svg>Nuevo pedido</button>`
      )}
      <div class="toolbar">
        <div class="search">
          <svg><use href="#i-search"/></svg>
          <input class="input" id="orderSearch" type="search" placeholder="Buscar por prenda, cliente o tela…" value="${esc(state.orderQuery)}" />
        </div>
        <select class="select" id="orderStatusFilter" style="width:200px">${statusOptions}</select>
        <span class="muted small">${list.length} ${plural(list.length, "resultado", "resultados")}</span>
        <span class="spacer"></span>
        <button class="btn btn-ghost btn-sm" data-action="clear-filters"><svg><use href="#i-refresh"/></svg>Limpiar filtros</button>
      </div>
      <div class="card">
        ${
          list.length
            ? `<div class="table-wrap"><table class="data">
                <thead><tr><th>Prenda</th><th>Cliente</th><th>Estado</th><th>Prioridad</th><th>Entrega</th><th class="num">Costo</th><th class="num">Saldo</th><th></th></tr></thead>
                <tbody>${rows}</tbody>
              </table></div>`
            : emptyBox(
                "i-shirt",
                orders().length ? "No hay pedidos con ese filtro" : "Todavía no hay pedidos",
                orders().length ? "Prueba con otra búsqueda o cambia el estado." : "Crea tu primer pedido y lleva la bitácora de cada prenda.",
                `<button class="btn btn-primary btn-sm" data-action="new-order">Crear pedido</button>`
              )
        }
      </div>
    </div>`;
  }

  // ─────────────────────────── Vista: clientes ───────────────────────────

  function clientStats(customerId) {
    const list = orders().filter((o) => o.customerId === customerId);
    const active = list.filter((o) => o.status !== "entregado");
    const total = list.reduce((sum, o) => sum + num(o.estimatedCost), 0);
    const due = list.reduce((sum, o) => sum + balance(o), 0);
    return { total: list.length, active: active.length, amount: total, due };
  }

  function viewClients() {
    const q = state.clientQuery.trim().toLowerCase();
    const list = customers()
      .filter((c) => {
        if (!q) return true;
        return `${c.fullName} ${c.phone} ${c.email} ${c.address} ${c.notes}`.toLowerCase().includes(q);
      })
      .sort((a, b) => a.fullName.localeCompare(b.fullName, "es"));

    const cards = list
      .map((c) => {
        const s = clientStats(c.id);
        return `<button class="client-card" type="button" data-action="open-client" data-id="${esc(c.id)}">
          <div class="client-top">
            <div class="avatar">${esc(initials(c.fullName))}</div>
            <div style="min-width:0">
              <div class="client-name">${esc(c.fullName)}</div>
              <div class="client-phone">${esc(c.phone || c.email || "Sin contacto")}</div>
            </div>
          </div>
          <div class="client-notes">${esc(c.notes || "")}</div>
          <div class="client-stats">
            <span><strong>${s.total}</strong> ${plural(s.total, "pedido", "pedidos")}</span>
            <span><strong>${s.active}</strong> ${plural(s.active, "activo", "activos")}</span>
            <span><strong>${esc(money(s.due))}</strong> por cobrar</span>
          </div>
        </button>`;
      })
      .join("");

    return `<div class="page">
      ${pageHead(
        "Clientes",
        "Clientes del taller",
        `${customers().length} ${plural(customers().length, "cliente registrado", "clientes registrados")}`,
        `<button class="btn btn-primary" data-action="new-client"><svg><use href="#i-plus"/></svg>Nuevo cliente</button>`
      )}
      <div class="toolbar">
        <div class="search">
          <svg><use href="#i-search"/></svg>
          <input class="input" id="clientSearch" type="search" placeholder="Buscar cliente por nombre, teléfono o correo…" value="${esc(state.clientQuery)}" />
        </div>
        <span class="muted small">${list.length} ${plural(list.length, "cliente", "clientes")}</span>
      </div>
      ${
        list.length
          ? `<div class="client-grid">${cards}</div>`
          : emptyBox(
              "i-users",
              customers().length ? "Ningún cliente coincide" : "Aún no hay clientes",
              customers().length ? "Prueba con otro nombre o teléfono." : "Registra a tu primera clienta o cliente para empezar a tomar pedidos.",
              `<button class="btn btn-primary btn-sm" data-action="new-client">Crear cliente</button>`
            )
      }
    </div>`;
  }

  // ─────────────────────────── Vista: agenda ───────────────────────────

  function agendaGroups() {
    const pending = reminders().filter((r) => !r.completed);
    const done = reminders().filter((r) => r.completed);
    const today = todayStr();
    const group = (r) => {
      const d = String(r.dueDate).slice(0, 10);
      if (d < today) return "atrasados";
      if (d === today) return "hoy";
      const diff = daysUntil(d);
      if (diff === 1) return "manana";
      if (diff != null && diff <= 7) return "semana";
      return "despues";
    };
    const sort = (a, b) => String(a.dueDate).localeCompare(String(b.dueDate));
    const buckets = { atrasados: [], hoy: [], manana: [], semana: [], despues: [] };
    pending.forEach((r) => buckets[group(r)].push(r));
    Object.keys(buckets).forEach((k) => buckets[k].sort(sort));
    done.sort((a, b) => String(b.dueDate).localeCompare(String(a.dueDate)));
    return { buckets, done, pending };
  }

  function reminderItem(r) {
    const order = r.orderId ? orderById(r.orderId) : null;
    const customer = r.customerId ? customerById(r.customerId) : null;
    const meta = [reminderWhen(r.dueDate)];
    if (customer) meta.push(customer.fullName);
    if (order) meta.push(order.pieceName);
    const late = !r.completed && String(r.dueDate) < todayStr() + "T00:00";
    return `<div class="reminder ${r.completed ? "is-done" : ""}">
      <button class="check-circle" type="button" title="${r.completed ? "Marcar pendiente" : "Marcar completado"}" data-action="toggle-reminder" data-id="${esc(r.id)}">
        <svg><use href="#i-check"/></svg>
      </button>
      <div class="reminder-body">
        <div class="reminder-title">${esc(r.title)}</div>
        <div class="reminder-meta">
          <span style="${late ? "color:var(--bad);font-weight:600" : ""}">${esc(meta[0])}</span>
          ${meta.slice(1).map((m) => `<span>· ${esc(m)}</span>`).join("")}
          ${r.priority !== "normal" ? prioTag(r.priority) : ""}
        </div>
        ${r.description ? `<div class="cell-sub" style="margin-top:3px">${esc(r.description)}</div>` : ""}
      </div>
      <div class="reminder-actions">
        <button class="icon-btn" type="button" title="Editar" data-action="edit-reminder" data-id="${esc(r.id)}"><svg><use href="#i-pencil"/></svg></button>
        <button class="icon-btn danger" type="button" title="Eliminar" data-action="delete-reminder" data-id="${esc(r.id)}"><svg><use href="#i-trash"/></svg></button>
      </div>
    </div>`;
  }

  function viewAgenda() {
    const { buckets, done, pending } = agendaGroups();
    const filter = state.agendaFilter;
    const titles = { atrasados: "Atrasados", hoy: "Hoy", manana: "Mañana", semana: "Esta semana", despues: "Más adelante" };

    const pendingHtml = Object.keys(buckets)
      .filter((k) => buckets[k].length && (filter === "pendientes" || filter === "todos"))
      .map(
        (k) =>
          `<div class="group-title ${k === "atrasados" ? "overdue" : k === "hoy" ? "today" : ""}">${titles[k]}<span class="count">${buckets[k].length}</span></div>` +
          buckets[k].map(reminderItem).join("")
      )
      .join("");

    const doneHtml =
      filter !== "pendientes" && done.length
        ? `<div class="group-title">Completados<span class="count">${done.length}</span></div>` + done.map(reminderItem).join("")
        : "";

    const nothing = filter === "pendientes" ? !pending.length : !pending.length && !done.length;

    return `<div class="page">
      ${pageHead(
        "Agenda",
        "Recordatorios y citas",
        `${pending.length} ${plural(pending.length, "pendiente", "pendientes")} · ${done.length} ${plural(done.length, "completado", "completados")}`,
        `<button class="btn btn-primary" data-action="new-reminder"><svg><use href="#i-plus"/></svg>Nuevo recordatorio</button>`
      )}
      <div class="toolbar">
        <div class="chips">
          <button class="chip ${filter === "pendientes" ? "is-active" : ""}" data-action="agenda-filter" data-filter="pendientes">Pendientes<span class="chip-count">${pending.length}</span></button>
          <button class="chip ${filter === "completados" ? "is-active" : ""}" data-action="agenda-filter" data-filter="completados">Completados<span class="chip-count">${done.length}</span></button>
          <button class="chip ${filter === "todos" ? "is-active" : ""}" data-action="agenda-filter" data-filter="todos">Todos<span class="chip-count">${pending.length + done.length}</span></button>
        </div>
        <span class="spacer"></span>
        <span class="muted small">Se ordenan por fecha y hora</span>
      </div>
      ${
        nothing
          ? `<div class="card">${emptyBox(
              "i-calendar",
              "Tu agenda está vacía",
              "Agrega pruebas, entregas o compras de materiales para no olvidarlas.",
              `<button class="btn btn-primary btn-sm" data-action="new-reminder">Crear recordatorio</button>`
            )}</div>`
          : `<div>${pendingHtml}${doneHtml}</div>`
      }
    </div>`;
  }

  // ─────────────────────────── Vista: ajustes ───────────────────────────

  function viewSettings() {
    const s = data().settings;
    const meta = state.meta || {};
    const rows = state.backups.length
      ? state.backups
          .slice(0, 12)
          .map(
            (b) => `<div class="backup-row">
              <div class="backup-main">
                <div class="backup-name">${esc(b.file)}</div>
                <div class="backup-meta">${esc(fmtDateTime(b.createdAt))} · ${Math.max(1, Math.round((b.sizeBytes || 0) / 1024))} KB</div>
              </div>
              ${b.automatic ? `<span class="badge-auto">auto</span>` : ""}
              <button class="btn btn-sm btn-ghost" data-action="backup-restore" data-file="${esc(b.file)}">Restaurar</button>
            </div>`
          )
          .join("")
      : `<p class="muted small" style="margin:8px 0">Todavía no hay respaldos. Crea uno antes de hacer cambios grandes.</p>`;

    return `<div class="page">
      ${pageHead("Ajustes", "Configuración del taller", "Todo es local: no se envía nada a internet.", "")}
      <div class="settings-grid">
        <div class="stack">
          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-scissors"/></svg>Datos del taller</div></div>
            <div class="card-pad">
              <form data-form="settings" class="form-grid" autocomplete="off">
                <div class="field full"><label class="label" for="wsName">Nombre del taller</label>
                  <input class="input" id="wsName" name="workshopName" maxlength="80" required value="${esc(s.workshopName || "")}" /></div>
                <div class="field"><label class="label" for="wsOwner">Tu nombre</label>
                  <input class="input" id="wsOwner" name="ownerName" maxlength="80" value="${esc(s.ownerName || "")}" placeholder="Para el saludo de inicio" /></div>
                <div class="field"><label class="label" for="wsCurrency">Símbolo de moneda</label>
                  <input class="input" id="wsCurrency" name="currency" maxlength="6" value="${esc(s.currency || "$")}" /></div>
                <div class="field"><label class="label" for="wsDays">Días de entrega por defecto</label>
                  <input class="input" id="wsDays" name="defaultDueDays" type="number" min="1" max="365" value="${esc(s.defaultDueDays || 7)}" /></div>
                <div class="field full"><button class="btn btn-primary" type="submit" style="width:auto;align-self:flex-start"><svg><use href="#i-check"/></svg>Guardar ajustes</button></div>
              </form>
            </div>
          </div>

          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-database"/></svg>Dónde se guardan tus datos</div></div>
            <div class="card-pad">
              <p class="setting-note">El archivo se guarda en tu equipo. Puedes copiarlo a un USB como respaldo manual.</p>
              <code class="path">${esc(meta.dataFile || "—")}</code>
              <div class="row-wrap" style="margin-top:12px">
                <button class="btn btn-ghost btn-sm" data-action="open-folder" data-folder="data"><svg><use href="#i-folder"/></svg>Abrir carpeta de datos</button>
                <button class="btn btn-ghost btn-sm" data-action="open-folder" data-folder="backups"><svg><use href="#i-folder"/></svg>Abrir respaldos</button>
              </div>
            </div>
          </div>

          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-scissors"/></svg>Acerca de</div><span class="muted tiny">v${esc(meta.appVersion || "0.1.0")}</span></div>
            <div class="card-pad">
              <p class="setting-note" style="margin-top:0">Costura es una aplicación local para llevar clientes, pedidos con medidas, agenda y bitácora de cada prenda. No requiere internet, cuenta ni contraseña.</p>
              <p class="setting-note">Hecha en Rust con Tauri; la interfaz se ejecuta dentro de la app.</p>
            </div>
          </div>
        </div>

        <div class="stack">
          <div class="card">
            <div class="card-head"><div class="card-title"><svg><use href="#i-database"/></svg>Respaldos</div><button class="btn btn-sm btn-ghost" data-action="backup-create"><svg><use href="#i-plus"/></svg>Crear respaldo</button></div>
            <div class="card-pad" id="backupList">
              ${rows}
              <p class="setting-note" style="margin:10px 0 0">Se crea un respaldo automático una vez al día y antes de restaurar, cargar el ejemplo o borrar todo. Se conservan los últimos 40.</p>
            </div>
          </div>

          <div class="card danger-zone">
            <div class="card-head"><div class="card-title"><svg><use href="#i-alert"/></svg>Zona delicada</div></div>
            <div class="card-pad stack" style="gap:12px">
              <div>
                <p class="setting-note" style="margin-top:0">Cargar los datos de ejemplo reemplaza lo que tengas ahora (se guarda un respaldo antes).</p>
                <button class="btn btn-ghost btn-sm" data-action="seed-demo"><svg><use href="#i-refresh"/></svg>Cargar datos de ejemplo</button>
              </div>
              <hr class="divider" />
              <div>
                <p class="setting-note" style="margin-top:0">Borrar todo deja la aplicación vacía. Los respaldos anteriores se conservan.</p>
                <button class="btn btn-danger btn-sm" data-action="reset-all"><svg><use href="#i-trash"/></svg>Borrar todos los datos</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>`;
  }

  async function loadBackups() {
    try {
      state.backups = await invoke("list_backups");
      const el = $("#backupList");
      if (!el) return;
      const rows = state.backups.length
        ? state.backups
            .slice(0, 12)
            .map(
              (b) => `<div class="backup-row">
                <div class="backup-main">
                  <div class="backup-name">${esc(b.file)}</div>
                  <div class="backup-meta">${esc(fmtDateTime(b.createdAt))} · ${Math.max(1, Math.round((b.sizeBytes || 0) / 1024))} KB</div>
                </div>
                ${b.automatic ? `<span class="badge-auto">auto</span>` : ""}
                <button class="btn btn-sm btn-ghost" data-action="backup-restore" data-file="${esc(b.file)}">Restaurar</button>
              </div>`
            )
            .join("")
        : `<p class="muted small" style="margin:8px 0">Todavía no hay respaldos. Crea uno antes de hacer cambios grandes.</p>`;
      el.innerHTML =
        rows +
        `<p class="setting-note" style="margin:10px 0 0">Se crea un respaldo automático una vez al día y antes de restaurar, cargar el ejemplo o borrar todo. Se conservan los últimos 40.</p>`;
    } catch (e) {
      /* el aviso ya se mostró */
    }
  }

  // ─────────────────────────── Modales ───────────────────────────

  function openModal(html, opts) {
    const dlg = $("#modal");
    const wide = !!(opts && opts.wide);
    dlg.className = "modal" + (wide ? " wide" : "");
    dlg.innerHTML = html;
    const body = dlg.querySelector(".modal-body");
    if (body) body.scrollTop = 0;
    if (!dlg.open) dlg.showModal();
    const focusable = dlg.querySelector(
      ".modal-body input:not([type=hidden]), .modal-body select, .modal-body textarea, .modal-body button"
    );
    if (focusable) setTimeout(() => focusable.focus(), 30);
  }

  function closeModal() {
    const dlg = $("#modal");
    if (dlg && dlg.open) dlg.close();
    state.currentOrderId = null;
    orderForm = null;
  }

  function confirmDialog(opts) {
    const dlg = $("#confirm");
    return new Promise((resolve) => {
      let settled = false;
      const finish = (value) => {
        if (settled) return;
        settled = true;
        try {
          dlg.close();
        } catch (e) {
          /* ya cerrado */
        }
        resolve(value);
      };

      dlg.innerHTML = `<div class="modal-card">
        <div class="modal-head"><div><h2 class="modal-title">${esc(opts.title || "Confirmar")}</h2></div></div>
        <div class="modal-body"><p class="confirm-text">${esc(opts.message || "")}</p></div>
        <div class="modal-foot">
          <button class="btn btn-ghost" type="button" data-confirm="no">Cancelar</button>
          <button class="btn ${opts.danger ? "btn-danger" : "btn-primary"}" type="button" data-confirm="yes">${esc(opts.confirmLabel || "Continuar")}</button>
        </div>
      </div>`;

      dlg.querySelector('[data-confirm="yes"]').addEventListener("click", () => finish(true));
      dlg.querySelector('[data-confirm="no"]').addEventListener("click", () => finish(false));
      dlg.oncancel = (e) => {
        e.preventDefault();
        finish(false);
      };
      dlg.onclick = (e) => {
        if (e.target === dlg) finish(false);
      };
      dlg.showModal();
      dlg.querySelector('[data-confirm="yes"]').focus();
    });
  }

  // ─────────────────────────── Formulario de pedido ───────────────────────────

  function newOrderState(order, presetCustomerId) {
    const form = {
      editingId: order ? order.id : null,
      type: (order && order.garmentType) || "camisa",
      values: {},
      options: {},
      style: (order && order.styleName) || "",
      measureNotes: (order && order.measurements && order.measurements.notes) || "",
      customerId: (order && order.customerId) || presetCustomerId || (customers()[0] ? customers()[0].id : ""),
    };
    ["camisa", "pantalon", "vestido", "bolso"].forEach((k) => {
      form.values[k] = G.defaultValues(k);
      form.options[k] = G.defaultOptions(k);
    });
    if (order && order.measurements) {
      Object.assign(form.values[form.type], order.measurements.values || {});
      Object.assign(form.options[form.type], order.measurements.options || {});
    }
    if (!form.style) {
      const styles = G.styles(form.type);
      form.style = styles[0] || "";
    }
    return form;
  }

  function openOrderForm(orderId, presetCustomerId) {
    if (!customers().length) {
      toast("Primero registra un cliente para poder crear el pedido.", "warn");
      pendingOrderAfterClient = true;
      openClientForm(null);
      return;
    }
    const order = orderId ? orderById(orderId) : null;
    if (orderId && !order) {
      toast("El pedido ya no existe.", "error");
      return;
    }
    orderForm = newOrderState(order, presetCustomerId);

    const s = data().settings;
    const days = order ? 0 : Number(s.defaultDueDays) || 7;
    const due = order ? order.dueDate : addDays(days);
    const priority = order ? order.priority : "normal";

    const customerOptions = customers()
      .slice()
      .sort((a, b) => a.fullName.localeCompare(b.fullName, "es"))
      .map((c) => `<option value="${esc(c.id)}" ${orderForm.customerId === c.id ? "selected" : ""}>${esc(c.fullName)}</option>`)
      .join("");

    const typeCards = ["camisa", "pantalon", "vestido", "bolso"]
      .map((k) => `<button type="button" class="type-card ${k === orderForm.type ? "is-active" : ""}" data-action="order-type" data-type="${k}">
        <svg><use href="#${G.GARMENTS[k].icon}"/></svg><span>${esc(G.label(k))}</span>
      </button>`)
      .join("");

    const html = `<form data-form="order" autocomplete="off">
      <div class="modal-card">
        <div class="modal-head">
          <div>
            <h2 class="modal-title">${order ? "Editar pedido" : "Nuevo pedido"}</h2>
            <p class="modal-sub">${order ? "Actualiza los datos, medidas o la tela de esta prenda." : "Registra la prenda, sus medidas y el anticipo."}</p>
          </div>
          <button class="icon-btn modal-close" type="button" data-action="close-modal" aria-label="Cerrar"><svg><use href="#i-x"/></svg></button>
        </div>
        <div class="modal-body">
          <div class="order-layout">
            <div>
              <div class="form-section">
                <div class="form-section-title"><svg><use href="#i-users"/></svg>Cliente y prenda</div>
                <div class="form-grid">
                  <div class="field full">
                    <label class="label" for="orderCustomer">Cliente</label>
                    <select class="select" id="orderCustomer" name="customerId" required>${customerOptions}</select>
                    <span class="hint">¿Falta alguien? Créalo en la sección Clientes.</span>
                  </div>
                  <div class="field full">
                    <label class="label" for="orderPiece">Prenda o proyecto</label>
                    <input class="input" id="orderPiece" name="pieceName" maxlength="160" required placeholder="Ej. Camisa de lino blanca" value="${esc(order ? order.pieceName : "")}" />
                  </div>
                  <div class="field full">
                    <span class="label">Tipo de prenda</span>
                    <div class="type-grid">${typeCards}</div>
                  </div>
                  <div class="field">
                    <label class="label" for="orderStyle">Estilo</label>
                    <select class="select" id="orderStyle" name="styleName"></select>
                  </div>
                  <div class="field">
                    <label class="label" for="orderPriority">Prioridad</label>
                    <select class="select" id="orderPriority" name="priority">
                      ${Object.keys(PRIORITY).map((p) => `<option value="${p}" ${priority === p ? "selected" : ""}>${PRIORITY[p].label}</option>`).join("")}
                    </select>
                  </div>
                  <div class="field">
                    <label class="label" for="orderDue">Fecha de entrega</label>
                    <input class="input" id="orderDue" name="dueDate" type="date" required value="${esc(due)}" />
                  </div>
                  <div class="field">
                    <label class="label" for="orderDeposit">Anticipo recibido</label>
                    <div class="amount-input"><span class="cur">${esc(s.currency || "$")}</span><input class="input" id="orderDeposit" name="depositPaid" type="number" min="0" step="0.01" value="${esc(order ? order.depositPaid : 0)}" /></div>
                  </div>
                </div>
              </div>

              <div class="form-section">
                <div class="form-section-title"><svg><use href="#i-ruler"/></svg>Medidas <span class="muted tiny" style="text-transform:none;letter-spacing:0;font-weight:500">en centímetros</span></div>
                <div id="measureArea"></div>
                <div class="field">
                  <label class="label" for="measureNotes">Notas de medidas o ajustes</label>
                  <textarea class="textarea" id="measureNotes" name="measureNotes" rows="2" maxlength="1000" placeholder="Ej. caída ancha, doble pinza, ajuste en cintura…">${esc(orderForm.measureNotes)}</textarea>
                </div>
              </div>

              <div class="form-section">
                <div class="form-section-title"><svg><use href="#i-scissors"/></svg>Tela, costo y notas</div>
                <div class="field">
                  <label class="label" for="orderFabric">Tela y detalles</label>
                  <textarea class="textarea" id="orderFabric" name="fabricDetails" rows="2" maxlength="1500" placeholder="Tela, color, forro, botones, requerimientos especiales…">${esc(order ? order.fabricDetails : "")}</textarea>
                </div>
                <div class="form-grid">
                  <div class="field">
                    <label class="label" for="orderCost">Costo total</label>
                    <div class="amount-input"><span class="cur">${esc(s.currency || "$")}</span><input class="input" id="orderCost" name="estimatedCost" type="number" min="0" step="0.01" value="${esc(order ? order.estimatedCost : 0)}" /></div>
                  </div>
                  <div class="field">
                    <label class="label" for="orderNote">Nota inicial</label>
                    <input class="input" id="orderNote" name="initialNote" maxlength="2000" placeholder="Ej. se tomó medida con ropa delgada" value="" />
                  </div>
                </div>
              </div>
            </div>

            <div class="preview-panel">
              <div class="preview-card">
                <div class="preview-svg" id="previewSvg"></div>
                <div class="preview-caption" id="previewCaption"></div>
              </div>
              <div class="preview-card">
                <div class="preview-summary" id="previewSummary"></div>
              </div>
            </div>
          </div>
        </div>
        <div class="modal-foot">
          <span class="muted small spread">Los datos se guardan solo en este equipo.</span>
          <button class="btn btn-ghost" type="button" data-action="close-modal">Cancelar</button>
          <button class="btn btn-primary" type="submit"><svg><use href="#i-check"/></svg>${order ? "Guardar cambios" : "Crear pedido"}</button>
        </div>
      </div>
    </form>`;

    openModal(html, { wide: true });
    renderOrderTypeAreas();
    renderOrderMeasureArea();
    renderOrderPreview();
  }

  function addDays(days) {
    const d = new Date();
    d.setDate(d.getDate() + Number(days || 0));
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }

  function renderOrderTypeAreas() {
    const styleSelect = $("#orderStyle");
    if (styleSelect) {
      const styles = G.styles(orderForm.type).slice();
      if (orderForm.style && !styles.includes(orderForm.style)) styles.push(orderForm.style);
      if (!orderForm.style) orderForm.style = styles[0] || "";
      styleSelect.innerHTML = styles.map((s) => `<option value="${esc(s)}" ${s === orderForm.style ? "selected" : ""}>${esc(s)}</option>`).join("");
    }
    $$("#modal .type-card").forEach((card) => {
      card.classList.toggle("is-active", card.dataset.type === orderForm.type);
    });
  }

  function renderOrderMeasureArea() {
    const area = $("#measureArea");
    if (!area) return;
    const type = orderForm.type;
    const spec = G.GARMENTS[type];
    const values = orderForm.values[type];

    const sliders = spec.fields
      .map((f) => {
        const value = Math.round(num(values[f.key]));
        const pct = Math.round(((value - f.min) / (f.max - f.min)) * 100);
        return `<div class="measure-row">
          <div class="measure-top"><span class="measure-name">${esc(f.label)}</span><span class="measure-val" data-value-for="${f.key}">${value} cm</span></div>
          <input type="range" min="${f.min}" max="${f.max}" step="1" value="${value}" data-measure="${f.key}" style="--fill:${pct}%" aria-label="${esc(f.label)}" />
        </div>`;
      })
      .join("");

    const options = spec.options
      .map((opt) => {
        const current = orderForm.options[type][opt.key] || opt.choices[0];
        return `<div class="field"><label class="label" for="opt-${type}-${opt.key}">${esc(opt.label)}</label>
          <select class="select" id="opt-${type}-${opt.key}" data-option="${opt.key}">
            ${opt.choices.map((c) => `<option value="${esc(c)}" ${c === current ? "selected" : ""}>${esc(c)}</option>`).join("")}
          </select>
        </div>`;
      })
      .join("");

    const presets = G.presets(type);
    const presetRow = presets
      ? `<div class="preset-row" style="margin-bottom:4px">
          ${Object.keys(presets)
            .map((name) => `<button class="preset" type="button" data-action="apply-preset" data-preset="${esc(name)}">${esc(name)}</button>`)
            .join("")}
        </div>`
      : "";

    area.innerHTML = `<div class="measure-grid">${sliders}</div>
      <div class="form-grid" style="margin-top:12px">${options}</div>
      ${presetRow}`;
  }

  function renderOrderPreview() {
    const svg = $("#previewSvg");
    const caption = $("#previewCaption");
    const summary = $("#previewSummary");
    if (!svg || !orderForm) return;

    const type = orderForm.type;
    const options = Object.assign({}, orderForm.options[type], { estilo: orderForm.style });
    svg.innerHTML = G.render(type, orderForm.values[type], options, { labels: false });
    if (caption) caption.textContent = `${G.label(type)}${orderForm.style ? " · " + orderForm.style : ""}`;

    const form = $("#modal form[data-form=order]");
    const cost = form ? num(form.querySelector('[name="estimatedCost"]').value) : 0;
    const deposit = form ? num(form.querySelector('[name="depositPaid"]').value) : 0;
    const saldo = Math.max(0, cost - deposit);
    if (summary) {
      summary.innerHTML = `<div class="preview-line"><span>Tipo</span><span>${esc(G.label(type))}</span></div>
        <div class="preview-line"><span>Estilo</span><span>${esc(orderForm.style || "—")}</span></div>
        <div class="preview-line"><span>Costo</span><span>${esc(money(cost))}</span></div>
        <div class="preview-line"><span>Anticipo</span><span>${esc(money(deposit))}</span></div>
        <div class="preview-line preview-total"><span>Saldo</span><span>${esc(money(saldo))}</span></div>`;
    }
  }

  function readOrderForm(form) {
    const fd = new FormData(form);
    const type = orderForm.type;
    return {
      id: orderForm.editingId,
      customerId: String(fd.get("customerId") || ""),
      pieceName: String(fd.get("pieceName") || "").trim(),
      garmentType: type,
      styleName: orderForm.style,
      status: orderForm.editingId ? (orderById(orderForm.editingId) || {}).status : "pendiente",
      priority: String(fd.get("priority") || "normal"),
      measurements: {
        kind: type,
        values: orderForm.values[type],
        options: orderForm.options[type],
        notes: String(fd.get("measureNotes") || "").trim(),
      },
      fabricDetails: String(fd.get("fabricDetails") || "").trim(),
      estimatedCost: num(fd.get("estimatedCost")),
      depositPaid: num(fd.get("depositPaid")),
      dueDate: String(fd.get("dueDate") || ""),
      initialNote: String(fd.get("initialNote") || "").trim(),
    };
  }

  // ─────────────────────────── Detalle de pedido ───────────────────────────

  function orderDetailHtml(order) {
    const customer = customerById(order.customerId);
    const currentIndex = STATUS_KEYS.indexOf(order.status);
    const steps = STATUS_KEYS.map((key, i) => {
      const cls = i < currentIndex ? "done" : i === currentIndex ? "current" : "";
      return `<button class="step ${cls}" type="button" data-action="set-status" data-status="${key}" data-id="${esc(order.id)}" title="Marcar como ${esc(statusLabel(key))}">
        <span class="step-dot">${i < currentIndex ? '<svg><use href="#i-check"/></svg>' : ""}</span>
        <span><span class="step-label">${esc(statusLabel(key))}</span></span>
      </button>`;
    }).join("");

    const meas = G.measureSummary(order.garmentType, (order.measurements && order.measurements.values) || {});
    const spec = G.GARMENTS[order.garmentType];
    const opts = spec
      ? spec.options
          .map((opt) => ({ label: opt.label, value: (order.measurements.options || {})[opt.key] }))
          .filter((o) => o.value)
      : [];

    const measHtml = meas.length
      ? `<div class="kv">${meas
          .map((m) => `<div class="kv-row"><span class="kv-key">${esc(m.label)}</span><span class="kv-val">${Math.round(m.value)} cm</span></div>`)
          .join("")}</div>`
      : `<p class="muted small">Sin medidas registradas.</p>`;

    const optsHtml = opts.length
      ? `<div class="row-wrap" style="margin-top:12px">${opts.map((o) => `<span class="tag">${esc(o.label)}: ${esc(o.value)}</span>`).join("")}</div>`
      : "";

    const notes = (order.notes || []).slice().sort((a, b) => String(b.timestamp).localeCompare(String(a.timestamp)));
    const notesHtml = notes.length
      ? notes
          .map(
            (n) => `<div class="note">
              <div class="note-head">
                <span class="note-stage">${esc(stageLabel(n.stage))}</span>
                <span class="row" style="gap:6px"><span class="note-time">${esc(fmtDateTime(n.timestamp))}</span>
                  <button class="icon-btn danger note-del" type="button" title="Eliminar nota" data-action="delete-note" data-order="${esc(order.id)}" data-note="${esc(n.id)}" style="width:22px;height:22px"><svg><use href="#i-x"/></svg></button>
                </span>
              </div>
              <div class="note-text">${esc(n.content)}</div>
            </div>`
          )
          .join("")
      : `<p class="muted small">Sin notas todavía. Agrega la primera abajo.</p>`;

    const bal = balance(order);
    const paid = bal <= 0.001;

    return `<div class="modal-card">
      <div class="modal-head">
        <div style="min-width:0">
          <h2 class="modal-title">${esc(order.pieceName)}</h2>
          <p class="modal-sub">${esc(customerName(order.customerId))}${customer && customer.phone ? " · " + esc(customer.phone) : ""} · ${esc(G.label(order.garmentType))}${order.styleName ? " · " + esc(order.styleName) : ""}</p>
          <div class="detail-head-info">
            ${statusPill(order.status)}
            ${prioTag(order.priority)}
            <span class="tag"><svg style="width:14px;height:14px"><use href="#i-calendar"/></svg>${esc(fmtDate(order.dueDate))}</span>
            ${order.deliveryDate ? `<span class="tag"><svg style="width:14px;height:14px"><use href="#i-check"/></svg>Entregado ${esc(fmtDate(order.deliveryDate))}</span>` : ""}
          </div>
        </div>
        <button class="icon-btn modal-close" type="button" data-action="close-modal" aria-label="Cerrar"><svg><use href="#i-x"/></svg></button>
      </div>
      <div class="modal-body">
        <div class="detail-grid">
          <div>
            <div class="card card-pad">
              <div class="form-section-title" style="margin-bottom:12px"><svg><use href="#i-clock"/></svg>Etapas del pedido</div>
              <div class="steps">${steps}</div>
            </div>

            <div class="card card-pad" style="margin-top:14px">
              <div class="form-section-title" style="margin-bottom:12px"><svg><use href="#i-wallet"/></svg>Costos y datos</div>
              <div class="kv">
                <div class="kv-row"><span class="kv-key">Costo total</span><span class="kv-val">${esc(money(order.estimatedCost))}</span></div>
                <div class="kv-row"><span class="kv-key">Anticipo</span><span class="kv-val">${esc(money(order.depositPaid))}</span></div>
                <div class="kv-row"><span class="kv-key">Fecha de entrega</span><span class="kv-val">${esc(fmtDate(order.dueDate))} <span class="muted small">${esc(relDue(order.dueDate))}</span></span></div>
                <div class="kv-row"><span class="kv-key">Creado</span><span class="kv-val">${esc(fmtDate(order.createdAt))}</span></div>
                <div class="kv-row"><span class="kv-key">Tipo</span><span class="kv-val">${esc(G.label(order.garmentType))}</span></div>
                <div class="kv-row"><span class="kv-key">Estilo</span><span class="kv-val">${esc(order.styleName || "—")}</span></div>
              </div>
              <div class="balance-box ${paid ? "is-paid" : ""}" style="margin-top:14px">
                <div><div class="balance-label">${paid ? "Pagado" : "Saldo pendiente"}</div><div class="balance-value">${esc(money(bal))}</div></div>
                <div class="muted small right">${paid ? "Anticipo completo" : "Costo − anticipo"}</div>
              </div>
              ${order.fabricDetails ? `<div style="margin-top:14px"><div class="kv-key">Tela y detalles</div><p class="small" style="margin:4px 0 0;white-space:pre-wrap">${esc(order.fabricDetails)}</p></div>` : ""}
            </div>

            <div class="card" style="margin-top:14px">
              <div class="card-head"><div class="card-title"><svg><use href="#i-scissors"/></svg>Bitácora</div><span class="muted tiny">${notes.length} ${plural(notes.length, "nota", "notas")}</span></div>
              <div class="card-pad">
                <div class="note-list">${notesHtml}</div>
                <form data-form="note" data-order="${esc(order.id)}" style="margin-top:12px" autocomplete="off">
                  <div class="form-row" style="align-items:flex-end">
                    <div class="field" style="flex:0 0 150px">
                      <label class="label" for="noteStage">Etapa</label>
                      <select class="select" id="noteStage" name="stage">
                        ${STAGES.filter((s) => s[0] !== "estado").map(([v, l]) => `<option value="${v}">${esc(l)}</option>`).join("")}
                      </select>
                    </div>
                    <div class="field">
                      <label class="label" for="noteContent">Nueva nota</label>
                      <input class="input" id="noteContent" name="content" maxlength="2000" placeholder="Ej. se ajustó 2 cm la cintura" required />
                    </div>
                    <button class="btn btn-primary" type="submit" style="flex:none"><svg><use href="#i-plus"/></svg>Agregar</button>
                  </div>
                </form>
              </div>
            </div>
          </div>

          <div>
            <div class="preview-card">
              <div class="preview-svg">${G.render(order.garmentType, order.measurements.values || {}, Object.assign({}, order.measurements.options || {}, { estilo: order.styleName }), { labels: true })}</div>
              <div class="preview-caption">${esc(G.label(order.garmentType))} · medidas en cm</div>
            </div>
            <div class="card card-pad" style="margin-top:14px">
              <div class="form-section-title" style="margin-bottom:12px"><svg><use href="#i-ruler"/></svg>Medidas</div>
              ${measHtml}
              ${optsHtml}
              ${order.measurements.notes ? `<p class="small muted" style="margin:10px 0 0">${esc(order.measurements.notes)}</p>` : ""}
            </div>
          </div>
        </div>
      </div>
      <div class="modal-foot">
        <button class="btn btn-ghost" type="button" data-action="print-order" data-id="${esc(order.id)}"><svg><use href="#i-printer"/></svg>Imprimir ficha</button>
        <span class="spacer"></span>
        <button class="btn btn-ghost" type="button" data-action="edit-order" data-id="${esc(order.id)}"><svg><use href="#i-pencil"/></svg>Editar</button>
        <button class="btn btn-danger" type="button" data-action="delete-order" data-id="${esc(order.id)}"><svg><use href="#i-trash"/></svg>Eliminar</button>
        <button class="btn btn-primary" type="button" data-action="close-modal">Cerrar</button>
      </div>
    </div>`;
  }

  function openOrderDetail(id) {
    const order = orderById(id);
    if (!order) {
      toast("El pedido ya no existe.", "error");
      return;
    }
    state.currentOrderId = id;
    openModal(orderDetailHtml(order), { wide: true });
  }

  function refreshOrderDetail() {
    if (!state.currentOrderId) return;
    const order = orderById(state.currentOrderId);
    const dlg = $("#modal");
    if (!order || !dlg.open) return;
    const body = dlg.querySelector(".modal-body");
    const scroll = body ? body.scrollTop : 0;
    dlg.innerHTML = orderDetailHtml(order);
    const nextBody = dlg.querySelector(".modal-body");
    if (nextBody) nextBody.scrollTop = scroll;
  }

  // ─────────────────────────── Clientes: formulario y detalle ───────────────────────────

  function openClientForm(id) {
    const c = id ? customerById(id) : null;
    const html = `<form data-form="customer" autocomplete="off">
      <div class="modal-card">
        <div class="modal-head">
          <div><h2 class="modal-title">${c ? "Editar cliente" : "Nuevo cliente"}</h2>
          <p class="modal-sub">${c ? "Actualiza los datos de contacto." : "Solo el nombre es obligatorio."}</p></div>
          <button class="icon-btn" type="button" data-action="close-modal" aria-label="Cerrar"><svg><use href="#i-x"/></svg></button>
        </div>
        <div class="modal-body">
          <input type="hidden" name="id" value="${esc(c ? c.id : "")}" />
          <div class="form-grid">
            <div class="field full"><label class="label" for="cName">Nombre completo</label>
              <input class="input" id="cName" name="fullName" maxlength="120" required value="${esc(c ? c.fullName : "")}" placeholder="Ej. María José Rivera" /></div>
            <div class="field"><label class="label" for="cPhone">Teléfono</label>
              <input class="input" id="cPhone" name="phone" maxlength="40" value="${esc(c ? c.phone : "")}" placeholder="+503 …" /></div>
            <div class="field"><label class="label" for="cEmail">Correo</label>
              <input class="input" id="cEmail" name="email" type="email" maxlength="160" value="${esc(c ? c.email : "")}" placeholder="opcional" /></div>
            <div class="field full"><label class="label" for="cAddress">Dirección</label>
              <input class="input" id="cAddress" name="address" maxlength="300" value="${esc(c ? c.address : "")}" /></div>
            <div class="field full"><label class="label" for="cNotes">Notas</label>
              <textarea class="textarea" id="cNotes" name="notes" maxlength="2000" rows="3" placeholder="Preferencias, telas, ajustes…">${esc(c ? c.notes : "")}</textarea></div>
          </div>
        </div>
        <div class="modal-foot">
          <button class="btn btn-ghost" type="button" data-action="close-modal">Cancelar</button>
          <button class="btn btn-primary" type="submit"><svg><use href="#i-check"/></svg>${c ? "Guardar cambios" : "Crear cliente"}</button>
        </div>
      </div>
    </form>`;
    openModal(html);
  }

  function openClientDetail(id) {
    const c = customerById(id);
    if (!c) {
      toast("El cliente ya no existe.", "error");
      return;
    }
    const s = clientStats(c.id);
    const list = orders()
      .filter((o) => o.customerId === c.id)
      .sort((a, b) => String(a.dueDate).localeCompare(String(b.dueDate)));

    const listHtml = list.length
      ? `<div class="list">${list
          .map(
            (o) => `<button class="list-row" type="button" data-action="open-order" data-id="${esc(o.id)}">
              ${dueBadge(o)}
              <div class="list-main"><div class="list-title">${esc(o.pieceName)}</div>
              <div class="list-sub">${esc(statusLabel(o.status))} · ${esc(fmtDate(o.dueDate))}</div></div>
              <div class="list-side">${balance(o) > 0 ? `<span class="muted small">${esc(money(balance(o)))}</span>` : ""}${statusPill(o.status)}</div>
            </button>`
          )
          .join("")}</div>`
      : `<p class="muted small" style="padding:8px 0">Este cliente aún no tiene pedidos.</p>`;

    const html = `<div class="modal-card">
      <div class="modal-head">
        <div class="row" style="gap:12px">
          <div class="avatar">${esc(initials(c.fullName))}</div>
          <div style="min-width:0">
            <h2 class="modal-title">${esc(c.fullName)}</h2>
            <p class="modal-sub">${esc(c.phone || "Sin teléfono")}${c.email ? " · " + esc(c.email) : ""}</p>
          </div>
        </div>
        <button class="icon-btn" type="button" data-action="close-modal" aria-label="Cerrar"><svg><use href="#i-x"/></svg></button>
      </div>
      <div class="modal-body">
        <div class="kv" style="margin-bottom:14px">
          <div class="kv-row"><span class="kv-key">Pedidos totales</span><span class="kv-val big">${s.total}</span></div>
          <div class="kv-row"><span class="kv-key">Pedidos activos</span><span class="kv-val big">${s.active}</span></div>
          <div class="kv-row"><span class="kv-key">Facturado</span><span class="kv-val">${esc(money(s.amount))}</span></div>
          <div class="kv-row"><span class="kv-key">Por cobrar</span><span class="kv-val" style="color:${s.due > 0 ? "var(--warn)" : "var(--good)"}">${esc(money(s.due))}</span></div>
        </div>
        ${c.address ? `<div class="kv-row"><span class="kv-key">Dirección</span><span class="kv-val">${esc(c.address)}</span></div>` : ""}
        ${c.notes ? `<div style="margin-top:10px"><span class="kv-key">Notas</span><p class="small" style="margin:4px 0 0;white-space:pre-wrap">${esc(c.notes)}</p></div>` : ""}
        <hr class="divider" />
        <div class="form-section-title" style="margin-bottom:8px"><svg><use href="#i-shirt"/></svg>Pedidos del cliente</div>
        ${listHtml}
      </div>
      <div class="modal-foot">
        <button class="btn btn-danger" type="button" data-action="delete-client" data-id="${esc(c.id)}"><svg><use href="#i-trash"/></svg>Eliminar</button>
        <span class="spacer"></span>
        <button class="btn btn-ghost" type="button" data-action="edit-client" data-id="${esc(c.id)}"><svg><use href="#i-pencil"/></svg>Editar</button>
        <button class="btn btn-primary" type="button" data-action="new-order-for-client" data-id="${esc(c.id)}"><svg><use href="#i-plus"/></svg>Nuevo pedido</button>
      </div>
    </div>`;
    openModal(html, { wide: true });
  }

  // ─────────────────────────── Recordatorios ───────────────────────────

  function openReminderForm(id) {
    const r = id ? reminders().find((x) => x.id === id) : null;
    const now = new Date();
    const def = new Date(now.getTime() + 60 * 60 * 1000);
    def.setMinutes(0, 0, 0);
    const defValue = `${def.getFullYear()}-${pad(def.getMonth() + 1)}-${pad(def.getDate())}T${pad(def.getHours())}:${pad(def.getMinutes())}`;
    const value = r ? String(r.dueDate).slice(0, 16) : defValue;

    const customerOptions = `<option value="">— Sin cliente —</option>` +
      customers()
        .slice()
        .sort((a, b) => a.fullName.localeCompare(b.fullName, "es"))
        .map((c) => `<option value="${esc(c.id)}" ${r && r.customerId === c.id ? "selected" : ""}>${esc(c.fullName)}</option>`)
        .join("");

    const orderOptions = `<option value="">— Sin pedido —</option>` +
      orders()
        .slice()
        .sort((a, b) => String(a.dueDate).localeCompare(String(b.dueDate)))
        .map((o) => `<option value="${esc(o.id)}" ${r && r.orderId === o.id ? "selected" : ""}>${esc(o.pieceName)} · ${esc(customerName(o.customerId))}</option>`)
        .join("");

    const html = `<form data-form="reminder" autocomplete="off">
      <div class="modal-card">
        <div class="modal-head">
          <div><h2 class="modal-title">${r ? "Editar recordatorio" : "Nuevo recordatorio"}</h2>
          <p class="modal-sub">Pruebas, entregas, compras de materiales o llamadas.</p></div>
          <button class="icon-btn" type="button" data-action="close-modal" aria-label="Cerrar"><svg><use href="#i-x"/></svg></button>
        </div>
        <div class="modal-body">
          <input type="hidden" name="id" value="${esc(r ? r.id : "")}" />
          <div class="form-grid">
            <div class="field full"><label class="label" for="rTitle">Título</label>
              <input class="input" id="rTitle" name="title" maxlength="140" required value="${esc(r ? r.title : "")}" placeholder="Ej. Prueba del vestido con Ana" /></div>
            <div class="field"><label class="label" for="rDue">Fecha y hora</label>
              <input class="input" id="rDue" name="dueDate" type="datetime-local" required value="${esc(value)}" /></div>
            <div class="field"><label class="label" for="rPriority">Prioridad</label>
              <select class="select" id="rPriority" name="priority">
                ${Object.keys(PRIORITY).map((p) => `<option value="${p}" ${r && r.priority === p ? "selected" : ""}>${PRIORITY[p].label}</option>`).join("")}
              </select></div>
            <div class="field"><label class="label" for="rCustomer">Cliente (opcional)</label>
              <select class="select" id="rCustomer" name="customerId">${customerOptions}</select></div>
            <div class="field"><label class="label" for="rOrder">Pedido (opcional)</label>
              <select class="select" id="rOrder" name="orderId">${orderOptions}</select></div>
            <div class="field full"><label class="label" for="rDesc">Descripción</label>
              <textarea class="textarea" id="rDesc" name="description" rows="2" maxlength="1500" placeholder="Detalles de la cita o pendiente…">${esc(r ? r.description : "")}</textarea></div>
          </div>
        </div>
        <div class="modal-foot">
          <button class="btn btn-ghost" type="button" data-action="close-modal">Cancelar</button>
          <button class="btn btn-primary" type="submit"><svg><use href="#i-check"/></svg>${r ? "Guardar" : "Crear recordatorio"}</button>
        </div>
      </div>
    </form>`;
    openModal(html);
  }

  // ─────────────────────────── Acciones ───────────────────────────

  async function handleAction(el) {
    const action = el.dataset.action;
    const id = el.dataset.id;

    switch (action) {
      case "new-order":
        openOrderForm(null, null);
        break;
      case "new-order-for-client":
        openOrderForm(null, id);
        break;
      case "new-client":
        openClientForm(null);
        break;
      case "new-client-from-order":
        openClientForm(null);
        break;
      case "edit-order":
        openOrderForm(id, null);
        break;
      case "open-order":
        openOrderDetail(id);
        break;
      case "open-client":
        openClientDetail(id);
        break;
      case "edit-client":
        openClientForm(id);
        break;
      case "new-reminder":
        openReminderForm(null);
        break;
      case "edit-reminder":
        openReminderForm(id);
        break;
      case "close-modal":
        closeModal();
        break;
      case "clear-filters":
        state.orderQuery = "";
        state.orderStatus = "todos";
        render();
        break;
      case "agenda-filter":
        state.agendaFilter = el.dataset.filter;
        render();
        break;
      case "order-type": {
        if (!orderForm) break;
        orderForm.type = el.dataset.type;
        orderForm.style = "";
        renderOrderTypeAreas();
        renderOrderMeasureArea();
        renderOrderPreview();
        break;
      }
      case "apply-preset": {
        if (!orderForm) break;
        const presets = G.presets(orderForm.type);
        const preset = presets && presets[el.dataset.preset];
        if (!preset) break;
        Object.assign(orderForm.values[orderForm.type], preset);
        if (G.styles(orderForm.type).includes(el.dataset.preset)) {
          orderForm.style = el.dataset.preset;
          renderOrderTypeAreas();
        }
        renderOrderMeasureArea();
        renderOrderPreview();
        break;
      }
      case "set-status":
        await changeStatus(id, el.dataset.status);
        break;
      case "print-order":
        printOrder(id);
        break;
      case "delete-note":
        await removeNote(el.dataset.order, el.dataset.note);
        break;
      case "toggle-reminder":
        try {
          state.data = await call("toggle_reminder", { id });
          render();
        } catch (e) {
          /* avisado */
        }
        break;
      case "delete-reminder":
        await removeReminder(id);
        break;
      case "delete-order":
        await removeOrder(id);
        break;
      case "delete-client":
        await removeClient(id);
        break;
      case "backup-create":
        await createBackup();
        break;
      case "backup-restore":
        await restoreBackup(el.dataset.file);
        break;
      case "open-folder":
        try {
          await call("open_folder", { which: el.dataset.folder || "data" });
        } catch (e) {
          /* avisado */
        }
        break;
      case "seed-demo":
        await loadDemo();
        break;
      case "reset-all":
        await resetAll();
        break;
      default:
        break;
    }
  }

  async function changeStatus(orderId, status) {
    const order = orderById(orderId);
    if (!order || order.status === status) return;
    try {
      state.data = await call("set_order_status", { id: orderId, status, note: null });
      toast(`Pedido marcado como «${statusLabel(status)}».`, "success");
      render();
      refreshOrderDetail();
    } catch (e) {
      /* avisado */
    }
  }

  async function removeNote(orderId, noteId) {
    const ok = await confirmDialog({
      title: "Eliminar nota",
      message: "Esta nota de la bitácora se borrará. No se puede deshacer.",
      confirmLabel: "Eliminar",
      danger: true,
    });
    if (!ok) return;
    try {
      state.data = await call("delete_order_note", { orderId, noteId });
      render();
      refreshOrderDetail();
      toast("Nota eliminada.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function removeOrder(orderId) {
    const order = orderById(orderId);
    if (!order) return;
    const ok = await confirmDialog({
      title: "Eliminar pedido",
      message: `Se eliminará «${order.pieceName}» y sus notas. Esta acción no se puede deshacer.`,
      confirmLabel: "Eliminar pedido",
      danger: true,
    });
    if (!ok) return;
    try {
      state.data = await call("delete_order", { id: orderId });
      closeModal();
      render();
      toast("Pedido eliminado.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function removeClient(clientId) {
    const c = customerById(clientId);
    if (!c) return;
    const st = clientStats(clientId);
    const ok = await confirmDialog({
      title: "Eliminar cliente",
      message: `Se eliminará a «${c.fullName}»${st.total ? ` junto con ${st.total} ${plural(st.total, "pedido", "pedidos")} y sus recordatorios` : ""}. Esta acción no se puede deshacer.`,
      confirmLabel: "Eliminar cliente",
      danger: true,
    });
    if (!ok) return;
    try {
      state.data = await call("delete_customer", { id: clientId });
      closeModal();
      render();
      toast("Cliente eliminado.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function removeReminder(reminderId) {
    const r = reminders().find((x) => x.id === reminderId);
    if (!r) return;
    const ok = await confirmDialog({
      title: "Eliminar recordatorio",
      message: `Se eliminará «${r.title}».`,
      confirmLabel: "Eliminar",
      danger: true,
    });
    if (!ok) return;
    try {
      state.data = await call("delete_reminder", { id: reminderId });
      render();
      toast("Recordatorio eliminado.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function createBackup() {
    try {
      await call("create_backup");
      await loadBackups();
      toast("Respaldo creado.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function restoreBackup(file) {
    const ok = await confirmDialog({
      title: "Restaurar respaldo",
      message: `Se reemplazarán los datos actuales por los del respaldo:\n${file}\n\nAntes de restaurar se guarda un respaldo de la versión actual.`,
      confirmLabel: "Restaurar",
    });
    if (!ok) return;
    try {
      state.data = await call("restore_backup", { file });
      closeModal();
      render();
      toast("Respaldo restaurado correctamente.", "success");
    } catch (e) {
      /* avisado */
    }
  }

  async function loadDemo() {
    const ok = await confirmDialog({
      title: "Cargar datos de ejemplo",
      message: "Se reemplazarán los clientes, pedidos y recordatorios actuales por un taller de ejemplo. Antes se guarda un respaldo.",
      confirmLabel: "Cargar ejemplo",
    });
    if (!ok) return;
    try {
      state.data = await call("load_demo");
      closeModal();
      render();
      toast("Datos de ejemplo cargados. Explóralos y luego borra lo que no necesites.", "success", 6000);
    } catch (e) {
      /* avisado */
    }
  }

  async function resetAll() {
    const first = await confirmDialog({
      title: "Borrar todos los datos",
      message: "Se eliminarán todos los clientes, pedidos y recordatorios de este equipo. Se guardará un respaldo antes de borrar.",
      confirmLabel: "Continuar",
      danger: true,
    });
    if (!first) return;
    const second = await confirmDialog({
      title: "¿Seguro?",
      message: "Esta es la última confirmación. Después de borrar, la aplicación quedará vacía.",
      confirmLabel: "Sí, borrar todo",
      danger: true,
    });
    if (!second) return;
    try {
      state.data = await call("reset_all");
      closeModal();
      render();
      toast("Datos borrados. Puedes restaurar un respaldo desde Ajustes.", "success", 6000);
    } catch (e) {
      /* avisado */
    }
  }

  // ─────────────────────────── Formularios ───────────────────────────

  async function handleForm(form) {
    const kind = form.dataset.form;
    const submit = form.querySelector('[type="submit"]');
    if (submit) submit.disabled = true;
    try {
      if (kind === "settings") {
        const fd = new FormData(form);
        state.data = await call("save_settings", {
          workshopName: String(fd.get("workshopName") || ""),
          ownerName: String(fd.get("ownerName") || ""),
          currency: String(fd.get("currency") || "$"),
          defaultDueDays: Number(fd.get("defaultDueDays") || 7),
        });
        toast("Ajustes guardados.", "success");
        render();
        return;
      }

      if (kind === "customer") {
        const fd = new FormData(form);
        const customerId = String(fd.get("id") || "");
        const previousIds = customers().map((c) => c.id);
        state.data = await call("save_customer", {
          input: {
            id: customerId || null,
            fullName: String(fd.get("fullName") || ""),
            phone: String(fd.get("phone") || ""),
            email: String(fd.get("email") || ""),
            address: String(fd.get("address") || ""),
            notes: String(fd.get("notes") || ""),
          },
        });
        render();
        if (!customerId && pendingOrderAfterClient) {
          pendingOrderAfterClient = false;
          const created = customers().find((c) => !previousIds.includes(c.id));
          toast("Cliente creado. Ahora registra su pedido.", "success");
          openOrderForm(null, created ? created.id : null);
          return;
        }
        closeModal();
        toast(customerId ? "Cliente actualizado." : "Cliente creado.", "success");
        return;
      }

      if (kind === "reminder") {
        const fd = new FormData(form);
        const reminderId = String(fd.get("id") || "");
        state.data = await call("save_reminder", {
          input: {
            id: reminderId || null,
            title: String(fd.get("title") || ""),
            description: String(fd.get("description") || ""),
            dueDate: String(fd.get("dueDate") || ""),
            priority: String(fd.get("priority") || "normal"),
            customerId: String(fd.get("customerId") || "") || null,
            orderId: String(fd.get("orderId") || "") || null,
          },
        });
        closeModal();
        render();
        toast(reminderId ? "Recordatorio actualizado." : "Recordatorio creado.", "success");
        return;
      }

      if (kind === "order") {
        if (!orderForm) return;
        const payload = readOrderForm(form);
        state.data = await call("save_order", { input: payload });
        closeModal();
        render();
        toast(payload.id ? "Pedido actualizado." : "Pedido creado.", "success");
        return;
      }

      if (kind === "note") {
        const orderId = form.dataset.order;
        const fd = new FormData(form);
        state.data = await call("add_order_note", {
          orderId,
          stage: String(fd.get("stage") || "seguimiento"),
          content: String(fd.get("content") || ""),
        });
        render();
        refreshOrderDetail();
        toast("Nota agregada.", "success");
        return;
      }
    } catch (e) {
      /* el aviso ya se mostró */
    } finally {
      if (submit) submit.disabled = false;
    }
  }

  // ─────────────────────────── Imprimir ficha ───────────────────────────

  function printOrder(orderId) {
    const order = orderById(orderId);
    if (!order) return;
    const customer = customerById(order.customerId);
    const s = data().settings;
    const meas = G.measureSummary(order.garmentType, order.measurements.values || {});
    const spec = G.GARMENTS[order.garmentType];
    const opts = spec
      ? spec.options.map((o) => ({ label: o.label, value: (order.measurements.options || {})[o.key] })).filter((o) => o.value)
      : [];

    const rows = meas.map((m) => `<tr><td>${esc(m.label)}</td><td>${Math.round(m.value)} cm</td></tr>`).join("");
    const optRows = opts.map((o) => `<tr><td>${esc(o.label)}</td><td>${esc(o.value)}</td></tr>`).join("");
    const notes = (order.notes || []).slice().sort((a, b) => String(a.timestamp).localeCompare(String(b.timestamp)));

    $("#printRoot").innerHTML = `<div class="print-sheet">
      <h1>${esc(s.workshopName || "Taller de costura")}</h1>
      <p class="print-sub">Ficha de pedido · ${esc(fmtDateLong(todayStr()))}</p>
      <hr />
      <div class="print-grid">
        <div><span>Cliente</span><strong>${esc(customer ? customer.fullName : customerName(order.customerId))}</strong></div>
        <div><span>Teléfono</span><strong>${esc(customer && customer.phone ? customer.phone : "—")}</strong></div>
        <div><span>Prenda</span><strong>${esc(order.pieceName)}</strong></div>
        <div><span>Tipo / estilo</span><strong>${esc(G.label(order.garmentType))}${order.styleName ? " · " + esc(order.styleName) : ""}</strong></div>
        <div><span>Entrega</span><strong>${esc(fmtDate(order.dueDate))}</strong></div>
        <div><span>Estado</span><strong>${esc(statusLabel(order.status))}</strong></div>
        <div><span>Costo</span><strong>${esc(money(order.estimatedCost))}</strong></div>
        <div><span>Anticipo</span><strong>${esc(money(order.depositPaid))}</strong></div>
        <div><span>Saldo</span><strong>${esc(money(balance(order)))}</strong></div>
        <div><span>Prioridad</span><strong>${esc((PRIORITY[order.priority] || { label: order.priority }).label)}</strong></div>
      </div>
      <hr />
      <h3 style="font-size:13px;margin:0 0 4px">Medidas (cm)</h3>
      <table class="print-table"><tbody>${rows}${optRows}</tbody></table>
      ${order.measurements.notes ? `<p class="print-notes"><strong>Notas de medidas:</strong> ${esc(order.measurements.notes)}</p>` : ""}
      ${order.fabricDetails ? `<p class="print-notes"><strong>Tela y detalles:</strong> ${esc(order.fabricDetails)}</p>` : ""}
      <h3 style="font-size:13px;margin:14px 0 4px">Bitácora</h3>
      <table class="print-table"><thead><tr><th>Fecha</th><th>Etapa</th><th>Detalle</th></tr></thead><tbody>
        ${notes.map((n) => `<tr><td>${esc(fmtDateTime(n.timestamp))}</td><td>${esc(stageLabel(n.stage))}</td><td>${esc(n.content)}</td></tr>`).join("")}
      </tbody></table>
      <div class="print-sign"><div>Firma del taller</div><div>Firma del cliente</div></div>
    </div>`;

    setTimeout(() => window.print(), 60);
  }

  // ─────────────────────────── Eventos ───────────────────────────

  function bindGlobalEvents() {
    document.addEventListener("click", async (e) => {
      const nav = e.target.closest("[data-view]");
      if (nav && !e.target.closest("[data-action]")) {
        setView(nav.dataset.view);
        return;
      }
      const el = e.target.closest("[data-action]");
      if (!el) return;
      if (el.tagName === "TR" && e.target.closest("button")) return;
      e.preventDefault();
      await handleAction(el);
    });

    document.addEventListener("submit", async (e) => {
      const form = e.target.closest("form[data-form]");
      if (!form) return;
      e.preventDefault();
      await handleForm(form);
    });

    document.addEventListener("input", (e) => {
      const target = e.target;
      if (target.id === "orderSearch") {
        state.orderQuery = target.value;
        const pos = target.selectionStart;
        render();
        const again = $("#orderSearch");
        if (again) {
          again.focus();
          try {
            again.setSelectionRange(pos, pos);
          } catch (err) {
            /* ignorar */
          }
        }
        return;
      }
      if (target.id === "clientSearch") {
        state.clientQuery = target.value;
        const pos = target.selectionStart;
        render();
        const again = $("#clientSearch");
        if (again) {
          again.focus();
          try {
            again.setSelectionRange(pos, pos);
          } catch (err) {
            /* ignorar */
          }
        }
        return;
      }

      const slider = target.closest("[data-measure]");
      if (slider && orderForm) {
        const key = slider.dataset.measure;
        const value = Number(slider.value);
        orderForm.values[orderForm.type][key] = value;
        const badge = document.querySelector(`[data-value-for="${key}"]`);
        if (badge) badge.textContent = `${Math.round(value)} cm`;
        const min = Number(slider.min);
        const max = Number(slider.max);
        slider.style.setProperty("--fill", `${Math.round(((value - min) / (max - min)) * 100)}%`);
        if (key === "estimatedCost" || key === "depositPaid") {
          /* no aplica */
        }
        renderOrderPreview();
        return;
      }

      if (target.closest('#modal form[data-form="order"]') && (target.name === "estimatedCost" || target.name === "depositPaid")) {
        renderOrderPreview();
      }
    });

    document.addEventListener("change", (e) => {
      const target = e.target;
      if (target.id === "orderStatusFilter") {
        state.orderStatus = target.value;
        render();
        return;
      }
      if (target.id === "orderStyle" && orderForm) {
        orderForm.style = target.value;
        renderOrderPreview();
        return;
      }
      const opt = target.closest("[data-option]");
      if (opt && orderForm) {
        orderForm.options[orderForm.type][opt.dataset.option] = opt.value;
        renderOrderPreview();
      }
    });

    const modal = $("#modal");
    if (modal) {
      modal.addEventListener("click", (e) => {
        if (e.target === modal) closeModal();
      });
      modal.addEventListener("close", () => {
        state.currentOrderId = null;
        orderForm = null;
      });
    }

    $("#themeToggle").addEventListener("click", () => {
      const current = document.documentElement.getAttribute("data-theme");
      applyTheme(current === "dark" ? "light" : "dark");
    });

    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        const confirm = $("#confirm");
        if (confirm && confirm.open) return;
        if ($("#modal") && $("#modal").open) closeModal();
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n") {
        if ($("#modal") && $("#modal").open) return;
        e.preventDefault();
        openOrderForm(null, null);
      }
    });
  }

  // Arranque
  window.addEventListener("error", (event) => {
    window.__lastError = String(event.message || event.error || "error desconocido");
  });
  window.addEventListener("unhandledrejection", (event) => {
    window.__lastError = String((event.reason && event.reason.message) || event.reason || "promesa rechazada");
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
