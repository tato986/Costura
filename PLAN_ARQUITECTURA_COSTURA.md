# PLAN DE ARQUITECTURA E INGENIERÍA DE SEGURIDAD — TALLER COSTURA v2.0

**Proyecto:** Taller Costura (`JLRDev.TallerCostura`)  
**Alcance:** Evolución de arquitectura de escritorio local (`Rust + Tauri 2 + costura.json`) a ecosistema multiplataforma sincronizado en tiempo real y *offline-first* (`Google Firebase` + `Android Kotlin/Jetpack Compose` + `Windows/macOS Tauri 2 + MSIX` + `iOS SwiftUI`).  
**Marco Normativo de Seguridad:** ISO/IEC 27001:2022, ISO/IEC 27002:2022 e ISO/IEC 27017:2015 (Seguridad en Servicios Cloud).  
**Estado del Documento:** Aprobado con Ajustes Técnicos y Decisiones de Arquitectura Cerradas — Listo para Ejecución de Fase 1.

---

## Parte 1: Arquitectura Firebase, Esquema Unificado y Seguridad (ISO/IEC 27001, 27002 y 27017)

### 1.1. Migración de `costura.json` a Cloud Firestore (Offline-First)

En la versión actual (`src-tauri/src/model.rs` y `store.rs`), toda la información del taller vive en un único documento monolítico `costura.json` con las colecciones en memoria (`customers`, `orders`, `reminders`, `supplies`, `movements`, `settings`). Para escalar a múltiples dispositivos concurrentes sin colisiones de escritura ni consumo excesivo de ancho de banda:

1. **Particionado Multi-Tenant Aislado por Usuario (`uid`):**
   - Cada taller se aloja bajo una raíz lógica estricta: `/talleres/{uid}`.
   - Ningún documento existe fuera del ámbito de un `uid` autenticado.
   - **Entorno Parametrizado y Emulator-First:** La configuración de inicialización se construye con plantillas parametrizadas (`.env.example`, `google-services.template.json`, `firebase.json`) conectadas en desarrollo a **Firebase Emulator Suite** (`Auth :9099`, `Firestore :8080`, `Storage :9199`), permitiendo auditar reglas y flujos offline antes de enlazar el `projectId` de producción.
2. **Desacople en Subcolecciones Independientes:**
   - `/talleres/{uid}` (Documento raíz: contiene `settings`, incluyendo la tasa impositiva opcional `taxRatePercent`, y metadatos del taller).
   - `/talleres/{uid}/clientes/{clienteId}`
   - `/talleres/{uid}/pedidos/{pedidoId}`
   - `/talleres/{uid}/insumos/{insumoId}`
   - `/talleres/{uid}/movimientos/{movimientoId}` (Kárdex transaccional de insumos con clave de idempotencia determinista).
   - `/talleres/{uid}/agenda/{recordatorioId}`
   - `/talleres/{uid}/auditoria/{eventoId}` (Bitácora inmutable de eventos críticos).
3. **Motor Offline-First y Sincronización Incremental:**
   - **Persistencia Local Activa:** Se habilita el caché persistente de Firestore (SQLite nativo en Android e iOS, IndexedDB en Tauri/WebView2/WKWebView). El taller puede operar sin conexión; las lecturas se resuelven contra el disco local con latencia `< 5 ms` y las mutaciones se encolan automáticamente.
   - **Sincronización Delta (`updatedAt`):** Todos los documentos incorporan `updatedAt` (marca de tiempo UTC ISO-8601) y `revision` (contador entero monótono por edición de pedido) para trazabilidad de cambios entre dispositivos desconectados.
   - **Importador y Respaldo `costura.json`:** La aplicación de escritorio conservará la capacidad de importar un archivo `costura.json` heredado hacia Firestore mediante lotes atómicos (`WriteBatch` de máximo 500 operaciones) y exportar en cualquier momento una instantánea completa a `costura.json` como copia de seguridad soberana en disco local.

---

### 1.2. Autenticación y Control de Acceso (`Firebase Auth` + Custodia Segura en SO)

1. **Proveedores Habilitados:**
   - **Fase 1 en adelante:** Correo electrónico y contraseña (con requisito de entropía mínima y verificación de correo) + **Google Sign-In (OAuth 2.0)**.
   - **Fase de ecosistema Apple (iOS):** Se añade **Sign In with Apple** para cumplimiento estricto de App Store Review Guidelines.
2. **Gestión de Sesiones y Custodia Cifrada de Tokens a Nivel de Sistema Operativo (ISO/IEC 27002 Control 5.17):**
   - Tokens JWT firmados por Google (`ID Token` con vigencia de 1 hora, renovados mediante `Refresh Token`).
   - **Android:** Almacenamiento respaldado por hardware en *Android Keystore* (`EncryptedSharedPreferences` / Jetpack DataStore con Tink AES-256-GCM).
   - **Windows y macOS (Tauri 2 / Rust):** En lugar de depender únicamente de la sesión en `IndexedDB` dentro del perfil del WebView2/WKWebView, el backend en Rust utiliza el crate **`keyring`** para custodiar los tokens de sesión (`refresh_token` y credenciales de sesión) directamente en el **Administrador de Credenciales de Windows (*Windows Credential Manager* / DPAPI)** y en el **Llavero de macOS (*macOS Keychain*)**. Al iniciar la app, Rust recupera el token del llavero del SO y lo inyecta en memoria al cliente Firebase; al cerrar sesión, Rust purga la entrada del Administrador de Credenciales.
   - **iOS:** Almacenamiento nativo en *iOS Keychain Services* con atributo `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`.
3. **Recuperación de Cuenta y Protección contra Fuerza Bruta:**
   - Flujo nativo de restablecimiento de contraseña vía enlace firmado de un solo uso (`sendPasswordResetEmail`).
   - Activación de **Firebase App Check** (Play Integrity en Android, DeviceCheck/App Attest en iOS, token de verificación en Desktop) para impedir que clientes o scripts no autorizados consuman cuotas de Firebase.

---

### 1.3. Controles de Seguridad y Privacidad (Alineación ISO/IEC 27001, 27002 y 27017)

