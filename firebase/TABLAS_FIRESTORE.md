# ESQUEMA DE COLECCIONES (TABLAS) PARA FIREBASE — TALLER COSTURA v2.0

Este documento contiene la guía paso a paso y la definición exacta de las **colecciones (tablas), documentos, campos, tipos de datos e índices** que debes configurar en **Firebase Console** (o que se crearán automáticamente al subir el primer lote desde la aplicación), así como los espacios (`placeholders`) donde colocarás tus credenciales para conectar la base de datos.

---

## 1. Qué Activar en tu Consola de Firebase (`console.firebase.google.com`)

1. **Crear proyecto:** Nombre sugerido `taller-costura` (copia el **`Project ID`** que te asigne Firebase).
2. **Authentication (`Build -> Authentication`):**
   - Pestaña **Sign-in method**:
     - Habilitar **Correo electrónico/contraseña** (*Email/Password*).
     - Habilitar **Google** (copia el **ID de cliente web / Web client ID** que termina en `.apps.googleusercontent.com`).
3. **Cloud Firestore (`Build -> Firestore Database`):**
   - Clic en **Crear base de datos** (*Create database*) -> Modo **Producción** -> Ubicación cercana (ej. `us-central1` o `southamerica-east1`).
   - En la pestaña **Reglas (*Rules*)**, pega el contenido íntegro de `firebase/firestore.rules`.
4. **Cloud Storage (`Build -> Storage`):**
   - Inicializar Storage y en la pestaña **Reglas (*Rules*)** pegar el contenido de `firebase/storage.rules`.

---

## 2. Estructura de Tablas (Colecciones y Subcolecciones en Cloud Firestore)

En Cloud Firestore, las "tablas" son **Colecciones** y las "filas" son **Documentos**.  
Toda la base de datos está aislada por el identificador de usuario (`{uid}`) bajo la colección raíz **`talleres`**.

```
/talleres/{uid}                          <-- Tabla Raíz (1 documento por taller/usuario)
   ├── /clientes/{clienteId}             <-- Subtabla 1: Clientes y saldos
   ├── /pedidos/{pedidoId}               <-- Subtabla 2: Pedidos (Camisa, Pantalón, Vestido, Bolso, Otros)
   ├── /insumos/{insumoId}               <-- Subtabla 3: Inventario de insumos
   ├── /movimientos/{movimientoId}       <-- Subtabla 4: Kárdex inmutable de entradas/salidas
   ├── /agenda/{recordatorioId}          <-- Subtabla 5: Recordatorios y citas
   └── /auditoria/{eventoId}             <-- Subtabla 6: Bitácora de seguridad ISO 27002
```

---

### Tabla Raíz: `talleres`
- **Ruta:** `/talleres/{uid}`
- **ID del Documento:** `{uid}` (el UID que asigna Firebase Auth al usuario).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `version` | `number` (int) | `2` | Sí | Versión del esquema de datos |
| `workshopName` | `string` | `"Mi taller de costura"` | Sí | Nombre del taller (`1..120` caracteres) |
| `ownerName` | `string` | `"Elena"` | Sí | Nombre del propietario (`0..120` caracteres) |
| `currency` | `string` | `"$"` | Sí | Símbolo o código de moneda (`"$"`, `"MXN"`, `"USD"`, `"EUR"`) |
| `taxRatePercent`| `number` (float) | `0` | Sí | Porcentaje de impuesto opcional para el Excel (`0` a `100`, ej. `16`) |
| `taxLabel` | `string` | `"IVA"` | Sí | Etiqueta del impuesto para reportes (`"IVA"`, `"IGV"`, etc.) |
| `defaultDueDays`| `number` (int) | `7` | Sí | Días de entrega por defecto (`1..365`) |
| `language` | `string` | `"es"` | Sí | Idioma (`"es"`, `"en"`, `"pt"`) |
| `updatedAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha/hora ISO-8601 de última actualización |

---

### Subtabla 1: `clientes`
- **Ruta:** `/talleres/{uid}/clientes/{clienteId}`
- **ID del Documento:** `{clienteId}` (UUID v4).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"7c9e6679-7425-40de-944b-e07fc1f90ae7"` | Sí | UUID del cliente (igual al ID del documento) |
| `fullName` | `string` | `"María José Rivera"` | Sí | Nombre completo (`1..120` caracteres) |
| `phone` | `string` | `"+503 7788-1122"` | Sí | Teléfono / WhatsApp (`0..40` caracteres) |
| `email` | `string` | `"maria@example.com"` | Sí | Correo electrónico (`0..160` caracteres, puede ir `""`) |
| `address` | `string` | `"Col. Escalón"` | Sí | Dirección física (`0..300` caracteres, puede ir `""`) |
| `notes` | `string` | `"Prefiere telas naturales"` | Sí | Notas y preferencias (`0..2000` caracteres) |
| `pendingBalance`| `number` (float) | `25.0` | Sí | Saldo total pendiente por cobrar (`>= 0`, vía `FieldValue.increment`) |
| `activeOrdersCount` | `number` (int) | `1` | Sí | Cantidad de pedidos activos (`!= "entregado"`) |
| `createdAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de registro ISO-8601 |
| `updatedAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de última modificación ISO-8601 |

