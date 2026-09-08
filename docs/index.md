# BasaltDB

> An educational relational database engine implemented in Rust.

BasaltDB answers one question: **how does a relational database actually
work internally?** It implements the core components of a relational DBMS
from scratch — storage, catalog, SQL parsing, execution, indexes,
transactions, recovery — favouring clarity and modularity over performance.

Start with [Setup & Tooling](instructions.md), then follow the development
in [Phases](phases/phase-1.md), or read the [key concepts](key-concepts/what-is-a-page.md).

## Project status

Early development. The engine grows incrementally, one phase at a time:

- **Phase 1** — [Storage engine](phases/phase-1.md): pages, heap files, tuples, catalog.
- **Phase 2** — [SQL parser](phases/phase-2.md): LALRPOP grammar producing an AST.