| Dominio / Control | Norma de Referencia | Implementación Técnica en "Taller Costura" |
| :--- | :--- | :--- |
| **Control de Acceso Lógico y Aislamiento Multi-Tenant** | ISO/IEC 27001 A.5.15, A.8.3 / ISO 27017 CLD.9.5.1 | Reglas `firestore.rules` y `storage.rules` con evaluación estricta `request.auth != null && request.auth.uid == uid`. Prohibición total de lecturas globales o transversales entre talleres. |
| **Custodia de Información de Autenticación** | ISO/IEC 27002 5.17 | Uso del crate `keyring` en Rust para guardar credenciales de sesión en **Windows Credential Manager** (DPAPI) y **macOS Keychain**, y *Android Keystore* / *iOS Keychain* en dispositivos móviles, evitando tokens expuestos en texto plano en perfiles del navegador. |
| **Cifrado en Tránsito y en Reposo** | ISO/IEC 27002 8.24 | **Tránsito:** TLS 1.3 obligatorio en todas las conexiones hacia `*.googleapis.com` y `*.firebaseio.com`. **Reposo Cloud:** AES-256 gestionado en Google Cloud. **Reposo Local:** Almacenamiento en rutas protegidas por ACL de usuario del SO (`%LOCALAPPDATA%` en Windows, Sandbox privado en Android/iOS/macOS). |
| **Validación Estricta de Entradas** | ISO/IEC 27002 8.26, 8.28 | Validación en tres capas: UI del cliente, lógica de dominio (Kotlin / Rust / Swift) y validación de esquema dentro de `firestore.rules` (longitud máxima de strings, rangos numéricos `>= 0`, tasa impositiva `0..100`, enumeraciones cerradas para estados y tipos de prenda). |
| **Registros de Auditoría (Logging)** | ISO/IEC 27002 8.15 | Subcolección `/talleres/{uid}/auditoria/{eventoId}` de solo creación (`allow create: if ...; allow update, delete: if false;`) que registra acciones sensibles: borrado de clientes, anulación de pedidos, ajustes manuales de inventario y exportaciones contables. |
| **Gestión Segura de Credenciales y Secretos** | ISO/IEC 27002 5.17, 8.12 | Exclusión estricta en `.gitignore` de `google-services.json`, `GoogleService-Info.plist`, `.env` y certificados `.pfx` de firma MSIX. Uso de plantillas parametrizadas en el repositorio. Restricción de API Keys en Google Cloud Console por SHA-256 de paquete Android, Bundle ID de Apple y dominios de Tauri. |
| **Privacidad, Retención y Borrado Seguro de PII** | ISO/IEC 27017 CLD.8.1.5 / ISO 27001 A.8.10 | Los datos personales de clientes (teléfono, dirección, correo, medidas anatómicas y saldos financieros) incluyen mecanismos de: **1) Minimización:** campos opcionales no forzados; **2) Portabilidad:** exportación directa a JSON/XLSX; **3) Derecho de Supresión (Borrado Seguro):** función de *"Eliminar Cuenta y Datos del Taller"* que purga en cascada Firestore, limpia los objetos de `/talleres/{uid}/pedidos/*` en Cloud Storage, borra la caché LRU local, limpia el llavero del SO (`keyring`) y elimina el usuario en Firebase Auth. |

---

### 1.4. Modelado de Datos Unificado (Kotlin, Swift, Rust y TypeScript/JS)

El esquema respeta 100 % la semántica actual de `src-tauri/src/model.rs` (`camelCase` en serialización JSON/Firestore) e incorpora el campo fiscal configurable (`taxRatePercent`) y metadatos de control de concurrencia offline (`revision`, `suppliesSnapshotHash`).

#### A. Documento Raíz del Taller (`/talleres/{uid}`)
| Campo | Tipo Firestore | Tipo Rust | Tipo Kotlin | Tipo Swift | Descripción / Restricción |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `version` | `int` | `u32` | `Int` | `Int` | Versión del esquema (`2`) |
| `workshopName` | `string` | `String` | `String` | `String` | Nombre del taller (`1..120` chars) |
| `ownerName` | `string` | `String` | `String` | `String` | Nombre del titular (`0..120` chars) |
| `currency` | `string` | `String` | `String` | `String` | Símbolo o código ISO (`"$"`, `"MXN"`, `"EUR"`) |
| `taxRatePercent`| `number` (float)| `f64` | `Double` | `Double` | **Nuevo:** Tasa impositiva configurable (ej. `0.0` por defecto, `16.0` IVA México, `21.0` España; rango `0.0..100.0`) |
| `taxLabel` | `string` | `String` | `String` | `String` | **Nuevo:** Etiqueta del impuesto en reportes (`"IVA"`, `"IGV"`, `"Impuesto"`, `0..20` chars) |
| `defaultDueDays`| `int` | `i64` | `Long` | `Int` | Días por defecto para entrega (`1..365`) |
| `language` | `string` | `String` | `String` | `String` | `"es"`, `"en"`, `"pt"` |
| `updatedAt` | `string` (ISO) | `String` | `String` | `String` | Timestamp UTC ISO-8601 |

#### B. Colección `clientes` (`/talleres/{uid}/clientes/{id}`)
| Campo | Tipo Firestore | Tipo Rust | Tipo Kotlin | Tipo Swift | Descripción / Restricción |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `id` | `string` (UUID) | `String` | `String` | `String` | UUID v4 (coincide con ID del documento) |
| `fullName` | `string` | `String` | `String` | `String` | Obligatorio (`1..120` chars) |
| `phone` | `string` | `String` | `String` | `String` | Teléfono/WhatsApp (`0..40` chars) |
| `email` | `string` | `String` | `String` | `String` | Correo opcional (`0..120` chars) |
| `address` | `string` | `String` | `String` | `String` | Dirección física (`0..250` chars) |
| `notes` | `string` | `String` | `String` | `String` | Preferencias o notas del cliente (`0..1000` chars) |
| `pendingBalance`| `number` (float)| `f64` | `Double` | `Double` | Saldo por cobrar actualizado vía `FieldValue.increment` |
| `activeOrdersCount`| `int` | `i64` | `Int` | `Int` | Conteo de pedidos activos actualizado vía `FieldValue.increment` |
| `createdAt` | `string` (ISO) | `String` | `String` | `String` | Fecha de alta UTC ISO-8601 |
| `updatedAt` | `string` (ISO) | `String` | `String` | `String` | Última modificación UTC ISO-8601 |

#### C. Colección `pedidos` (`/talleres/{uid}/pedidos/{id}`)
Soporta las prendas estándar (`camisa`, `pantalon`, `vestido`, `bolso`) y la categoría flexible **`otro`** (pestaña **"Otros"** en la interfaz para proyectos personalizados como llaveros, composturas, dobladillos, cortinas o arreglos).