---

### Subtabla 2: `pedidos`
- **Ruta:** `/talleres/{uid}/pedidos/{pedidoId}`
- **ID del Documento:** `{pedidoId}` (UUID v4).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"b1a2c3d4-..."` | Sí | UUID del pedido |
| `customerId` | `string` | `"7c9e6679-..."` | Sí | ID del cliente dueño del pedido |
| `pieceName` | `string` | `"Arreglo de cierre y llavero"` | Sí | Nombre de la prenda o proyecto (`1..160` caracteres) |
| `garmentType` | `string` | `"otro"` | Sí | Categoría: `"camisa"`, `"pantalon"`, `"vestido"`, `"bolso"`, **`"otro"`** |
| `customType` | `string` | `"Llavero / Compostura"` | Sí | Subtipo libre cuando `garmentType` es `"otro"` (`0..80` caracteres) |
| `styleName` | `string` | `"Personalizado"` | Sí | Corte o estilo (`0..80` caracteres) |
| `status` | `string` | `"en_confeccion"` | Sí | Estado: `"pendiente"`, `"en_corte"`, `"en_confeccion"`, `"prueba_ajuste"`, `"terminado"`, `"entregado"` |
| `priority` | `string` | `"normal"` | Sí | Prioridad: `"normal"`, `"urgente"`, `"prioritaria"` |
| `measurements` | `map` | `{ "kind": "otro", "values": {"largo": 12.5}, "options": {}, "notes": "" }` | Sí | Mapa con medidas en **cm** (`values`), opciones (`options`) y notas |
| `fabricDetails` | `string` | `"Cuero sintético negro"` | Sí | Detalles de tela y materiales (`0..1500` caracteres) |
| `estimatedCost` | `number` (float) | `45.0` | Sí | Costo total acordado (`>= 0`) |
| `depositPaid` | `number` (float) | `20.0` | Sí | Anticipo pagado (`>= 0`) |
| `dueDate` | `string` | `"2026-11-26"` | Sí | Fecha de entrega prometida (`YYYY-MM-DD`) |
| `deliveryDate` | `string` o `null` | `null` | No | Fecha real en que se entregó (`YYYY-MM-DD` o `null`) |
| `revision` | `number` (int) | `1` | Sí | Contador entero de versión del pedido (`>= 1`) para idempotencia de insumos |
| `notes` | `array` de `map` | `[{ "id": "...", "timestamp": "...", "stage": "recepcion", "content": "..." }]` | Sí | Historial de notas de la bitácora del pedido |
| `supplies` | `array` de `map` | `[{ "supplyId": "s1", "quantity": 1.5, "cut": "al_hilo", "note": "" }]` | Sí | Insumos vinculados al pedido |
| `images` | `array` de `map` | `[{ "id": "img1", "storagePath": "/talleres/{uid}/pedidos/{orderId}/img1.webp", "downloadUrl": "...", "width": 1024, "height": 768, "sizeBytes": 124000 }]` | Sí | Máximo **4** imágenes comprimidas en Firebase Storage |
| `createdAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de creación ISO-8601 |
| `updatedAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de última edición ISO-8601 |

---

### Subtabla 3: `insumos`
- **Ruta:** `/talleres/{uid}/insumos/{insumoId}`
- **ID del Documento:** `{insumoId}` (UUID v4).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"s1"` | Sí | UUID del insumo |
| `name` | `string` | `"Tela de lino italiano"` | Sí | Nombre del insumo (`1..120` caracteres) |
| `category` | `string` | `"tela"` | Sí | Categoría: `"tela"`, `"hilo"`, `"boton"`, `"cierre"`, `"elastico"`, `"entretela"`, `"otro"` |
| `unit` | `string` | `"metro"` | Sí | Unidad: `"yarda"`, `"metro"`, `"unidad"`, `"cono"`, `"rollo"`, `"par"`, `"otro"` |
| `stock` | `number` (float) | `15.0` | Sí | Stock actual (actualizado con `FieldValue.increment`) |
| `minStock` | `number` (float) | `5.0` | Sí | Stock mínimo para alerta (`>= 0`) |
| `unitCost` | `number` (float) | `8.5` | Sí | Costo por unidad (`>= 0`) |
| `supplier` | `string` | `"Mercería Central"` | Sí | Nombre del proveedor (`0..160` caracteres) |
| `notes` | `string` | `"Ancho 1.50m, color marfil"` | Sí | Notas del material (`0..1000` caracteres) |
| `createdAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de alta ISO-8601 |
| `updatedAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de última modificación ISO-8601 |

