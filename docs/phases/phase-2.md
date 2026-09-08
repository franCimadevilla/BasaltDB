# Fase 2 — SQL Parser

## Decisiones de diseño clave

| Concepto | Decisión para Fase 2 |
|----------|----------------------|
| **Lexer** | Generado por LALRPOP a partir de la gramática (`src/parser/grammar.lalrpop`). Sin lexer manual; los tokens se definen con regex en la propia gramática. Añade `lalrpop` como dependencia (ver `Cargo.toml`). |
| **Parser** | Generado por LALRPOP (LR(1)) en lugar de recursive-descent manual o Pest. La gramática embebe el action code que construye directamente el AST. Más extensible para evolucionar el lenguaje SQL de BasaltDB. |
| **Posición de tokens** | Cada token guarda `line` y `column` para producir mensajes de error claros y localizables. |
| **AST independiente de storage** | `src/parser/ast/` define sus propios tipos (`DataType`, `Literal`, `Expr`, `Statement`). La conversión a `storage::Value` / `storage::ColumnType` se hará en la fase de planner. | 
| **Palabras clave** | Case-insensitive (`SELECT`, `select`, `SeLeCt` son equivalentes). |
| **Identificadores** | Se preservan tal como se escriben. La comparación con el catálogo será case-sensitive en esta fase (simplificación documentada). |
| **Tipos soportados** | `INTEGER`, `VARCHAR`, `BOOLEAN`, con opciones `NULL` / `NOT NULL` por columna. Espejan los `ColumnType` del catálogo (Fase 1). |
| **Punto y coma** | Terminador opcional al final de la sentencia. Una llamada al parser procesa **una sola** sentencia; dividir múltiples sentencias queda para la fase de REPL. |
| **Strings** | Delimitados por comillas simples `'...'`; el escape de una comilla se escribe duplicándola (`''`), estilo SQL. |
| **Predicados** | Comparaciones (`=`, `<`, `>`, `<=`, `>=`, `<>`) combinables con `AND` / `OR` / `NOT`. Sin aritmética en esta fase. |

### Decisión: por qué LALRPOP (2026-09-08)

Se abandona la implementación manual (lexer + recursive-descent a mano) en
favor de la dependencia `lalrpop = "0.23.1"`. Alternativas valoradas:

| Alternativa | Veredicto | Motivo |
|-------------|-----------|--------|
| **Implementación manual** | Descartada | Requiere cientos de líneas de código (`token.rs` + `lexer.rs` + `parser.rs`) y limita la extensibilidad del lenguaje SQL de BasaltDB: cada nueva cláusula obliga a tocar lexer, parser y tests a mano. |
| **Pest** | Descartada | Parser PEG más simple, capaz de validar hasta gramáticas de sintaxis, pero no incluye el árbol AST ni el action code embebido en la gramática. Obligaría a una segunda fase manual de construcción del AST a partir del parse tree. |
| **ANTLR4Rust** | Descartada | Equivalente Rust del framework ANTLR, pero el proyecto está abandonado por su autor; riesgo inaceptable de mantenibilidad de la dependencia. |
| **LALRPOP (elegida)** | Adoptada | Generador LR(1) mantenido y ampliamente usado en Rust. La gramática (`.lalrpop`) define tokens (regex), reglas y action code que construye `ast.rs` directamente. Añadir sintaxis SQL futura = extender la gramática, sin reescribir el parser. Compromiso consciente: se introduce una dependencia de build/codegen a cambio de extensibilidad y menos código manual. |

Implicaciones:

- `Cargo.toml` gana la dependencia `lalrpop` (y `lalrpop-util` / build-script según sea necesario).
- `src/parser/grammar.lalrpop` es la única fuente de verdad de la sintaxis; `token.rs` / `lexer.rs` / `parser.rs` manuales desaparecen o quedan como shims finos.
- Los errores de LALRPOP (`ParseError`) se mapean a `Error::Syntax` con `line`/`column` para mantener el formato de error definido abajo.
- Elimina el fichero experimental `src/parser/grammar.pest` si aún existe.