| Campo | Tipo Firestore | Tipo Rust | Tipo Kotlin | Tipo Swift | Descripción / Restricción |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `id` | `string` (UUID) | `String` | `String` | `String` | UUID v4 |
| `customerId` | `string` | `String` | `String` | `String` | Referencia a `clientes/{id}` |
| `pieceName` | `string` | `String` | `String` | `String` | Nombre del trabajo (`1..140` chars) |
| `garmentType` | `string` | `String` | `String` | `String` | Enum: `"camisa"`, `"pantalon"`, `"vestido"`, `"bolso"`, `"otro"` |
| `customType` | `string` | `String` | `String` | `String` | Subcategoría libre cuando `garmentType == "otro"` (ej. `"Llavero"`, `"Arreglo cierre"`) |
| `styleName` | `string` | `String` | `String` | `String` | Corte/estilo (ej. `"Slim"`, `"Recto"`, `"A medida"`) |
| `status` | `string` | `String` | `String` | `String` | Enum bitácora: `"pendiente"`, `"en_corte"`, `"en_confeccion"`, `"prueba_ajuste"`, `"terminado"`, `"entregado"` |
| `priority` | `string` | `String` | `String` | `String` | Enum: `"normal"`, `"urgente"`, `"prioritaria"` |
| `measurements` | `map` | `Measurements`| `Measurements`| `Measurements`| Objeto `{ kind: string, values: Map<String, Double>, options: Map<String, String>, notes: string }` (valores en **cm**) |
| `fabricDetails` | `string` | `String` | `String` | `String` | Descripción de tela y acabados (`0..1000` chars) |
| `estimatedCost` | `number` | `f64` | `Double` | `Double` | Precio total acordado (`>= 0.0`) |
| `depositPaid` | `number` | `f64` | `Double` | `Double` | Anticipo pagado (`>= 0.0`) |
| `dueDate` | `string` | `String` | `String` | `String` | Fecha compromiso `YYYY-MM-DD` |
| `deliveryDate` | `string?` | `Option<String>`| `String?` | `String?` | Fecha real de entrega `YYYY-MM-DD` o `null` |
| `notes` | `array<map>` | `Vec<OrderNote>`| `List<OrderNote>`| `[OrderNote]` | Bitácora cronológica `{ id, timestamp, stage, content }` |
| `supplies` | `array<map>` | `Vec<SupplyUsage>`| `List<SupplyUsage>`| `[SupplyUsage]`| Insumos vinculados `{ supplyId, quantity, cut, note }` |
| `images` | `array<map>` | `Vec<OrderImage>`| `List<OrderImage>`| `[OrderImage]`| Referencias Storage `{ id, storagePath, downloadUrl, width, height, sizeBytes }` |
| `revision` | `int` | `u64` | `Long` | `Int64` | **Nuevo:** Contador monótono incrementado en cada edición del pedido (`1, 2, 3...`) |
| `createdAt` | `string` (ISO) | `String` | `String` | `String` | Timestamp UTC ISO-8601 |
| `updatedAt` | `string` (ISO) | `String` | `String` | `String` | Timestamp UTC ISO-8601 |

#### D. Colección `insumos` (`/talleres/{uid}/insumos/{id}`) y `movimientos` (`/talleres/{uid}/movimientos/{id}`)
| Campo (`insumos`) | Tipo Firestore | Tipo Rust | Tipo Kotlin | Tipo Swift | Descripción / Restricción |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `id` | `string` (UUID) | `String` | `String` | `String` | UUID v4 |
| `name` | `string` | `String` | `String` | `String` | Nombre del insumo (`1..120` chars) |
| `category` | `string` | `String` | `String` | `String` | Enum: `"tela"`, `"hilo"`, `"boton"`, `"cierre"`, `"elastico"`, `"entretela"`, `"otro"` |
| `unit` | `string` | `String` | `String` | `String` | Enum: `"yarda"`, `"metro"`, `"unidad"`, `"cono"`, `"rollo"`, `"par"`, `"otro"` |
| `stock` | `number` | `f64` | `Double` | `Double` | Existencia actual modificada exclusivamente vía `FieldValue.increment` |
| `minStock` | `number` | `f64` | `Double` | `Double` | Umbral de alerta de stock bajo (`>= 0.0`) |
| `unitCost` | `number` | `f64` | `Double` | `Double` | Costo por unidad (`>= 0.0`) |
| `supplier` | `string` | `String` | `String` | `String` | Proveedor habitual (`0..120` chars) |
| `notes` | `string` | `String` | `String` | `String` | Color, lote, ancho de rollo (`0..500` chars) |
| `createdAt` / `updatedAt` | `string` (ISO) | `String` | `String` | `String` | Timestamps UTC ISO-8601 |

#### E. Resolución de Conflictos Offline en `WriteBatch`, `FieldValue.increment` e Idempotencia

Cuando dos dispositivos del taller (por ejemplo, el teléfono Android en la mesa de corte y la PC Windows en recepción) operan sin conexión a internet y posteriormente reconectan, el sistema aplica una estrategia de **convergencia determinista dividida en tres reglas**:

1. **Comportamiento Conmutativo de `FieldValue.increment` (Stock de Insumos y Saldos de Clientes):**
   - El campo `stock` de `/insumos/{supplyId}` y el campo `pendingBalance` de `/clientes/{customerId}` **nunca se sobrescriben con valores absolutos calculados por un cliente** (`stock = 8.0`), sino que se mutan exclusivamente mediante transformaciones de servidor `FieldValue.increment(-delta)`.
   - **Caso de concurrencia offline:** Si el Insumo $X$ tiene `10.0 m`, el Dispositivo A (offline) consume `2.0 m` en el Pedido 1 (`increment(-2.0)`) y el Dispositivo B (offline) consume `3.5 m` en el Pedido 2 (`increment(-3.5)`). Al reconectar ambos dispositivos en cualquier orden, Firestore aplica ambas operaciones conmutativas sobre la base real: $10.0 - 2.0 - 3.5 = 4.5\text{ m}$. Ningún descuento se pierde por condición de carrera.
2. **Comportamiento de Campos Descriptivos (`Last-Write-Wins` con `mergeFields` y `updatedAt`):**
   - Para campos descriptivos de un documento (`pieceName`, `status`, `fabricDetails`, `dueDate`), Firestore aplica semántica *Last-Write-Wins (LWW)* ordenada por el instante de recepción de la cola offline, registrando `updatedAt`.
   - Para minimizar colisiones en un mismo pedido editado desde dos dispositivos offline, las actualizaciones puntuales de estado en la bitácora (`set_order_status` o `add_order_note`) utilizan actualizaciones parciales (`update(mapOf("status" to newStatus, "notes" to FieldValue.arrayUnion(note), "updatedAt" to now))`) en lugar de sobrescribir el documento `Order` completo. Así, si el Dispositivo A añade una nota de bitácora y el Dispositivo B cambia la fecha de entrega, ambos cambios se fusionan sin pisarse.
3. **Prevención de Descuentos Duplicados al Reintentar o Editar un Pedido (`Idempotencia por Revisión`):**
   - **El riesgo:** Si un usuario edita un pedido varias veces o la cola offline reintenta un lote, no debe descontarse dos veces el mismo insumo.
   - **El mecanismo técnico (`apply_supply_deltas` transaccional en `WriteBatch`):**
     - Cada documento `Order` almacena la lista exacta de insumos actualmente aplicados a ese pedido (`order.supplies: List<SupplyUsage>`) y su número de versión `order.revision` (`1, 2, 3...`).
     - Al editar un pedido existente, el cliente lee el estado previo persistido localmente (`previousOrder.supplies` y `previousOrder.revision`), calcula únicamente el diferencial neto por insumo:
       $$\Delta_{\text{supplyId}} = \text{cantidad}_{\text{nueva}} - \text{cantidad}_{\text{previa}}$$
     - Si $\Delta_{\text{supplyId}} == 0$, no se emite ninguna operación de stock para ese insumo.
     - Si $|\Delta_{\text{supplyId}}| > 10^{-9}$, dentro del **mismo `WriteBatch` atómico** se encolan tres escrituras vinculadas:
       1. Actualización del pedido con la nueva lista `supplies` y `revision = previousOrder.revision + 1`.
       2. `FieldValue.increment(-delta)` sobre `/insumos/{supplyId}`.
       3. Creación del registro de kárdex en `/movimientos/{movId}` cuyo **ID de documento es determinista**:
          $$\text{movId} = \text{``mov\_''} + \text{orderId} + \text{``\_r''} + \text{newRevision} + \text{``\_''} + \text{supplyId}$$
     - **Garantía contra duplicados:** Dado que las 3 operaciones viajan en un único `WriteBatch` local de Firestore y el `movId` es determinista por `(orderId, revision, supplyId)`, los reintentos de red de Firestore son idempotentes y una segunda edición en el dispositivo parte siempre del `previousOrder.supplies` ya actualizado en la caché local.
     - **Reconciliación ante edición concurrente extrema del mismo pedido en dos dispositivos offline:** Si dos dispositivos distintos modificaran los insumos del *mismo* pedido mientras ambos estaban offline (colisión en la misma `revision`), al detectar dos movimientos con la misma pareja `(orderId, revision)` o discrepancia entre $\sum \text{movimientos}(\text{orderId})$ y `order.supplies`, el cliente ejecuta una rutina ligera de reconciliación (`reconcileOrderSupplies(orderId)`) que compara la suma de deltas del kárdex para ese `orderId` contra `order.supplies` actual y emite un único movimiento de ajuste correctivo si existiera diferencia.

