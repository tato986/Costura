// ─────────────────────────────────────────────────────────────────────────────
// Costura v2.0 — Puente Modular Firebase (App + Auth + Firestore Offline + Storage)
// ─────────────────────────────────────────────────────────────────────────────
// - Carga la configuración desde el backend en Rust (`get_firebase_env_config`),
//   que hidrata desde `.env` local sin hardcodear secretos en `src/` ni en Git.
// - Usa los bundles modulares locales en `./vendor/firebase/` (cumple CSP 'self').
// - Custodia el token de sesión en el llavero del SO vía crate `keyring` en Rust.
// - Materializa y sincroniza la raíz `/talleres/{uid}` y sus 6 subcolecciones
//   (`clientes`, `pedidos`, `insumos`, `movimientos`, `agenda`, `auditoria`)
//   mediante lotes atómicos `WriteBatch` (máx. 500 ops) y `increment()`.

import {
  initializeApp,
  getApps,
  getApp,
} from "./vendor/firebase/firebase-app.js";

import {
  getAuth,
  connectAuthEmulator,
  signInWithEmailAndPassword,
  createUserWithEmailAndPassword,
  GoogleAuthProvider,
  signInWithCredential,
  signInWithPopup,
  signInWithRedirect,
  getRedirectResult,
  signOut as fbSignOut,
  onAuthStateChanged,
  sendPasswordResetEmail,
} from "./vendor/firebase/firebase-auth.js";

import {
  initializeFirestore,
  getFirestore,
  persistentLocalCache,
  connectFirestoreEmulator,
  doc,
  writeBatch,
  increment,
  getDoc,
  getDocs,
  collection,
} from "./vendor/firebase/firebase-firestore.js";

import {
  getStorage,
  connectStorageEmulator,
  ref as storageRef,
  uploadString,
  getDownloadURL,
} from "./vendor/firebase/firebase-storage.js";

const DEFAULT_SLOTS = {
  mode: "offline_local",
  useEmulators: false,
  emulatorAuthHost: "http://127.0.0.1:9099",
  emulatorFirestoreHost: "127.0.0.1:8080",
  emulatorStorageHost: "127.0.0.1:9199",
  projectId: "REEMPLAZAR_PROJECT_ID",
  apiKey: "REEMPLAZAR_CON_FIREBASE_API_KEY",
  authDomain: "REEMPLAZAR_PROJECT_ID.firebaseapp.com",
  storageBucket: "REEMPLAZAR_PROJECT_ID.firebasestorage.app",
  messagingSenderId: "REEMPLAZAR_MESSAGING_SENDER_ID",
  appId: "REEMPLAZAR_WEB_APP_ID",
  googleWebClientId: "REEMPLAZAR_GOOGLE_OAUTH_WEB_CLIENT_ID.apps.googleusercontent.com",
  activeUid: null,
};

const runtime = {
  slots: { ...DEFAULT_SLOTS },
  app: null,
  auth: null,
  db: null,
  storage: null,
  emulatorsConnected: false,
  currentUser: null,
  keyringRestoredUid: null,
  lastSyncSummary: null,
  lastAuthError: null,
  listeners: new Set(),
};

function getTauriInvoke() {
  return window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke.bind(window.__TAURI__.core)
    : null;
}

export function getWorkshopPaths(uid) {
  const safeUid = String(uid || "").trim() || "PENDIENTE_UID_FIREBASE";
  const root = `/talleres/${safeUid}`;
  return {
    uid: safeUid,
    rootWorkshopDoc: root,
    clientes: `${root}/clientes`,
    pedidos: `${root}/pedidos`,
    insumos: `${root}/insumos`,
    movimientos: `${root}/movimientos`,
    agenda: `${root}/agenda`,
    auditoria: `${root}/auditoria`,
    storagePedidosPrefix: `/talleres/${safeUid}/pedidos`,
  };
}

export function deterministicMovementId(orderId, revision, supplyId) {
  return `mov_${orderId}_r${revision}_${supplyId}`;
}