### Fuera de alcance (futuras extensiones)

- Aritmética en predicados (`+`, `-`, `*`, `/`).
- Identificadores entre comillas (`"col name"`).
- Subconsultas y joins.
- `GROUP BY`, `ORDER BY`, agregaciones.
- `VARCHAR(n)` con longitud.
- Comentarios en SQL (`--`, `/* */`).
- Múltiples sentencias separadas por `;` en una sola entrada.

Estas extensiones son triviales de añadir después porque con LALRPOP basta con
extender `grammar.lalrpop` (nuevos `match` + reglas + action code) sin
reescribir lexer/parser a mano.

---

## Gramática soportada

```
statement    := create_table | insert | select | update | delete

create_table := CREATE TABLE ident '(' column_def (',' column_def)* ')'
column_def   := ident type ('NOT NULL' | 'NULL')?
type         := INTEGER | VARCHAR | BOOLEAN

insert       := INSERT INTO ident ('(' ident (',' ident)* ')')? VALUES tuple (',' tuple)*
tuple        := '(' literal (',' literal)* ')'

select       := SELECT select_list FROM ident ('WHERE' expr)?
select_list  := '*' | expr (',' expr)*

update       := UPDATE ident SET assignment (',' assignment)* ('WHERE' expr)?
assignment   := ident '=' expr

delete       := DELETE FROM ident ('WHERE' expr)?

expr         := or_expr
or_expr      := and_expr ('OR' and_expr)*
and_expr     := not_expr ('AND' not_expr)*
not_expr     := 'NOT' not_expr | comparison
comparison   := operand (cmp_op operand)?
cmp_op       := '=' | '<' | '>' | '<=' | '>=' | '<>'
operand      := literal | ident | '(' expr ')'
literal      := integer | string | TRUE | FALSE | NULL
```

Precedencia de operadores (de menor a mayor): `OR` < `AND` < `NOT` < comparación.

---

## Estructura de módulos (actualizada a LALRPOP)

```
src/parser/
    mod.rs            # docs del módulo + re-exports (parse, Statement, Expr, ...)
                      # + mapeo ParseError -> Error::Syntax
    ast.rs            # DataType, Literal, Expr, Statement + sub-estructuras
    grammar.lalrpop   # única fuente de verdad: tokens (match), gramática y action code -> ast.rs
    build.rs (raíz)   # lalrpop::process_root() para generar el parser en build
```

### API pública

```rust
// src/parser/mod.rs
pub fn parse(sql: &str) -> Result<Statement>;
```

Los tipos del AST derivan `Debug`, `Clone`, `PartialEq` para poder
asertar igualdad en los tests.

---

## Tipos del AST

```rust
pub enum Statement {
    CreateTable(CreateTableStatement),
    Insert(InsertStatement),
    Select(SelectStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
}

pub struct CreateTableStatement {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,      // name, data_type, nullable
}

pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,               // true si "NULL" u omitido; false si "NOT NULL"
}

pub enum DataType { Integer, Varchar, Boolean }   // espeja storage::ColumnType

pub struct InsertStatement {
    pub table_name: String,
    pub columns: Option<Vec<String>>, // None => sin lista de columnas
    pub values: Vec<Vec<Literal>>,    // un Vec<Literal> por tupla
}

pub struct SelectStatement {
    pub columns: Vec<SelectItem>,     // '*' o expresiones
    pub table: String,
    pub predicate: Option<Expr>,
}

pub enum SelectItem { Wildcard, Expr(Expr) }

pub struct UpdateStatement {
    pub table: String,
    pub assignments: Vec<(String, Expr)>,
    pub predicate: Option<Expr>,
}

pub struct DeleteStatement {
    pub table: String,
    pub predicate: Option<Expr>,
}

pub enum Literal { Integer(i64), String(String), Boolean(bool), Null }

pub enum Expr {
    Literal(Literal),
    Column(String),
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    Not(Box<Expr>),
}

pub enum BinaryOp { Eq, Lt, Gt, Le, Ge, Ne, And, Or }
```