#### F. Colección `agenda` (`/talleres/{uid}/agenda/{id}`)
| Campo | Tipo Firestore | Tipo Rust | Tipo Kotlin | Tipo Swift | Descripción / Restricción |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `id` | `string` (UUID) | `String` | `String` | `String` | UUID v4 |
| `title` | `string` | `String` | `String` | `String` | Título del recordatorio (`1..140` chars) |
| `description` | `string` | `String` | `String` | `String` | Detalle (`0..600` chars) |
| `dueDate` | `string` | `String` | `String` | `String` | Fecha estricta `YYYY-MM-DD` (ej. `"2026-11-20"`, exactamente 10 caracteres, sin hora combinada) |
| `dueTime` | `string?` | `Option<String>`| `String?` | `String?` | Hora opcional `HH:mm` de 24h (ej. `"16:30"` o `null`) para alarma local |
| `priority` | `string` | `String` | `String` | `String` | Enum: `"normal"`, `"urgente"`, `"prioritaria"` |
| `completed` | `boolean` | `bool` | `Boolean` | `Bool` | Estado de cumplimiento |
| `customerId` | `string?` | `Option<String>`| `String?` | `String?` | Vínculo opcional a cliente |
| `orderId` | `string?` | `Option<String>`| `String?` | `String?` | Vínculo opcional a pedido |
| `createdAt` / `updatedAt` | `string` (ISO) | `String` | `String` | `String` | Timestamps UTC ISO-8601 |

---

### 1.5. Imágenes, Ligereza (`Firebase Storage`) y Caché Local Acotada (LRU `150 MB`)

Para evitar que las fotografías de cámaras móviles modernas saturen la cuota de Firebase Storage, el plan de datos móviles o la memoria interna de teléfonos de gama media/baja:

1. **Compresión Obligatoria en Cliente antes de Subir:**
   - Redimensionamiento proporcional con lado mayor máximo de **`1280 px`**.
   - Codificación en **WebP** (o JPEG de respaldo en API antigua) con factor de calidad **`75 %`**.
   - Eliminación de metadatos EXIF de geolocalización (GPS) por privacidad (ISO/IEC 27017), conservando únicamente la orientación corregida.
   - **Peso objetivo por imagen:** `60 KB – 180 KB` (límite duro en reglas de servidor: `400 KB`).
   - **Máximo por pedido:** `4` imágenes.
2. **Estructura de Rutas Aisladas por `uid`:**
   - `/talleres/{uid}/pedidos/{orderId}/{imageId}.webp`
3. **Política de Caché Local en Disco Acotada con Expulsión LRU (`150 MB` Máximo):**
   - Cada cliente gestiona un directorio dedicado de caché de imágenes (`context.cacheDir/order_images` en Android, `app_cache_dir/order_images` en Tauri Windows/macOS, `Library/Caches/order_images` en iOS).
   - **Límite duro de almacenamiento en disco:** **`150 MB`** (suficiente para almacenar localmente más de `1,000` fotografías comprimidas de pedidos recientes sin impacto en teléfonos de 32 GB / 64 GB).
   - **Algoritmo de Expulsión LRU (*Least Recently Used*):**
     - Cada vez que una imagen se lee o se escribe en la caché local, se actualiza su marca de tiempo de último acceso (`lastModified`).
     - Al guardar una nueva imagen en caché, el gestor `ImageDiskCache` verifica el tamaño total del directorio. Si supera los **`150 MB`**, ordena los archivos por `lastModified` ascendente (los más antiguos no consultados primero) y elimina archivos hasta reducir la ocupación al **`85 %` del límite (`127.5 MB`)**, evitando ejecuciones de limpieza en cada lectura individual.
     - Las imágenes pendientes de subida en la cola offline (`pending_upload_*.webp`) están marcadas como protegidas y **nunca** son expulsadas por el recolector LRU hasta que su subida a Firebase Storage se confirme.

---

### 1.6. Reglas de Seguridad (`firestore.rules` y `storage.rules`)

#### Diseño de `firestore.rules`
```javascript
rules_version = '2';
service cloud.firestore {
  match /databases/{database}/documents {

    function isOwner(uid) {
      return request.auth != null && request.auth.uid == uid;
    }

    function validString(val, minLen, maxLen) {
      return val is string && val.size() >= minLen && val.size() <= maxLen;
    }

    function validNonNegativeNumber(val) {
      return (val is int || val is float) && val >= 0;
    }

    function validTaxRate(val) {
      return (val is int || val is float) && val >= 0 && val <= 100;
    }

    match /talleres/{uid} {
      allow read: if isOwner(uid);
      allow create, update: if isOwner(uid)
        && validString(request.resource.data.workshopName, 1, 120)
        && validString(request.resource.data.currency, 1, 10)
        && validTaxRate(request.resource.data.taxRatePercent)
        && request.resource.data.language in ['es', 'en', 'pt'];
      allow delete: if isOwner(uid);

      match /clientes/{clienteId} {
        allow read, delete: if isOwner(uid);
        allow create, update: if isOwner(uid)
          && validString(request.resource.data.fullName, 1, 120)
          && validString(request.resource.data.phone, 0, 40)
          && validString(request.resource.data.address, 0, 250)
          && validString(request.resource.data.notes, 0, 1000);
      }

      match /pedidos/{pedidoId} {
        allow read, delete: if isOwner(uid);
        allow create, update: if isOwner(uid)
          && validString(request.resource.data.pieceName, 1, 140)
          && request.resource.data.garmentType in ['camisa', 'pantalon', 'vestido', 'bolso', 'otro']
          && request.resource.data.status in ['pendiente', 'en_corte', 'en_confeccion', 'prueba_ajuste', 'terminado', 'entregado']
          && request.resource.data.priority in ['normal', 'urgente', 'prioritaria']
          && validNonNegativeNumber(request.resource.data.estimatedCost)
          && validNonNegativeNumber(request.resource.data.depositPaid)
          && request.resource.data.images.size() <= 4;
      }

      match /insumos/{insumoId} {
        allow read, delete: if isOwner(uid);
        allow create, update: if isOwner(uid)
          && validString(request.resource.data.name, 1, 120)
          && request.resource.data.category in ['tela', 'hilo', 'boton', 'cierre', 'elastico', 'entretela', 'otro']
          && request.resource.data.unit in ['yarda', 'metro', 'unidad', 'cono', 'rollo', 'par', 'otro']
          && validNonNegativeNumber(request.resource.data.minStock)
          && validNonNegativeNumber(request.resource.data.unitCost);
      }

      match /movimientos/{movId} {
        allow read, delete: if isOwner(uid);
        allow create: if isOwner(uid)
          && request.resource.data.kind in ['entrada', 'salida', 'devolucion', 'ajuste'];
        allow update: if false; // El kárdex de movimientos es inmutable
      }

      match /agenda/{recordatorioId} {
        allow read, delete: if isOwner(uid);
        allow create, update: if isOwner(uid)
          && validString(request.resource.data.title, 1, 140)
          && request.resource.data.dueDate is string
          && request.resource.data.dueDate.matches('^\\d{4}-\\d{2}-\\d{2}$')
          && (!('dueTime' in request.resource.data) || request.resource.data.dueTime == null || (request.resource.data.dueTime is string && request.resource.data.dueTime.matches('^\\d{2}:\\d{2}$')))
          && request.resource.data.priority in ['normal', 'urgente', 'prioritaria']
          && request.resource.data.completed is bool;
      }

      match /auditoria/{eventoId} {
        allow read: if isOwner(uid);
        allow create: if isOwner(uid);
        allow update, delete: if false; // Pista de auditoría ISO 27002 inmutable
      }
    }
  }
}
```