export function isCloudReady(slotsArg) {
  const s = slotsArg || runtime.slots;
  return (
    Boolean(s && s.projectId) &&
    !String(s.projectId).startsWith("REEMPLAZAR_") &&
    Boolean(s && s.apiKey) &&
    !String(s.apiKey).startsWith("REEMPLAZAR_")
  );
}

function notifyListeners() {
  const status = getStatus();
  runtime.listeners.forEach((fn) => {
    try {
      fn(status);
    } catch (_) {
      /* ignore listener errors */
    }
  });
  window.dispatchEvent(new CustomEvent("costura:firebase-status", { detail: status }));
}

export function onStatusChange(callback) {
  if (typeof callback === "function") {
    runtime.listeners.add(callback);
  }
  return () => runtime.listeners.delete(callback);
}

export function getStatus() {
  return {
    configured: isCloudReady(runtime.slots),
    initialized: Boolean(runtime.app && runtime.db && runtime.auth),
    mode: runtime.slots.mode || "offline_local",
    useEmulators: Boolean(runtime.slots.useEmulators),
    projectId: runtime.slots.projectId,
    authDomain: runtime.slots.authDomain,
    storageBucket: runtime.slots.storageBucket,
    currentUser: runtime.currentUser
      ? {
          uid: runtime.currentUser.uid,
          email: runtime.currentUser.email || "",
          displayName: runtime.currentUser.displayName || "",
        }
      : null,
    activeUid:
      (runtime.currentUser && runtime.currentUser.uid) ||
      runtime.slots.activeUid ||
      runtime.keyringRestoredUid ||
      null,
    keyringRestoredUid: runtime.keyringRestoredUid,
    lastSyncSummary: runtime.lastSyncSummary,
    lastAuthError: runtime.lastAuthError,
  };
}