Ejemplo:

```sql
SELECT name FROM student WHERE age > 20
```

se convierte en una `SelectStatement { columns: [Expr(Column("name"))],
table: "student", predicate: Some(Binary { op: Gt, left: Column("age"),
right: Literal(Integer(20)) }) }`.

---

## Manejo de errores

Se añade **una** variante nueva a `src/error.rs`:

```rust
Error::Syntax(String)
```

Formato de mensaje:

```
Syntax error at line 1, column 14: expected ',' or ')', found ';'
```

- El tokenizer generado por LALRPOP reporta errores (p. ej. string sin cerrar,
  carácter inválido) como `Error::Syntax`, incluyendo posición.
- Los `ParseError` de LALRPOP se convierten en `Error::Syntax` con formato
  `Syntax error at line 1, column 14: ...`, preservando `expected`/`found`
  cuando la variante del error los expone.
- Nunca se hacen panics por sintaxis inválida; todo fluye por `Result<T, Error>`.

---

## Orden de implementación (actualizado a LALRPOP)

Sigue este orden; cada paso tiene tests de entradas válidas **e** inválidas y
da feedback inmediato:

### Paso 1 — Setup LALRPOP + AST
- `ast.rs`: todas las estructuras/enums del esquema anterior.
- `build.rs` en la raíz + dependencia `lalrpop`; `grammar.lalrpop` mínimo que compile.
- `mod.rs`: `parse(sql) -> Result<Statement>` como wrapper fino + mapeo de errores a `Error::Syntax`.
- Test: el wrapper compila y un `SELECT * FROM t` mínimo parsea.

### Paso 2 — Expresiones (WHERE) en la gramática
- Reglas de precedencia en `grammar.lalrpop`: `or_expr` → `and_expr` → `not_expr` →
  `comparison` → `operand`, con keywords case-insensitive (`match` con `r"(?i)AND"` etc.).
- Test: `a = 1 AND b <> 'x'`, `NOT a`, precedencia de `OR`/`AND`, paréntesis.

### Paso 3 — CREATE TABLE (regla en `grammar.lalrpop`)
- Test: varias columnas, `NOT NULL` / `NULL`, tipo inválido, `)` faltante.

### Paso 4 — INSERT
- Test: sin lista de columnas y con lista; múltiples tuplas; aridad de tuplas.

### Paso 5 — SELECT
- Test: `*`, lista de columnas, con y sin `WHERE`.

### Paso 6 — UPDATE
- Test: una y varias asignaciones, con y sin `WHERE`.

### Paso 7 — DELETE
- Test: con y sin `WHERE`.

### Paso 8 — Tutorial
- Conectar el parser al stub del REPL en `main.rs`: leer una línea, llamar a
  `parse`, imprimir el AST (con `Debug`). Opcional pero didáctico; deja el REPL
  listo para la fase de ejecución.

---

## Tests

### Unitarios
- Tests `#[cfg(test)]` dentro de `grammar.lalrpop` (vía módulo generado) y `ast.rs` / `mod.rs` (mapeo de errores).

### Integración: `tests/parser_tests.rs`
- Mismo estilo que `tests/storage_tests.rs`: helpers + un `mod` por sentencia.
- `fn parse(sql: &str) -> basalt_db::Result<Statement>` de conveniencia.
- Casos `valid_*`: asertan igualdad con el AST esperado (`PartialEq`).
- Casos `invalid_*`: asertan `Error::Syntax` y comprueban un fragmento del
  mensaje (`assert!(matches!(...))` + `msg.contains(...)`).