#### Diseño de `storage.rules`
```javascript
rules_version = '2';
service firebase.storage {
  match /b/{bucket}/o {
    match /talleres/{uid}/pedidos/{orderId}/{fileName} {
      allow read, delete: if request.auth != null && request.auth.uid == uid;
      allow write: if request.auth != null
        && request.auth.uid == uid
        && request.resource.size <= 400 * 1024 // Máximo 400 KB por imagen comprimida
        && request.resource.contentType.matches('image/(webp|jpeg|png)');
    }
  }
}
```

---

## Parte 2: Sistema de Diseño Humano y Sobrio (Eliminación de "IA Slop")

### 2.1. Reglas Visuales Estrictas

En el CSS actual (`src/css/app.css`) existen tintes violetas en todos los fondos (`#f4f2f8`, `#120f1c`), un acento secundario magenta (`--accent-2: #db2777`), sombras de hasta `64px` (`--shadow-lg`) y bordes redondeados amplios (`20px`). Para lograr una herramienta de trabajo industrial, sobria y legible bajo iluminación de taller:

1. **Cero Degradados Decorativos:** Queda prohibido el uso de `linear-gradient` o `radial-gradient` en botones, tarjetas, fondos, barras laterales o insignias. Todos los colores son sólidos y planos.
2. **Eliminación de Sombras Difusas:** Las tarjetas, tablas y paneles laterales **no llevan `box-shadow`**. La jerarquía espacial se define exclusivamente mediante **bordes sólidos de `1px`** (`--border`) y contraste entre el lienzo de fondo (`--bg`) y la superficie de la tarjeta (`--surface`). Solo los diálogos modales flotantes y menús desplegables pueden usar una sombra técnica mínima de separación (`0 4px 12px rgba(0,0,0,0.12)`).
3. **Geometría Contenida y Utilitaria:** Radios de borde reducidos a `6px` para controles/botones (`--radius-sm`), `8px` para tarjetas/paneles (`--radius`) y `10px` para modales (`--radius-lg`).
4. **Tipografía y Datos Tabulares:** Uso obligatorio de `font-variant-numeric: tabular-nums` en todas las columnas de medidas (cm), saldos monetarios, fechas y existencias de insumos para alineación vertical exacta.
5. **Cero Textos de Relleno:** Se eliminan subtítulos redundantes o frases genéricas; cada vista muestra encabezados directos, contadores reales y acciones operativas.

---

### 2.2. Regla Estricta de Uso del Color Morado

- **El morado es exclusivamente un acento funcional de interacción y estado activo:**
  - Botón de acción primaria (`+ Nuevo pedido`, `Guardar`).
  - Indicador lateral/inferior de la pestaña activa en la navegación (y filtro de categoría activo: `Todos | Camisas | Pantalones | Vestidos | Bolsos | Otros`).
  - Paso actual en la línea de tiempo / bitácora del pedido (`Pendiente -> En corte -> En confección -> Prueba/Ajuste -> Terminado -> Entregado`).
  - Anillo de foco (`focus-visible`) en campos de formulario y casillas de verificación activas.
- **Dónde NUNCA debe usarse el morado:**
  - Nunca como color de fondo de tarjetas, encabezados de tabla o barra lateral.
  - Nunca teñir los grises neutros del fondo general con matices púrpura.
  - Se elimina por completo el acento secundario rosa/magenta (`--accent-2`).

---

### 2.3. Tokens Exactos para Modo Claro y Modo Oscuro

Los tokens están calibrados para cumplir un contraste **WCAG 2.1 AA/AAA** (`>= 7:1` en texto principal y `>= 4.5:1` en controles con acento morado) y evitar la vibración cromática sobre fondos oscuros. Se aplican de manera idéntica en Jetpack Compose (Material 3 `ColorScheme`), CSS (Tauri Windows/macOS) y SwiftUI.

| Token Semántico | Variable CSS (Tauri) | Token Compose / SwiftUI | Modo Claro (Hex) | Modo Oscuro (Hex) | Uso Específico |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Fondo Base** | `--bg` | `Background` | `#F6F6F7` | `#111214` | Lienzo general de la aplicación (neutro puro) |
| **Superficie 1** | `--surface` | `Surface` | `#FFFFFF` | `#18191C` | Tarjetas, tablas, modales y barra lateral |
| **Superficie 2** | `--surface-2` | `SurfaceVariant` | `#EFEFF1` | `#202226` | Filas alternas, cabeceras de tabla, inputs inactivos |
| **Superficie 3** | `--surface-3` | `SurfaceHover` | `#E4E4E7` | `#2A2C32` | Estado `:hover` en filas y botones secundarios |
| **Borde Fino (1px)** | `--border` | `OutlineVariant` | `#DCDCE0` | `#2E3036` | División estructural de `1px solid` en tarjetas y celdas |
| **Borde Fuerte** | `--border-strong` | `Outline` | `#B8B8BF` | `#454852` | Bordes de inputs activos o divisores principales |
| **Texto Principal** | `--text` | `OnSurface` | `#161618` | `#F2F2F4` | Títulos, nombres de clientes, importes y medidas |
| **Texto Secundario** | `--text-soft` | `OnSurfaceVariant` | `#4A4A52` | `#B8BAC2` | Etiquetas de formulario, metadatos secundarios |
| **Texto Atenuado** | `--muted` | `TextMuted` | `#71717A` | `#858894` | Placeholders, marcas de tiempo y notas al pie |
| **Morado Acento** | `--accent` | `Primary` | `#5B21B6` | `#A78BFA` | **Claro:** Morado profundo saturado. **Oscuro:** Morado luminoso lavado sin vibración |
| **Morado Hover** | `--accent-hover` | `PrimaryHover` | `#4C1D95` | `#C4B5FD` | Estado `:hover` / `:active` del botón primario |
| **Texto sobre Acento**| `--on-accent` | `OnPrimary` | `#FFFFFF` | `#140E24` | Texto/icono dentro del botón primario morado |
| **Superficie Acento** | `--accent-soft` | `PrimaryContainer` | `#F3EEFC` | `#231C35` | Fondo sutil de chip seleccionado o etapa actual |
| **Borde Acento** | `--accent-border` | `PrimaryBorder` | `#C4B5FD` | `#5B438A` | Borde `1px` de elemento activo o foco |
| **Éxito (Entregado)** | `--good` / `--good-soft`| `StatusSuccess` | `#15803D` / `#DCFCE7` | `#4ADE80` / `#132A1C` | Pedido entregado, saldo liquidado, stock óptimo |
| **Alerta (Prueba)** | `--warn` / `--warn-soft`| `StatusWarning` | `#B45309` / `#FEF3C7` | `#FBBF24` / `#2D2110` | Stock bajo mínimo, entrega próxima |
| **Peligro (Urgente)** | `--bad` / `--bad-soft` | `StatusDanger` | `#B91C1C` / `#FEE2E2` | `#F87171` / `#301717` | Pedido vencido/urgente, insumo agotado, borrar |

