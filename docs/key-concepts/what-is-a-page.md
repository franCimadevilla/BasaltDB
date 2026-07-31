# ¿Qué es una Page?

Una **Page** (también llamada *disk page* o *database page*) es la **unidad mínima de almacenamiento que el motor lee y escribe en disco**.

No se almacenan filas individuales en disco.

No se almacenan tablas completas.

Se almacenan **páginas**.

Puedes imaginar el disco como un enorme array de páginas.

```
Disco

+---------+
| Page 0  |
+---------+
| Page 1  |
+---------+
| Page 2  |
+---------+
| Page 3  |
+---------+
|   ...   |
+---------+
```

Cada página tiene un tamaño fijo.

En casi todas las bases de datos modernas:

* 4 KB
* 8 KB
* 16 KB

PostgreSQL usa 8 KB.

SQLite usa normalmente 4 KB.

---

# ¿Por qué existen las páginas?

Porque el hardware funciona así.

Un SSD o un HDD no puede leer 43 bytes.

Lee bloques.

Por ejemplo:

Quieres leer una fila de 120 bytes.

El SSD realmente hará algo parecido a:

```
leer(Page 183)
```

y traerá los 8192 bytes completos.

Nunca solamente esos 120 bytes.

Por eso las bases de datos organizan todo en páginas.

---

# Una página es un bloque de memoria

Supongamos una página de 4096 bytes.

```
4096 bytes

+------------------------------------------------------+
|                                                      |
|                                                      |
|                                                      |
|                                                      |
+------------------------------------------------------+
```

Nada más.

Simplemente un array de bytes.

En Rust podría representarse como

```rust
struct Page {
    data: [u8; 4096]
}
```

Eso ya es una página.

No sabe nada de SQL.

No sabe nada de tablas.

No sabe nada de columnas.

Sólo contiene bytes.

---

# Una página es como una hoja de un libro

Imagina un libro.

No arrancas media hoja.

Lees una hoja completa.

La página de base de datos funciona exactamente igual.

```
Libro

Página 1
Página 2
Página 3
Página 4
```

Cuando necesitas algo de la página 3, lees la página entera.

---

# ¿Qué contiene una página?

Depende.

Una página puede contener:

* registros (tuplas)
* índices
* metadatos
* overflow
* información interna

En la Fase 1 únicamente contendrá **tuplas**.

---

# Una tabla no es un archivo de texto

Imagina esta tabla:

| id | nombre |
| -- | ------ |
| 1  | Ana    |
| 2  | Luis   |
| 3  | Pedro  |

No se almacena así:

```
1,Ana
2,Luis
3,Pedro
```

Se almacena como varias páginas.

```
Archivo customers.tbl

+-----------+
| Page 0    |
+-----------+
| Page 1    |
+-----------+
| Page 2    |
+-----------+
```

Y dentro de cada página viven varias filas.

```
Page 0

+--------------------------------+
| fila 1                         |
| fila 2                         |
| fila 3                         |
| fila 4                         |
| fila 5                         |
+--------------------------------+
```

Cuando la página se llena...

...se crea otra.

---

# ¿Qué es un Heap File?

Un Heap File no es más que una colección de páginas.

```
Heap File

+---------+
| Page 0  |
+---------+
| Page 1  |
+---------+
| Page 2  |
+---------+
| Page 3  |
+---------+
```

Por eso en tu roadmap aparece

```
Heap File
    ├── Page 0
    ├── Page 1
    ├── Page 2
```

---

# ¿Cómo encuentra una fila?

No por su posición física.

Normalmente mediante

```
(PageID, SlotID)
```

Ejemplo

```
Page = 17

Slot = 4
```

Eso significa

```
Ve a la página 17.

Dentro de ella busca el slot 4.
```

Esta idea será muy importante cuando implementes las *slotted pages*.

---

# ¿Qué operaciones básicas tiene una página?

Una página normalmente sabe hacer cosas como

```
insert(record)

delete(record)

update(record)

read(record)
```

Internamente modifica únicamente su array de bytes.

---

# ¿Por qué no usamos `Vec<Record>`?

Porque una base de datos debe controlar absolutamente todo.

Un `Vec<Record>`:

* mueve memoria automáticamente
* cambia de tamaño
* depende del allocator de Rust
* no coincide con el formato en disco

La base de datos necesita exactamente esto:

```
Byte 0

Byte 1

Byte 2

...

Byte 4095
```

Cada byte tiene un significado.

---

# ¿Qué relación tiene con la RAM?

Cuando haces

```sql
SELECT *
FROM users;
```

la base de datos hace algo parecido a

```
Disco
    ↓

leer página 42

    ↓

RAM

Page {
    data: [u8;4096]
}
```

Trabaja sobre la copia en memoria.

Cuando termina:

```
RAM

↓

escribir página

↓

Disco
```

Nunca modifica directamente el disco.

---

# ¿Por qué el tamaño es fijo?

Porque facilita muchísimo el sistema.

Si todas las páginas ocupan 4096 bytes:

La página 0 empieza en

```
0
```

La página 1 en

```
4096
```

La página 2 en

```
8192
```

La página 3 en

```
12288
```

Calcular la posición es trivial:

```
offset = page_id × PAGE_SIZE
```

No hace falta recorrer el archivo buscando dónde empieza cada una.

---

# ¿Cómo se relaciona con el resto de la Fase 1?

Ahora puedes entender el roadmap casi como una cadena de responsabilidades:

```
Storage Engine

           Heap File
                │
                ▼
        +----------------+
        |    Page 0      |
        +----------------+
        |    Page 1      |
        +----------------+
        |    Page 2      |
        +----------------+

Cada página contiene

        ▼

      Tuplas

Cada tupla debe

        ▼

serializarse a bytes

Los bytes se colocan

        ▼

dentro de la página

Cuando la página se llena

        ▼

Page Allocation crea otra

Dentro de la página

        ▼

Slotted Page organiza el espacio libre
```

## La idea clave de toda la fase

Si tuviera que resumir la Fase 1 en una sola frase, sería esta:

> **Implementar un sistema capaz de almacenar registros arbitrarios en un archivo de disco dividido en páginas de tamaño fijo, gestionando eficientemente el espacio libre y permitiendo localizar cada registro mediante una estructura interna estable.**

Todo lo demás —el `PageManager`, el `HeapFile`, la serialización de tuplas, la asignación de páginas y las *slotted pages*— son componentes que cooperan para hacer posible esa única idea. Una vez comprendas cómo una página almacena bytes y cómo esos bytes representan tuplas, el resto del diseño del motor de almacenamiento resulta mucho más natural.