### Criterios de aceptación → tests
| Criterio | Cómo se garantiza |
|----------|-------------------|
| AST correcto para las 5 sentencias | tests `valid_*` con igualdad de AST |
| Errores de sintaxis claros | tests `invalid_*` que comprueban línea/columna y `expected ... found ...` |

### Comandos esperados (repositorio en verde)
```powershell
cargo test
cargo clippy --all-targets
```

---

## Qué se ha implementado en esta fase

Fase cerrada el 2026-09-08. El parser manual previsto inicialmente se sustituyó
por LALRPOP (ver "Decisión: por qué LALRPOP" arriba); todo lo descrito en este
documento como "actualizado a LALRPOP" refleja lo implementado.

### Módulos

```
src/parser/
    mod.rs            # lalrpop_mod!(grammar) + parse() + mapeo ParseError -> Error::Syntax
                      # (offset de byte a line/column) + re-exports del AST
    ast.rs            # Statement, CreateTable/Insert/Select/Update/DeleteStatement,
                      # ColumnDef, DataType, SelectItem, Literal, Expr, BinaryOp
                      # (todos con Debug, Clone, PartialEq; Copy/Eq donde aplica)
    grammar.lalrpop   # única fuente de verdad: bloque match (19 keywords case-insensitive
                      # con prioridad sobre IDENT), reglas sintácticas en minúsculas,
                      # lexemas en MAYÚSCULAS, action code que construye ast.rs
build.rs (raíz)       # lalrpop::process_root(): genera el parser en OUT_DIR durante el build
```

Ficheros eliminados durante la fase: `src/parser/grammar.pest` (experimento
descartado) y `src/parser/build.rs` (ubicación incorrecta del shim).

### API pública

```rust
// src/parser/mod.rs, re-exportado vía basalt_db::parser
pub fn parse(sql: &str) -> Result<Statement>;   // una sentencia; `;` final opcional
pub use ast::{Statement, Expr, Literal, BinaryOp, ...};
```

`src/error.rs` expone `Error::Syntax(String)` con formato
`Syntax error at line L, column C: ...`, incluyendo tokens `expected` cuando
LALRPOP los proporciona. Nunca hay panics por sintaxis inválida.

### Recuento de tests (todo en verde)

- `tests/parser_tests.rs`: **51 tests** de integración en 8 módulos
  (`create_table`, `insert`, `select`, `update`, `delete`, `expr`, `lexing`,
  `error`): `valid_*` con igualdad total de AST, `invalid_*` con
  `Error::Syntax` + fragmentos `line/column`.
- Suite completa: 13 lib + 51 parser + 26 storage + 1 doctest = **91 passed**.
- `cargo clippy --all-targets` limpio (solo un warning preexistente en
  `src/error.rs`, ajeno a esta fase).
- Evidencia capturada: `docs/phases/phase-2-proof.txt` (transcript de 10
  sentencias de ejemplo + log de tests) y demo conservada en
  `examples/parse_demo.rs` (regenerar con `cargo run --example parse_demo`).

### Desviaciones respecto al plan

1. **Sin tests unitarios `#[cfg(test)]` en `grammar.lalrpop`/`ast.rs`**: los
   ficheros `.lalrpop` no admiten tests unitarios de forma idiomática; toda la
   cobertura vive en `tests/parser_tests.rs`. Los criterios de aceptación se
   cumplen igualmente (AST + errores claros).
2. **Paso 8 (REPL) pendiente**: `src/main.rs` sigue siendo el stub de eco de la
   Fase 1; conectar `parse` al REPL queda para la fase de ejecución.
3. **Separador `","` inline en listas**: las reglas de lista usan el literal
   `","` en vez del terminal `COMMA` porque LALRPOP excluye los string
   literals de los valores de secuencia (un terminal nombrado inyectaría `()`
   en cada tupla). Documentado con un comentario en la gramática.
4. **Dependencias finales**: `[dependencies] lalrpop-util = "0.23.1"` y
   `[build-dependencies] lalrpop = "0.23.1"` (no `lalrpop` en dependencies).