---

## Parte 3: Estrategia Multiplataforma Ligera (Android, Windows/macOS y iOS)

### 3.1. Android Nativo — Prioridad 1 de Implementación de Cliente (Kotlin + Jetpack Compose)

Diseñado para arrancar en frío en `< 800 ms` y funcionar con fluidez en teléfonos de gama media/baja con 3 GB – 4 GB de RAM utilizados habitualmente en el taller.

1. **Arquitectura MVVM Ligera (Sin sobre-ingeniería):**
   - **Capas:** `ui` (Jetpack Compose Material 3 con los tokens exactos de la Parte 2) -> `viewmodel` (`StateFlow` + `ViewModel`) -> `data` (`CosturaRepository` + Firebase SDK + `ImageDiskCache` LRU 150 MB).
   - **Cero frameworks pesados de inyección por reflexión:** Inyección manual mediante contenedor ligero (`AppContainer`) para ahorrar tiempo de arranque y tamaño de DEX.
2. **Configuración Offline-First en Firestore:**
   ```kotlin
   val settings = firestoreSettings {
       setLocalCacheSettings(
           persistentCacheSettings {
               setSizeBytes(100L * 1024L * 1024L) // 100 MB de caché SQLite local para documentos
           }
       )
   }
   Firebase.firestore.firestoreSettings = settings
   ```
3. **Vista de Pedidos y Pestaña "Otros":**
   - Barra horizontal de filtros (`ScrollableTabRow` / `FilterChip`): **`Todos` | `Camisas` | `Pantalones` | `Vestidos` | `Bolsos` | `Otros`**.
   - Al elegir **"Otros"** en el formulario de pedido (`garmentType = "otro"`), se despliega el campo libre *"Tipo de trabajo o artículo"* (`customType`: ej. llavero, compostura de cierre, dobladillo, cojín) y una tabla dinámica de medidas en `cm` donde el sastre puede usar medidas libres o plantillas rápidas.
4. **Compresión Nativa de Imágenes + Caché en Disco LRU (`150 MB`):**
   - Uso directo de `android.graphics.BitmapFactory` con `inSampleSize` + `Bitmap.createScaledBitmap` (máx. `1280px`) + `Bitmap.compress(Bitmap.CompressFormat.WEBP_LOSSY, 75, outStream)` (o `WEBP` en API < 30) y lectura de rotación con `androidx.exifinterface`.
   - Gestor `ImageDiskCache` sobre `context.cacheDir.resolve("order_images")` con límite estricto de **`150 MB`** y purga LRU al `85 %` (`127.5 MB`) de los archivos más antiguos no pendientes de subida.
5. **Generación Nativa de Fichas Imprimibles / WhatsApp en PDF:**
   - Uso de la API del sistema `android.graphics.pdf.PdfDocument` para dibujar en `Canvas` la **Ficha de Taller** (datos del cliente, prenda, tabla de medidas en cm, corte, insumos, saldo pendiente y notas de bitácora) en tamaño A4 o Ticket.
   - Compartición inmediata hacia WhatsApp o impresora Wi-Fi mediante `FileProvider` + `Intent.ACTION_SEND` (`application/pdf`). Peso añadido al APK: **0 KB**.
6. **Exportación Contable a `.xlsx` Bajo Demanda (3 Pestañas con Impuesto Configurable):**
   - **Prohibido Apache POI** (añade > 15 MB y provoca `OutOfMemoryError` en móviles de gama baja).
   - **Solución Ultraligera:** Generador OpenXML `.xlsx` nativo en Kotlin basado en `java.util.zip.ZipOutputStream` + escritor XML streaming (`XlsxStreamWriter` en Kotlin puro sin dependencias externas, **0 KB de sobrecarga**).
   - **Estructura exacta del libro `.xlsx` exportado:**
     - **Pestaña 1 — `Resumen_Mes` (Hacienda / Contabilidad):**
       - Periodo consultado y moneda del taller.
       - Total Ingresos Brutos (pedidos del periodo), Total Anticipos Cobrados y Saldo Pendiente por Cobrar.
       - **Desglose Fiscal Dinámico (según `taxRatePercent` y `taxLabel` en Ajustes):** Si `taxRatePercent > 0` (ej. `16 % IVA`), calcula y desglosa automáticamente **Subtotal Base Gravable**, **Impuesto Trasladado (`taxLabel` `%`)** y **Total Neto**. Si `taxRatePercent == 0`, muestra la fila de impuesto en `0.00 (Exento / No aplica)` sin forzar una tasa fija.
       - Costo Total de Insumos Consumidos, Margen Bruto Operativo y desglose mensual por categoría (`Camisas`, `Pantalones`, `Vestidos`, `Bolsos`, `Otros`).
     - **Pestaña 2 — `Detalle_Pedidos`:** Folio/ID, Fecha Creación, Fecha Compromiso, Fecha Entrega, Cliente, Teléfono, Categoría, Subtipo (`Otros`), Prenda/Trabajo, Estado, Costo Total, Impuesto Estimado, Anticipo, Saldo Pendiente.
     - **Pestaña 3 — `Movimientos_Insumos`:** Fecha/Hora, Insumo, Categoría, Unidad, Tipo de Movimiento (`Entrada`, `Salida`, `Devolución`, `Ajuste`), Cantidad ($\Delta$), Costo Unitario, Impacto Monetario, Motivo / Pedido vinculado (`orderId` + `revision`).
7. **Presupuesto de Peso del APK / AAB:**
   - Con R8 (`isMinifyEnabled = true`, `isShrinkResources = true`), el APK final por arquitectura (ABI split) se mantiene en **`<= 8 MB`**.

---

