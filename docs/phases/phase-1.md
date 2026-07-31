
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