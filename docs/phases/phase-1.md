
## Decisiones de diseño clave (inspiradas en SQLite)

| Concepto | Recomendación para Fase 1 |
|----------|---------------------------|
| **Tamaño de página** | 4096 bytes (fijo). Más tarde se puede hacer configurable. |
| **Numeración** | Páginas desde **0**. SQLite usa 1. |
| **Slotted page** | Header fijo → Slot directory (crece hacia abajo) → Free space → Records (crecen hacia arriba). |
| **Endianness** | Big-endian (estilo SQLite). Porqué? Big-endian guarda el significant bit primero haciendo la comparación y sorting eficiente y preservando el orden sin hacer falta conversión (revertir el orden). |
| **Header de página** | Al menos: `page_id`, `num_slots`, `free_space_start` / `free_space_end`, `checksum` opcional. |
| **Row format** | Longitud-prefijada + null bitmap + valores. Suficiente para INTEGER / VARCHAR / BOOLEAN / NULL. |
| **Catalog** | Tablas especiales (`_tables`, `_columns`) almacenadas también como heap files. |

### Layout típico de una slotted page (4 KB)

```
+------------------+  offset 0
| Page Header      |  ~16-32 bytes (magic, page_id, num_slots, free_start, free_end, ...)
+------------------+
| Slot Directory   |  array de u16 offsets (crece hacia la derecha / abajo)
| slot[0]          |
| slot[1]          |
| ...              |
+------------------+  ← free_space_start
|                  |
|   Free Space     |
|                  |
+------------------+  ← free_space_end
| Record N         |  (crecen hacia la izquierda / arriba)
| Record N-1       |
| ...              |
| Record 0         |
+------------------+  offset 4096
```

## Orden de implementación (de cero a “algo que funciona”)

Sigue este orden; cada paso tiene tests y te da feedback inmediato:

### Paso 1 – Page de tamaño fijo + I/O
- `Page` = `[u8; 4096]` o `Box<[u8; PAGE_SIZE]>`.
- `PageId` = `u32` o `u64`.
- Métodos: `new()`, `read_from(file, page_id)`, `write_to(file, page_id)`.
- Test: escribir página llena de un patrón, cerrar, reabrir y verificar.

### Paso 2 – Slotted Page
- Header + slot directory.
- `insert_record(&[u8]) -> SlotId`
- `get_record(slot_id) -> &[u8]`
- `delete_record` (opcional al principio, o solo marcar).
- Test: insertar varios records de tamaños distintos, leerlos, persistir la página.

### Paso 3 – Heap File
- Un archivo = colección de páginas.
- `allocate_page()`, `get_page(page_id)`, `write_page`.
- Free list simple (o solo “siguiente página libre” al final del archivo).
- Test: crear heap, insertar varias páginas, reabrir el archivo y leer.

### Paso 4 – Row Format (serialización)
- Tipos: `Integer(i64)`, `Varchar(String)`, `Boolean(bool)`, `Null`.
- `serialize(&[Value]) -> Vec<u8>`
- `deserialize(&[u8]) -> Vec<Value>`
- Null bitmap al inicio.
- Test: round-trip de varias filas.

### Paso 5 – Catalog
- Tablas de sistema:
  - `_tables` (table_id, name, root_page, …)
  - `_columns` (table_id, column_id, name, type, nullable, …)
- Al crear una tabla de usuario, insertas filas en el catálogo.
- Al arrancar, lees el catálogo para saber qué tablas existen.
- Test: `CREATE TABLE` (solo a nivel de storage) y luego consultar el esquema.

---

## Qué se ha implementado en esta fase

Resumen de la sesión: el motor de almacenamiento (Fase 1) está **completo y
probado**. Aunque todavía no existe un binario ejecutable (el `main.rs` sigue
siendo un "Hello, world!" y el SQL es una fase posterior), los tres criterios de
aceptación de la fase se cumplen y quedan garantizados por tests:

- Las páginas se escriben y se vuelven a leer de disco de forma persistente.
- Se pueden insertar y recuperar tuplas de longitud variable.
- El catálogo permite crear y consultar esquemas de tablas.

### Módulos implementados (en `src/storage/`)

| Módulo | Responsabilidad | API pública principal |
|--------|-----------------|-----------------------|
| `page.rs` | Página de tamaño fijo (4096 B) e I/O a disco | `PAGE_SIZE`, `PageId`, `Page::new/read_from/write_to` |
| `slotted_page.rs` | Layout de slotted page (header + slot directory) | `SlottedPage::new/from_page/insert_record/get_record/delete_record`, `free_space`, `is_deleted` |
| `heap_file.rs` | Colección de páginas en un único archivo | `HeapFile::create/open/allocate_page/read_page/write_page/delete_page` |
| `row.rs` | Serialización de filas (length-prefix + null bitmap) | `Value::{Integer,Varchar,Boolean,Null}`, `serialize`, `deserialize` |
| `catalog.rs` | Tablas de sistema `_tables` / `_columns` como heap files | `Catalog::create/open`, `create_table`, `get_table`, `get_all_tables`, `ColumnDef`/`ColumnSchema`/`TableSchema` |

### Decisiones de formato concretas

- **Header de slotted page (16 bytes):** magic `0xBA5A5D70`, `page_id`,
  `num_slots`, `free_start`, `free_end`. Todos los enteros multi-byte en
  **big-endian**.
- **Slot:** un `u16` con el offset del record; el record lleva prefijo de
  longitud `u16`. Borrar marca el slot como `0` (sin compactación todavía,
  simplificación documentada).
- **Row format:** `[u32 total_len][u16 num_values][null bitmap][valores…]`,
  autocontenido (no necesita el esquema para deserializar).
- **Heap file:** el recuento de páginas se deduce del tamaño del archivo al
  reabrir; los borrados se devuelven a una *free list* en memoria (los pages
  liberados reaparecen como páginas a cero tras un reinicio, lo cual es seguro).

### Manejo de errores

`error.rs` define el enum `Error` con variantes descriptivas
(`Io`, `Storage`, `PageFull`, `InvalidSlot`, `InvalidPageId`, `Corrupt`,
`Deserialize`, `NotFound`, `Duplicate`). Ninguna situación recuperable lanza
panics; todas las operaciones devuelven `Result<T, Error>`.

### Tests (todos en verde)

- **13 tests unitarios** dentro de los módulos.
- **26 tests de integración** en `tests/storage_tests.rs`, organizados por paso
  (`page_tests`, `slotted_page_tests`, `heap_file_tests`, `row_tests`,
  `catalog_tests`).
- **1 doctest** en `lib.rs`.
- `cargo clippy --all-targets` sin warnings.

### Cambios de toolchain y configuración

- `Cargo.toml`: `edition = "2024"` (el `2026` no lo soporta cargo 1.88) y
  dev-dependency `tempfile` para los tests.
- `rust-toolchain.toml`: fija el toolchain GNU (`stable-x86_64-pc-windows-gnu`)
  porque la máquina no tiene instalados los MSVC Build Tools (`link.exe`).
- `docs/instructions.md`: guía de instalación y comandos para colaboradores.