### 3.2. Escritorio Windows y macOS (Rust / Tauri 2) + Empaquetado Microsoft Store (MSIX)

Decisión de arquitectura confirmada: **Tanto Windows como macOS compartirán el mismo proyecto base Rust + Tauri 2**, logrando paridad funcional y visual del 100 % y binarios nativos ultraligeros (`~6 MB` en macOS `.app/.dmg` Universal Binary y `< 12 MB` en Windows `.exe`/MSIX).

1. **Encapsulamiento Multiplataforma de `WEBVIEW2_USER_DATA_FOLDER`:**
   - En `src-tauri/src/main.rs` (líneas 59–63), la configuración exclusiva de WebView2 se aísla con compilación condicional para que la compilación en macOS (`aarch64-apple-darwin` y `x86_64-apple-darwin`) sea 100 % limpia:
   ```rust
   #[cfg(target_os = "windows")]
   {
       let webview_dir = store.dir().join("webview");
       let _ = std::fs::create_dir_all(&webview_dir);
       if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
           std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_dir);
       }
   }
   ```
2. **Custodia Segura de Tokens en el Sistema Operativo con el Crate `keyring` (ISO/IEC 27002 Control 5.17):**
   - Se añade `keyring = "3"` en `src-tauri/Cargo.toml`.
   - Se implementan los comandos Tauri `save_secure_session_token`, `get_secure_session_token` y `clear_secure_session_token` bajo el servicio `"JLRDev.TallerCostura"`.
   - En **Windows**, `keyring` cifra y almacena la credencial directamente en el **Administrador de Credenciales de Windows (*Windows Credential Manager* / WinCred DPAPI)**.
   - En **macOS**, `keyring` almacena la credencial en el **Llavero de macOS (*macOS Keychain*)**.
   - El frontend WebView no persiste tokens de refresco en `localStorage` ni deja sesiones desprotegidas en texto plano.
3. **Sincronización Híbrida Firebase + Espejo Local `costura.json` + Caché LRU de Imágenes (`150 MB`):**
   - El frontend de Tauri (`src/js/`) incorpora el SDK modular ligero de Firebase (`firebase/app`, `firebase/auth`, `firebase/firestore`, `firebase/storage` empaquetados localmente en `src/js/vendor/firebase/` para cumplir la política CSP sin depender de CDNs externos).
   - Cada vez que Firestore emite un snapshot actualizado o se realiza una mutación local, el frontend sincroniza con el backend Rust mediante `sync_mirror_state(data: AppData)`, que actualiza `costura.json` y mantiene activo el sistema de respaldos automáticos diarios (`ensure_daily_backup`) en el disco local.
   - El módulo Rust de almacenamiento gestiona la carpeta local `cache/order_images` aplicando la misma política de **límite máximo de `150 MB` con expulsión LRU al `85 %` (`127.5 MB`)**.
4. **Exportación Contable `.xlsx` en Rust con `rust_xlsxwriter`:**
   - Se añade `rust_xlsxwriter = "0.79"` en `src-tauri/Cargo.toml`.
   - Nuevo comando Tauri `#[tauri::command] pub fn export_accounting_xlsx(state: Db<'_>, year: i32, month: u32, path: String) -> Result<String, String>` que genera el archivo `.xlsx` con las **3 pestañas** (`Resumen_Mes` con el cálculo opcional de `taxRatePercent`, `Detalle_Pedidos`, `Movimientos_Insumos`).
5. **Actualización de `csp` en `src-tauri/tauri.conf.json`:**
   - Permite conexión a Firebase Cloud y a puertos locales del Firebase Emulator Suite durante desarrollo:
   ```json
   "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: asset: http://asset.localhost https://firebasestorage.googleapis.com http://127.0.0.1:9199 http://localhost:9199; connect-src 'self' https://*.googleapis.com https://*.firebaseio.com https://*.firebaseapp.com wss://*.firebaseio.com http://127.0.0.1:8080 http://127.0.0.1:9099 http://127.0.0.1:9199 ws://127.0.0.1:8080"
   ```
6. **Conservación Intacta del Empaquetado MSIX y `runFullTrust` (Windows) + Bundle `.app/.dmg` (macOS):**
   - En Windows se mantiene intacto `src-tauri/msix/AppxManifest.xml` con `Identity Name="JLRDev.TallerCostura"`, `Publisher="CN=1D206152-7DEC-4E64-832B-010FB525F11D"`, `EntryPoint="Windows.FullTrustApplication"` y `<rescap:Capability Name="runFullTrust" />`, justificando `runFullTrust` ante Microsoft Store por la custodia en Windows Credential Manager y la escritura de respaldos soberanos (`costura.json` y reportes `.xlsx`).
   - En macOS se habilita el target `"app"` y `"dmg"` en `tauri.conf.json` con firma y notarización estándar de Apple Developer.

---

### 3.3. Estrategia para iPhone / iPad (iOS 16+ Nativo en SwiftUI)

1. **Interfaz y Plataforma:** **Swift + SwiftUI nativo** aplicando exactamente los tokens hexadecimales de la Parte 2 (`Color(hex:)` en `Asset Catalog` con variantes Light/Dark).
2. **Integración Firebase y Seguridad:**
   - `firebase-ios-sdk` vía Swift Package Manager (`FirebaseAuth`, `FirebaseFirestore`, `FirebaseStorage`) con persistencia SQLite offline activa y soporte para *Correo/Contraseña*, *Google Sign-In* y *Sign In with Apple*.
   - Tokens custodiados en *iOS Keychain* y caché de imágenes en `Library/Caches/order_images` acotada a **`150 MB` con expulsión LRU**.
3. **PDF y `.xlsx` en iOS:**
   - PDF: `UIGraphicsPDFRenderer` nativo de UIKit/PDFKit (0 KB extra).
   - `.xlsx`: Generador OpenXML nativo basado en ZIP/XML generando las 3 pestañas contables (incluyendo el desglose fiscal configurable `taxRatePercent`), compartido mediante `ShareLink` / `UIActivityViewController`.

---

## Parte 4: Flujo de Trabajo, Orden de Ejecución y Puntos de Control (Checkpoints)

### 4.1. Hoja de Ruta Secuencial Actualizada (Android Primero tras Firebase y Design System)

```
[FASE 1] Infraestructura Firebase (Emulator-First) + Esquema v2 + Reglas ISO 27001/27017
   │     (firebase.json, firestore.rules, storage.rules, plantillas parametrizadas,
   │      resolución de conflictos offline e idempotencia de movimientos, tests de reglas)
   ▼
[FASE 2] Sistema de Diseño Humano y Sobrio (Especificación de Tokens Multiplataforma)
   │     (Definición formal de tokens Light/Dark sin "IA Slop", paleta neutra + morado
   │      exclusivo de acento lista para Jetpack Compose, CSS Tauri y SwiftUI)
   ▼
[FASE 3] MVP Android Nativo (Kotlin + Jetpack Compose + Offline + PDF + XLSX)  <-- PRIORIDAD 1 CLIENTE
   │     (Auth Email/Google, PersistentCache 100 MB, caché imágenes LRU 150 MB,
   │      pestaña "Otros", WriteBatch idempotente, PdfDocument, XLSX 3 hojas con impuesto)
   ▼
[FASE 4] Sincronización Escritorio Windows/macOS (Tauri 2 + keyring + XLSX + MSIX) y Cierre iOS
         (4A: Rediseño app.css sobrio, custodia de tokens en Windows Credential Manager /
          macOS Keychain con crate keyring, sync Firebase <-> costura.json, rust_xlsxwriter,
          guarda #[cfg(target_os = "windows")], MSIX y build macOS .app/.dmg.
          4B: Cliente iOS SwiftUI conectado al mismo backend Firebase)
```