---

### Subtabla 4: `movimientos` (Kárdex Inmutable de Insumos)
- **Ruta:** `/talleres/{uid}/movimientos/{movimientoId}`
- **ID del Documento:**
  - Si proviene de un pedido: `mov_{orderId}_r{revision}_{supplyId}` (determinista para evitar descuentos duplicados).
  - Si es una entrada o ajuste manual: UUID v4.

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"mov_ord1_r1_s1"` | Sí | ID del movimiento |
| `supplyId` | `string` | `"s1"` | Sí | ID del insumo afectado |
| `timestamp` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha y hora exacta del movimiento |
| `delta` | `number` (float) | `-2.5` | Sí | Variación aplicada (`-` salida, `+` entrada o devolución) |
| `kind` | `string` | `"salida"` | Sí | Tipo: `"entrada"`, `"salida"`, `"devolucion"`, `"ajuste"` |
| `reason` | `string` | `"Pedido creado: Camisa de lino"` | Sí | Motivo legible para la hoja 3 del Excel contable |
| `orderId` | `string` o `null` | `"ord1"` | No | Pedido vinculado (o `null` si fue compra/ajuste manual) |
| `orderRevision` | `number` o `null` | `1` | No | Revisión del pedido que originó el descuento |

---

### Subtabla 5: `agenda`
- **Ruta:** `/talleres/{uid}/agenda/{recordatorioId}`
- **ID del Documento:** `{recordatorioId}` (UUID v4).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"rem-01"` | Sí | UUID del recordatorio |
| `title` | `string` | `"Prueba de vestido con Ana"` | Sí | Título (`1..140` caracteres) |
| `description` | `string` | `"Confirmar ajuste de cintura"` | Sí | Notas (`0..1500` caracteres) |
| `dueDate` | `string` | `"2026-11-20"` | Sí | Fecha estricta solo día (`YYYY-MM-DD`, exactamente 10 caracteres, sin hora combinada) |
| `dueTime` | `string` o `null` | `"16:30"` | No | Hora opcional en formato 24h (`HH:mm`, ej. `"16:30"` o `null`) |
| `priority` | `string` | `"prioritaria"` | Sí | Prioridad: `"normal"`, `"urgente"`, `"prioritaria"` |
| `completed` | `boolean` | `false` | Sí | `true` si ya se completó, `false` si está pendiente |
| `customerId` | `string` o `null` | `"7c9e6679-..."` | No | Cliente asociado opcional |
| `orderId` | `string` o `null` | `"b1a2c3d4-..."` | No | Pedido asociado opcional |
| `createdAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de creación ISO-8601 |
| `updatedAt` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha de última actualización ISO-8601 |

---

### Subtabla 6: `auditoria` (Bitácora de Seguridad ISO/IEC 27002 8.15)
- **Ruta:** `/talleres/{uid}/auditoria/{eventoId}`
- **ID del Documento:** `{eventoId}` (UUID v4). Solo permite `create` y `read` (prohibido `update` y `delete`).

| Campo (Columna) | Tipo en Firestore | Valor de Ejemplo | Obligatorio | Descripción |
| :--- | :--- | :--- | :---: | :--- |
| `id` | `string` | `"aud-01"` | Sí | UUID del evento |
| `timestamp` | `string` | `"2026-11-19T12:00:00-06:00"` | Sí | Fecha y hora UTC/ISO-8601 |
| `action` | `string` | `"order.delete"` | Sí | Acción crítica (`customer.delete`, `order.delete`, `supply.adjust`, etc.) |
| `entityType` | `string` | `"pedidos"` | Sí | Colección afectada |
| `entityId` | `string` | `"b1a2c3d4-..."` | Sí | ID del registro afectado |
| `summary` | `string` | `"Pedido eliminado: Camisa"` | Sí | Resumen de auditoría |

---

## 3. Archivos con los Espacios Listos para tus Credenciales

Cuando tengas creado tu proyecto en Firebase Console, solo tendrás que pasarnos (o colocar en estos archivos) los valores reales:

1. **Web / Escritorio (Windows y macOS Tauri 2):**
   - `firebase/firebase-config.template.json` y `.env.example`
   - `src/js/firebase-bridge.js` (`FIREBASE_CONNECTION_SLOTS`)
   - `src-tauri/src/model.rs` (`FirebaseConnectionSlots`)
2. **Android (Kotlin / Jetpack Compose):**
   - `firebase/google-services.template.json` (se copiará como `android-app/app/google-services.json`).
3. **Reglas e Índices Listos para Desplegar:**
   - `firebase/firestore.rules`
   - `firebase/storage.rules`
   - `firebase/firestore.indexes.json`