function parseHostPort(hostStr, defaultHost, defaultPort) {
  const cleaned = String(hostStr || "")
    .trim()
    .replace(/^https?:\/\//i, "");
  const parts = cleaned.split(":");
  const host = parts[0] || defaultHost;
  const port = parseInt(parts[1], 10) || defaultPort;
  return { host, port };
}

/**
 * Inicializa Firebase App, Auth, Firestore (con persistentLocalCache 100 MB) y Storage
 * leyendo la configuración hidratada desde `.env` en Rust.
 */
export async function initFromEnv(overrideSlots) {
  const invoke = getTauriInvoke();
  if (overrideSlots) {
    runtime.slots = { ...runtime.slots, ...overrideSlots };
  } else if (invoke) {
    try {
      const envSlots = await invoke("get_firebase_env_config");
      if (envSlots) {
        runtime.slots = { ...runtime.slots, ...envSlots };
      }
      const rawKeyring = await invoke("get_secure_session_token");
      if (rawKeyring) {
        try {
          const parsed = JSON.parse(rawKeyring);
          if (parsed && parsed.uid) {
            runtime.keyringRestoredUid = parsed.uid;
          }
        } catch (_) {
          /* ignore invalid keyring json */
        }
      }
    } catch (_) {
      /* fallback to current slots */
    }
  }

  if (!isCloudReady(runtime.slots)) {
    notifyListeners();
    return getStatus();
  }

  const firebaseConfig = {
    apiKey: runtime.slots.apiKey,
    authDomain: runtime.slots.authDomain,
    projectId: runtime.slots.projectId,
    storageBucket: runtime.slots.storageBucket,
    messagingSenderId: runtime.slots.messagingSenderId,
    appId: runtime.slots.appId,
  };

  if (!runtime.app) {
    runtime.app = getApps().length ? getApp() : initializeApp(firebaseConfig);
    runtime.auth = getAuth(runtime.app);

    try {
      // Caché persistente local Offline-First (100 MB) en IndexedDB
      runtime.db = initializeFirestore(runtime.app, {
        localCache: persistentLocalCache({
          cacheSizeBytes: 100 * 1024 * 1024,
        }),
      });
    } catch (_) {
      runtime.db = getFirestore(runtime.app);
    }

    runtime.storage = getStorage(runtime.app);

    if (
      (runtime.slots.mode === "emulator" || runtime.slots.useEmulators) &&
      !runtime.emulatorsConnected
    ) {
      const authUrl = runtime.slots.emulatorAuthHost || "http://127.0.0.1:9099";
      const fsHp = parseHostPort(runtime.slots.emulatorFirestoreHost, "127.0.0.1", 8080);
      const stHp = parseHostPort(runtime.slots.emulatorStorageHost, "127.0.0.1", 9199);
      try {
        connectAuthEmulator(runtime.auth, authUrl, { disableWarnings: true });
        connectFirestoreEmulator(runtime.db, fsHp.host, fsHp.port);
        connectStorageEmulator(runtime.storage, stHp.host, stHp.port);
        runtime.emulatorsConnected = true;
      } catch (_) {
        /* already connected */
      }
    }

    onAuthStateChanged(runtime.auth, async (user) => {
      runtime.currentUser = user || null;
      const invokeFn = getTauriInvoke();
      if (user) {
        runtime.slots.activeUid = user.uid;
        runtime.keyringRestoredUid = user.uid;
        if (invokeFn && user.refreshToken) {
          try {
            await invokeFn("save_secure_session_token", {
              uid: user.uid,
              refreshToken: user.refreshToken,
              email: user.email || user.displayName || null,
            });
          } catch (_) {
            /* keyring save warning non-fatal */
          }
        }
      }
      notifyListeners();
    });

    try {
      const cred = await getRedirectResult(runtime.auth);
      if (cred && cred.user) {
        runtime.lastAuthError = null;
        await persistAuthenticatedUserInRust(cred.user);
      }
    } catch (err) {
      runtime.lastAuthError = translateAuthError(err);
    }
  }

  notifyListeners();
  return getStatus();
}

export function translateAuthError(err) {
  const code = String((err && err.code) || "");
  const msg = String((err && err.message) || err || "");

  if (
    code.includes("auth/wrong-password") ||
    code.includes("auth/invalid-credential") ||
    code.includes("auth/invalid-login-credentials") ||
    msg.includes("INVALID_LOGIN_CREDENTIALS") ||
    msg.includes("INVALID_PASSWORD")
  ) {
    return "Contraseña incorrecta o credenciales no válidas. Verifica tu correo y contraseña.";
  }
  if (code.includes("auth/user-not-found") || msg.includes("EMAIL_NOT_FOUND")) {
    return "Usuario no encontrado. Verifica el correo electrónico o crea una cuenta para tu taller.";
  }
  if (code.includes("auth/email-already-in-use") || msg.includes("EMAIL_EXISTS")) {
    return "Este correo ya está registrado. Cambia a «Iniciar sesión» o recupera tu contraseña.";
  }
  if (code.includes("auth/invalid-email") || msg.includes("INVALID_EMAIL")) {
    return "El formato del correo electrónico no es válido.";
  }
  if (code.includes("auth/weak-password") || msg.includes("WEAK_PASSWORD")) {
    return "La contraseña es demasiado débil. Usa al menos 6 caracteres.";
  }
  if (
    code.includes("auth/network-request-failed") ||
    msg.toLowerCase().includes("network") ||
    msg.toLowerCase().includes("failed to fetch")
  ) {
    return "Sin conexión a internet. Verifica tu red o usa «Continuar en modo local sin conexión».";
  }
  if (code.includes("auth/too-many-requests") || msg.includes("TOO_MANY_ATTEMPTS")) {
    return "Demasiados intentos fallidos. Espera unos minutos o restablece tu contraseña.";
  }
  if (code.includes("auth/popup-closed-by-user") || code.includes("auth/cancelled-popup-request")) {
    return "Se cerró la ventana de inicio de sesión con Google antes de completar el acceso.";
  }
  if (code.includes("auth/popup-blocked")) {
    return "El sistema bloqueó la ventana emergente de Google. Intenta de nuevo o usa correo y contraseña.";
  }
  return msg || "No se pudo completar la autenticación.";
}

async function persistAuthenticatedUserInRust(user) {
  if (!user || !user.uid) return;
  runtime.currentUser = user;
  runtime.slots.activeUid = user.uid;
  runtime.keyringRestoredUid = user.uid;
  const invoke = getTauriInvoke();
  if (invoke) {
    await invoke("save_secure_session_token", {
      uid: user.uid,
      refreshToken: user.refreshToken || `session_${user.uid}`,
      email: user.email || user.displayName || null,
    });
  }
  notifyListeners();
}

export async function loginWithEmail(email, password) {
  await initFromEnv();
  if (!runtime.auth) {
    throw new Error("Firebase no está configurado. Verifica tu archivo .env o continúa en modo local.");
  }
  try {
    const cred = await signInWithEmailAndPassword(
      runtime.auth,
      String(email || "").trim(),
      String(password || "")
    );
    await persistAuthenticatedUserInRust(cred.user);
    return cred.user;
  } catch (err) {
    const translated = new Error(translateAuthError(err));
    translated.code = err && err.code;
    throw translated;
  }
}

export async function registerWithEmail(email, password) {
  await initFromEnv();
  if (!runtime.auth) {
    throw new Error("Firebase no está configurado. Verifica tu archivo .env o continúa en modo local.");
  }
  try {
    const cred = await createUserWithEmailAndPassword(
      runtime.auth,
      String(email || "").trim(),
      String(password || "")
    );
    await persistAuthenticatedUserInRust(cred.user);
    return cred.user;
  } catch (err) {
    const translated = new Error(translateAuthError(err));
    translated.code = err && err.code;
    throw translated;
  }
}

export async function loginWithGoogle() {
  if (!runtime.auth) {
    await initFromEnv();
  }
  if (!runtime.auth) {
    throw new Error("Firebase no está configurado. Verifica tu archivo .env o continúa en modo local.");
  }
  try {
    const invoke = getTauriInvoke();
    if (invoke) {
      const idToken = await invoke("google_oauth_desktop_login");
      const credential = GoogleAuthProvider.credential(idToken);
      const cred = await signInWithCredential(runtime.auth, credential);
      await persistAuthenticatedUserInRust(cred.user);
      return cred.user;
    }

    const provider = new GoogleAuthProvider();
    provider.setCustomParameters({ prompt: "select_account" });
    const cred = await signInWithPopup(runtime.auth, provider);
    await persistAuthenticatedUserInRust(cred.user);
    return cred.user;
  } catch (err) {
    const translated = new Error(translateAuthError(err));
    translated.code = err && err.code;
    throw translated;
  }
}

export async function resetPassword(email) {
  await initFromEnv();
  if (!runtime.auth) {
    throw new Error("Firebase no está configurado. Verifica tu archivo .env.");
  }
  const cleanEmail = String(email || "").trim();
  if (!cleanEmail) {
    throw new Error("Escribe tu correo electrónico arriba para enviarte el enlace de recuperación.");
  }
  try {
    await sendPasswordResetEmail(runtime.auth, cleanEmail);
  } catch (err) {
    const translated = new Error(translateAuthError(err));
    translated.code = err && err.code;
    throw translated;
  }
}

export async function logout() {
  if (runtime.auth) {
    await fbSignOut(runtime.auth);
  }
  runtime.currentUser = null;
  runtime.keyringRestoredUid = null;
  runtime.slots.activeUid = null;
  const invoke = getTauriInvoke();
  if (invoke) {
    try {
      await invoke("clear_secure_session_token");
    } catch (_) {
      /* ignore */
    }
  }
  notifyListeners();
}

/**
 * Sanitiza un pedido antes de subirlo a Firestore:
 * - Limita `images` a un máximo de 4 referencias ligeras (para cumplir `images.size() <= 4`
 *   y el límite de 1 MB por documento de Firestore).
 */
function sanitizeOrderForFirestore(order, uid) {
  const rawImages = Array.isArray(order.storageImages) && order.storageImages.length
    ? order.storageImages
    : Array.isArray(order.images)
      ? order.images
      : [];

  const cleanImages = rawImages.slice(0, 4).map((item, idx) => {
    if (typeof item === "string") {
      // Si es un dataURL base64 local pesado, guardamos solo metadato de referencia en el documento Firestore
      const isDataUrl = item.startsWith("data:");
      return {
        id: `img_${idx + 1}`,
        storagePath: `/talleres/${uid}/pedidos/${order.id}/img_${idx + 1}.webp`,
        downloadUrl: isDataUrl ? "" : item,
        sizeBytes: item.length,
      };
    }
    return item;
  });

  const dueDate = String(order.dueDate || "").slice(0, 10);
  const deliveryDate = order.deliveryDate ? String(order.deliveryDate).slice(0, 10) : null;

  return {
    id: String(order.id),
    customerId: String(order.customerId),
    pieceName: String(order.pieceName || "").slice(0, 160),
    garmentType: String(order.garmentType || "otro"),
    customType: String(order.customType || "").slice(0, 80),
    styleName: String(order.styleName || "").slice(0, 80),
    status: String(order.status || "pendiente"),
    priority: String(order.priority || "normal"),
    measurements: order.measurements || { kind: order.garmentType || "otro", values: {}, options: {}, notes: "" },
    fabricDetails: String(order.fabricDetails || "").slice(0, 1500),
    estimatedCost: Math.max(0, Number(order.estimatedCost) || 0),
    depositPaid: Math.max(0, Number(order.depositPaid) || 0),
    dueDate,
    deliveryDate,
    revision: Math.max(1, parseInt(order.revision, 10) || 1),
    notes: Array.isArray(order.notes) ? order.notes : [],
    supplies: Array.isArray(order.supplies) ? order.supplies : [],
    images: cleanImages,
    createdAt: String(order.createdAt || new Date().toISOString()),
    updatedAt: String(order.updatedAt || new Date().toISOString()),
  };
}

/**
 * Sanitiza un recordatorio de `agenda` garantizando estrictamente:
 * - `dueDate`: solo fecha `YYYY-MM-DD` (10 caracteres)
 * - `dueTime`: `HH:mm` (5 caracteres, 24h) o `null`
 */
function sanitizeReminderForFirestore(rem) {
  let dueDate = String(rem.dueDate || "").trim();
  let dueTime = rem.dueTime != null ? String(rem.dueTime).trim() : null;

  if (dueDate.includes("T")) {
    const parts = dueDate.split("T");
    dueDate = parts[0].slice(0, 10);
    if (!dueTime && parts[1]) {
      dueTime = parts[1].slice(0, 5);
    }
  } else {
    dueDate = dueDate.slice(0, 10);
  }

  if (dueTime && !/^\d{2}:\d{2}$/.test(dueTime)) {
    dueTime = dueTime.slice(0, 5);
    if (!/^\d{2}:\d{2}$/.test(dueTime)) {
      dueTime = null;
    }
  }
  if (!dueTime) {
    dueTime = null;
  }

  return {
    id: String(rem.id),
    title: String(rem.title || "").slice(0, 140),
    description: String(rem.description || "").slice(0, 1500),
    dueDate,
    dueTime,
    priority: String(rem.priority || "normal"),
    completed: Boolean(rem.completed),
    customerId: rem.customerId ? String(rem.customerId) : null,
    orderId: rem.orderId ? String(rem.orderId) : null,
    createdAt: String(rem.createdAt || new Date().toISOString()),
    updatedAt: String(rem.updatedAt || new Date().toISOString()),
  };
}

/**
 * Ejecuta una lista de operaciones `{ ref, data, merge }` en lotes atómicos `WriteBatch`
 * de máximo 450 operaciones por lote (límite de Firestore: 500).
 */
async function commitInBatches(db, operations) {
  const CHUNK_SIZE = 450;
  let committedCount = 0;
  for (let i = 0; i < operations.length; i += CHUNK_SIZE) {
    const slice = operations.slice(i, i + CHUNK_SIZE);
    const batch = writeBatch(db);
    for (const op of slice) {
      if (op.merge) {
        batch.set(op.ref, op.data, { merge: true });
      } else {
        batch.set(op.ref, op.data);
      }
    }
    await batch.commit();
    committedCount += slice.length;
  }
  return committedCount;
}

/**
 * Inicializa y materializa la raíz `/talleres/{uid}` y las 6 subcolecciones
 * (`clientes`, `pedidos`, `insumos`, `movimientos`, `agenda`, `auditoria`) en Cloud Firestore.
 * Si `materializeAllTables` es `true`, garantiza que cada una de las 6 subcolecciones
 * tenga al menos 1 documento válido para que todas las tablas aparezcan creadas en Firebase Console.
 */
export async function syncAndMaterializeWorkshop({ materializeAllTables = true } = {}) {
  await initFromEnv();
  if (!runtime.db) {
    throw new Error("Firestore no está inicializado. Verifica tu archivo .env.");
  }
  const user = runtime.auth && runtime.auth.currentUser;
  if (!user || !user.uid) {
    throw new Error(
      "Debes iniciar sesión con tu cuenta de Firebase (Correo/Contraseña o Google) para que firestore.rules autorice la escritura en /talleres/{uid}."
    );
  }

  const uid = user.uid;
  const invoke = getTauriInvoke();
  if (!invoke) {
    throw new Error("El comando de Tauri no está disponible.");
  }

  const commandName = materializeAllTables
    ? "get_firestore_materialized_snapshot"
    : "get_firestore_export_preview";

  const snapshot = await invoke(commandName, { uid });
  const db = runtime.db;
  const ops = [];

  // 1. Documento raíz: /talleres/{uid}
  const workshopRef = doc(db, "talleres", uid);
  ops.push({
    ref: workshopRef,
    data: {
      version: Number(snapshot.workshopDoc.version) || 2,
      workshopName: String(snapshot.workshopDoc.workshopName || "Mi taller de costura").slice(0, 120),
      ownerName: String(snapshot.workshopDoc.ownerName || "").slice(0, 120),
      currency: String(snapshot.workshopDoc.currency || "$").slice(0, 10),
      taxRatePercent: Math.min(100, Math.max(0, Number(snapshot.workshopDoc.taxRatePercent) || 0)),
      taxLabel: String(snapshot.workshopDoc.taxLabel || "IVA").slice(0, 20),
      defaultDueDays: Math.min(365, Math.max(1, parseInt(snapshot.workshopDoc.defaultDueDays, 10) || 7)),
      language: ["es", "en", "pt"].includes(snapshot.workshopDoc.language)
        ? snapshot.workshopDoc.language
        : "es",
      updatedAt: String(snapshot.workshopDoc.updatedAt || new Date().toISOString()),
    },
    merge: true,
  });

  // 2. Subcolección: clientes (/talleres/{uid}/clientes/{clienteId})
  const clientes = snapshot.collections.clientes || [];
  for (const c of clientes) {
    ops.push({
      ref: doc(db, "talleres", uid, "clientes", String(c.id)),
      data: {
        id: String(c.id),
        fullName: String(c.fullName || "").slice(0, 120),
        phone: String(c.phone || "").slice(0, 40),
        email: String(c.email || "").slice(0, 160),
        address: String(c.address || "").slice(0, 300),
        notes: String(c.notes || "").slice(0, 2000),
        pendingBalance: Math.max(0, Number(c.pendingBalance) || 0),
        activeOrdersCount: Math.max(0, parseInt(c.activeOrdersCount, 10) || 0),
        createdAt: String(c.createdAt || new Date().toISOString()),
        updatedAt: String(c.updatedAt || new Date().toISOString()),
      },
      merge: true,
    });
  }

  // 3. Subcolección: pedidos (/talleres/{uid}/pedidos/{pedidoId})
  const pedidos = snapshot.collections.pedidos || [];
  for (const o of pedidos) {
    ops.push({
      ref: doc(db, "talleres", uid, "pedidos", String(o.id)),
      data: sanitizeOrderForFirestore(o, uid),
      merge: true,
    });
  }

  // 4. Subcolección: insumos (/talleres/{uid}/insumos/{insumoId})
  const insumos = snapshot.collections.insumos || [];
  for (const s of insumos) {
    ops.push({
      ref: doc(db, "talleres", uid, "insumos", String(s.id)),
      data: {
        id: String(s.id),
        name: String(s.name || "").slice(0, 120),
        category: String(s.category || "otro"),
        unit: String(s.unit || "unidad"),
        stock: Number(s.stock) || 0,
        minStock: Math.max(0, Number(s.minStock) || 0),
        unitCost: Math.max(0, Number(s.unitCost) || 0),
        supplier: String(s.supplier || "").slice(0, 160),
        notes: String(s.notes || "").slice(0, 1000),
        createdAt: String(s.createdAt || new Date().toISOString()),
        updatedAt: String(s.updatedAt || new Date().toISOString()),
      },
      merge: true,
    });
  }

  // 5. Subcolección: movimientos (/talleres/{uid}/movimientos/{movId}) — Inmutable (allow create; allow update: if false)
  // Verificamos cuáles ya existen para no disparar `update` sobre documentos inmutables
  const existingMovIds = new Set();
  try {
    const movSnap = await getDocs(collection(db, "talleres", uid, "movimientos"));
    movSnap.forEach((d) => existingMovIds.add(d.id));
  } catch (_) {
    /* ignore if offline or empty */
  }

  const movimientos = snapshot.collections.movimientos || [];
  let createdMovements = 0;
  for (const m of movimientos) {
    const movId = String(m.id);
    if (existingMovIds.has(movId)) continue;
    ops.push({
      ref: doc(db, "talleres", uid, "movimientos", movId),
      data: {
        id: movId,
        supplyId: String(m.supplyId),
        timestamp: String(m.timestamp || new Date().toISOString()),
        delta: Number(m.delta) || 0,
        kind: ["entrada", "salida", "devolucion", "ajuste"].includes(m.kind) ? m.kind : "ajuste",
        reason: String(m.reason || "Movimiento de inventario").slice(0, 200),
        orderId: m.orderId ? String(m.orderId) : null,
        orderRevision: m.orderRevision != null ? Number(m.orderRevision) : null,
      },
      merge: false, // Creación pura (inmutable)
    });
    createdMovements++;
  }

  // 6. Subcolección: agenda (/talleres/{uid}/agenda/{recordatorioId})
  const agenda = snapshot.collections.agenda || [];
  for (const r of agenda) {
    ops.push({
      ref: doc(db, "talleres", uid, "agenda", String(r.id)),
      data: sanitizeReminderForFirestore(r),
      merge: true,
    });
  }

  // 7. Subcolección: auditoria (/talleres/{uid}/auditoria/{eventoId}) — Inmutable (allow create; allow update, delete: if false)
  const existingAudIds = new Set();
  try {
    const audSnap = await getDocs(collection(db, "talleres", uid, "auditoria"));
    audSnap.forEach((d) => existingAudIds.add(d.id));
  } catch (_) {
    /* ignore */
  }

  const auditoria = snapshot.collections.auditoria || [];
  let createdAudits = 0;
  for (const a of auditoria) {
    const audId = String(a.id);
    if (existingAudIds.has(audId)) continue;
    ops.push({
      ref: doc(db, "talleres", uid, "auditoria", audId),
      data: {
        id: audId,
        timestamp: String(a.timestamp || new Date().toISOString()),
        action: String(a.action || "workshop.sync").slice(0, 80),
        entityType: String(a.entityType || "talleres").slice(0, 40),
        entityId: String(a.entityId || uid).slice(0, 120),
        summary: String(a.summary || "Sincronización de taller"),
      },
      merge: false, // Creación pura (inmutable)
    });
    createdAudits++;
  }

  const totalWritten = await commitInBatches(db, ops);

  const summary = {
    uid,
    rootPath: `/talleres/${uid}`,
    totalOperations: totalWritten,
    counts: {
      talleres: 1,
      clientes: clientes.length,
      pedidos: pedidos.length,
      insumos: insumos.length,
      movimientos: createdMovements,
      agenda: agenda.length,
      auditoria: createdAudits,
    },
    syncedAt: new Date().toISOString(),
  };

  runtime.lastSyncSummary = summary;
  notifyListeners();
  return summary;
}

/**
 * Registra un descuento/devolución atómica de insumo en Firestore usando
 * `WriteBatch` + `increment(-delta)` + ID determinista `mov_{orderId}_r{revision}_{supplyId}`.
 */
export async function applySupplyDeltaTransaction({
  orderId,
  revision,
  supplyId,
  delta,
  pieceName,
}) {
  await initFromEnv();
  const user = runtime.auth && runtime.auth.currentUser;
  if (!runtime.db || !user) return null;

  const numDelta = Number(delta) || 0;
  if (Math.abs(numDelta) < 1e-9) return null;

  const uid = user.uid;
  const movId = deterministicMovementId(orderId, revision, supplyId);
  const movRef = doc(runtime.db, "talleres", uid, "movimientos", movId);
  const existing = await getDoc(movRef);
  if (existing.exists()) {
    return { movId, skipped: true };
  }

  const supplyRef = doc(runtime.db, "talleres", uid, "insumos", supplyId);
  const batch = writeBatch(runtime.db);
  batch.update(supplyRef, {
    stock: increment(-numDelta),
    updatedAt: new Date().toISOString(),
  });
  batch.set(movRef, {
    id: movId,
    supplyId: String(supplyId),
    timestamp: new Date().toISOString(),
    delta: -numDelta,
    kind: numDelta > 0 ? "salida" : "devolucion",
    reason: `Pedido r${revision}: ${pieceName || orderId}`.slice(0, 200),
    orderId: String(orderId),
    orderRevision: Number(revision) || 1,
  });

  await batch.commit();
  return { movId, skipped: false };
}

/**
 * Sube una imagen comprimida (<= 400 KB WebP/JPEG) a Firebase Storage bajo
 * `/talleres/{uid}/pedidos/{orderId}/{imageId}.webp` y actualiza el gestor local
 * de caché LRU (`150 MB`) desmarcando `pending_upload`.
 */
export async function uploadOrderImage(orderId, imageId, dataUrl) {
  const invoke = getTauriInvoke();
  if (invoke && dataUrl) {
    try {
      await invoke("cache_order_image", {
        orderId: String(orderId),
        imageId: String(imageId),
        payload: String(dataUrl),
        pendingUpload: true,
      });
    } catch (_) {
      /* cache write warning non-fatal */
    }
  }

  await initFromEnv();
  const user = runtime.auth && runtime.auth.currentUser;
  if (!runtime.storage || !user) {
    throw new Error("Debes iniciar sesión en Firebase para subir imágenes a Storage.");
  }
  const path = `talleres/${user.uid}/pedidos/${orderId}/${imageId}.webp`;
  const ref = storageRef(runtime.storage, path);
  await uploadString(ref, dataUrl, "data_url");
  const downloadUrl = await getDownloadURL(ref);

  if (invoke) {
    try {
      await invoke("mark_cached_image_uploaded", {
        orderId: String(orderId),
        imageId: String(imageId),
      });
    } catch (_) {
      /* non-fatal */
    }
  }

  return {
    id: imageId,
    storagePath: `/${path}`,
    downloadUrl,
  };
}

/**
 * Sincroniza en segundo plano el estado local hacia Firestore cuando hay un usuario autenticado,
 * sin bloquear la interfaz local.
 */
export async function backgroundSyncIfAuthenticated() {
  if (!runtime.auth || !runtime.auth.currentUser || !runtime.db) {
    return null;
  }
  try {
    return await syncAndMaterializeWorkshop({ materializeAllTables: false });
  } catch (_) {
    return null;
  }
}

const BridgeAPI = {
  get slots() {
    return runtime.slots;
  },
  getWorkshopPaths,
  deterministicMovementId,
  isCloudReady,
  getStatus,
  onStatusChange,
  initFromEnv,
  translateAuthError,
  loginWithEmail,
  registerWithEmail,
  loginWithGoogle,
  resetPassword,
  logout,
  syncAndMaterializeWorkshop,
  backgroundSyncIfAuthenticated,
  applySupplyDeltaTransaction,
  uploadOrderImage,
};

window.CosturaFirebaseBridge = BridgeAPI;
initFromEnv().catch(() => {});

export default BridgeAPI;