---

### 4.2. Checklist de Validación y Auditoría por Fase (Quality Gates)

Ninguna fase se da por cerrada sin superar estas pruebas verificables:

#### Checkpoint Fase 1: Seguridad, Esquema Cloud y Emuladores
- [ ] **Entorno Parametrizado + Emulator Suite:** Configuración lista con `firebase.json`, plantillas `.env.example` / `google-services.template.json` y suite de emuladores locales operativa sin exponer secretos en Git.
- [ ] **Aislamiento Cross-Tenant (ISO 27017):** Prueba automatizada en Firebase Emulator Suite donde el `Usuario A` intenta leer, modificar o borrar `/talleres/{uid_B}/clientes/*`, `/pedidos/*` o archivos en Storage de `uid_B` -> **100 % de peticiones rechazadas (`PERMISSION_DENIED`)**.
- [ ] **Validación de Esquema en Servidor:** Intento de insertar un pedido con `estimatedCost: -50`, `garmentType: "invalido"`, `taxRatePercent: 150` o más de 4 imágenes -> **Rechazado por `firestore.rules`**.
- [ ] **Límite de Storage:** Intento de subir un archivo ejecutable o una imagen de `2 MB` sin comprimir -> **Rechazado por `storage.rules` (`> 400 KB`)**.
- [ ] **Inmutabilidad de Auditoría y Kárdex:** Intento de ejecutar `update` o `delete` sobre `/movimientos/{id}` o `/auditoria/{id}` -> **Rechazado**.

#### Checkpoint Fase 2: Sistema de Diseño Sobrio (Tokens y Contraste)
- [ ] **Auditoría Visual Anti-"IA Slop":** Cero degradados decorativos, cero fondos teñidos de violeta, bordes estructurales de `1px` y morado (`#5B21B6` en claro / `#A78BFA` en oscuro) restringido a acciones primarias y estados activos.
- [ ] **Contraste WCAG 2.1:** Relación de contraste verificada `>= 7:1` en textos principales y `>= 4.5:1` en botones primarios e indicadores de estado tanto en Modo Claro como en Modo Oscuro.

#### Checkpoint Fase 3: MVP Android Nativo (Kotlin / Jetpack Compose)
- [ ] **Autenticación y Persistencia Offline en Taller:** Inicio de sesión con Correo/Contraseña o Google Sign-In. En *Modo Avión*, registrar un cliente, crear un pedido en la pestaña **"Otros"** vinculando 2 insumos, adjuntar una foto comprimida y cambiar el estado en bitácora.
- [ ] **Convergencia Conmutativa e Idempotencia de Stock:** Editar dos veces el mismo pedido en *Modo Avión* cambiando la cantidad de un insumo (ej. de `2 m` a `3 m`) y verificar que solo se descuente la diferencia neta (`-1 m`) con ID de movimiento determinista `mov_{orderId}_r{revision}_{supplyId}` sin duplicar descuentos.
- [ ] **Compresión y Caché LRU de Imágenes (`150 MB`):** Foto de cámara reducida a `<= 1280px` y `<= 180 KB` en WebP; verificación unitaria del recolector LRU expulsando archivos antiguos al superar `150 MB` y respetando imágenes pendientes de subida.
- [ ] **Exportación PDF y `.xlsx` en Móvil:** Generación de ficha de pedido en PDF para WhatsApp y generación del archivo `.xlsx` de 3 pestañas (`Resumen_Mes` con cálculo de tasa impositiva configurable `taxRatePercent`, `Detalle_Pedidos`, `Movimientos_Insumos`) con 0 KB de librerías externas.
- [ ] **Peso del Paquete Android:** APK Release firmado y optimizado con R8 **`<= 8 MB`**.

#### Checkpoint Fase 4: Escritorio Windows/macOS (Tauri 2 + MSIX) e iOS
- [ ] **Custodia en Windows Credential Manager / macOS Keychain (ISO 27002 5.17):** Verificación de que el token de sesión se guarda y elimina en el Administrador de Credenciales de Windows (y Llavero de macOS) mediante el crate `keyring` en Rust.
- [ ] **Sincronización y Migración `costura.json`:** Migración limpia de datos locales existentes en `costura.json` hacia `/talleres/{uid}/...`, sincronización en tiempo real con el MVP de Android y exportación `.xlsx` en Rust (`rust_xlsxwriter`) idéntica a la de Android.
- [ ] **Empaquetado Multiplataforma:** `cargo check` y `cargo build --release` sin advertencias; guarda `#[cfg(target_os = "windows")]` activa; paquete Windows **MSIX** (`AppxManifest.xml` + `runFullTrust`) íntegro; paquete macOS `.app/.dmg` universal funcional; cliente iOS SwiftUI sincronizado.
- [ ] **Borrado Seguro de Cuenta (Derecho de Supresión ISO 27017):** Purga completa de documentos en Firestore, fotos en Storage, caché LRU en disco, entrada en `keyring`/Keystore y cuenta en Firebase Auth.

---

### 4.3. Registro de Decisiones de Arquitectura Confirmadas

Las 5 decisiones clave han sido definidas y quedan integradas como requisitos vinculantes del proyecto:

| # | Punto de Decisión | Decisión Confirmada por el Usuario |
| :--- | :--- | :--- |
| **1** | **Entorno de Firebase** | Iniciar con **plantillas parametrizadas y Firebase Emulator Suite** en la Fase 1; preparado para conectar el `projectId` real de producción sin cambios de código. |
| **2** | **Métodos de Autenticación** | Habilitar **Correo/Contraseña + Google Sign-In** desde el inicio (e incorporar *Sign In with Apple* al abordar el cliente de iOS). |
| **3** | **Estrategia para macOS** | Compilar el mismo proyecto **Tauri 2 (Rust)** como binario `.app/.dmg` universal para mantener paridad 100 % con Windows y peso ultraligero (~6 MB). |
| **4** | **Orden de Implementación** | **Android Primero:** Tras Fase 1 (Firebase/Seguridad) y Fase 2 (Tokens del Design System), construir primero el **MVP de Android en Kotlin + Jetpack Compose** (Fase 3) y después actualizar el cliente de escritorio Tauri en Windows/macOS e iOS (Fase 4). |
| **5** | **Impuestos en Reporte `.xlsx`** | Incluir un campo configurable opcional de tasa impositiva (`taxRatePercent`, por defecto `0 %`, y etiqueta `taxLabel`, ej. `"IVA"`) en Ajustes para que la pestaña `Resumen_Mes` desglose impuestos cuando aplique sin forzar una tasa fija. |
