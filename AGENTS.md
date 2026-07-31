# AGENTS.md

# BasaltDB

> Educational relational database engine implemented in Rust.
>
> The objective is **not** to compete with PostgreSQL or SQLite, but to study, understand and implement the core components of a relational DBMS from scratch following modern software engineering practices.

---

# Project Vision

BasaltDB is an educational and research-oriented implementation of a simplified relational database engine.

The project should prioritize:

- Clean architecture.
- Extensibility.
- Readability.
- Modularity.
- Correctness over performance.
- Learning internal DBMS concepts.

The implementation should remain sufficiently realistic so that each subsystem resembles those found in production databases while keeping the overall complexity manageable.

The project should avoid unnecessary shortcuts that hide how a database engine actually works.

---

# Goals

Implement a simplified relational database supporting:

- persistent storage
- relational tables
- SQL parsing
- query execution
- indexes
- transactions (simplified)
- recovery (simplified)

The project is intended to evolve incrementally through well-defined phases.

---

# Non-goals

The following features are intentionally out of scope:

- distributed databases
- replication
- clustering
- authentication
- authorization
- networking
- client/server protocol
- stored procedures
- triggers
- cost-based optimizer
- MVCC
- full SQL standard compliance

---

# General Principles

## Simplicity

Always choose the simplest implementation that correctly demonstrates the underlying database concept.

Avoid premature optimization.

---

## Educational Focus

Every subsystem should expose clearly how real databases work internally.

When possible, implementations should resemble PostgreSQL or SQLite concepts without copying their complexity.

---

## Layer Separation

Each module should have one responsibility.

Storage must not depend on SQL.

Parser must not depend on storage.

Execution should orchestrate lower layers.

---

## Extensibility

Every abstraction should make future improvements possible.

Avoid monolithic implementations.

Prefer traits over giant match statements.

---

# Proposed Architecture

```
              SQL

               │

         SQL Parser

               │

              AST

               │

      Logical Execution

               │

      Physical Operators

               │

       Storage Engine

               │

       Buffer Manager

               │

          Disk Pages
```

---

# Development Roadmap

## Phase 0

Architecture

Deliverables

- project structure
- module boundaries
- storage format specification
- documentation

---

## Phase 1

Storage Engine

Implement

- page manager
- heap file
- tuple serialization
- page allocation
- slotted pages

Concepts

- disk pages
- tuple layout
- serialization
- free space management

---

## Phase 2

System Catalog

Implement

- tables metadata
- schemas
- columns
- internal catalog

Concepts

- metadata management
- catalog tables

---

## Phase 3

SQL Parser

Implement

Supported SQL

- CREATE TABLE
- INSERT
- SELECT
- UPDATE
- DELETE

Components

- lexer
- parser
- AST

---

## Phase 4

Execution Engine

Implement

- sequential scan
- projection
- filtering
- insert executor
- update executor
- delete executor

Concepts

- iterator model
- execution operators

---

## Phase 5

Indexes

Implement

First

- hash index

Later

- B+ tree

Concepts

- page splits
- search trees
- index maintenance

---

## Phase 6

Transactions

Implement

- BEGIN
- COMMIT
- ROLLBACK

Concepts

- ACID
- transaction manager

---

## Phase 7

Logging

Implement

- Write Ahead Log
- checkpoints (optional)

Concepts

- durability
- crash recovery

---

## Phase 8

Concurrency

Implement

Simplified locking

- shared lock
- exclusive lock

Concepts

- lock manager
- isolation

---

# Suggested Module Structure

```
src/

    catalog/
    common/
    parser/
    planner/
    executor/
    storage/
    buffer/
    page/
    heap/
    tuple/
    index/
    transaction/
    recovery/
    sql/
    error/
```

The exact organization may evolve over time.

---

# Core Components

## Storage Engine

Responsible for

- pages
- heap files
- serialization
- persistence

Must never depend on SQL.

---

## Buffer Manager

Responsible for

- page cache
- dirty pages
- eviction
- page pinning

---

## Page Manager

Responsible for

- fixed-size pages
- page allocation
- reading
- writing

---

## Heap File

Responsible for

- tuple insertion
- tuple deletion
- tuple lookup

---

## Tuple

Responsible for

- row serialization
- null bitmap
- variable-length values

---

## Catalog

Responsible for

- table definitions
- schemas
- metadata

---

## SQL Parser

Responsible for converting SQL text into an AST.

No execution logic belongs here.

---

## Planner

Transforms the AST into executable operators.

Initially this may simply wrap the AST.

---

## Executor

Runs physical operators.

Operators should follow an iterator-based design whenever possible.

---

## Index Manager

Responsible for maintaining indexes independently from storage.

---

## Transaction Manager

Responsible for transaction lifecycle.

---

# Storage Concepts

The storage engine should use fixed-size pages.

Example

```
Database File

Page 0

Page 1

Page 2

...
```

Each page should eventually implement a slotted-page layout.

```
+--------------------+
| Header             |
+--------------------+
| Slot Directory     |
|                    |
|                    |
+--------------------+
| Free Space         |
+--------------------+
| Tuple Data         |
+--------------------+
```

---

# SQL Scope

Supported SQL should remain intentionally limited.

Examples

```
CREATE TABLE

INSERT

SELECT

UPDATE

DELETE
```

Complex SQL features may be added later.

---

# Rust Guidelines

Prefer

- ownership
- traits
- enums
- Result
- generics

Avoid

- unnecessary cloning
- Rc unless required
- unsafe unless absolutely necessary

Favor explicit types.

Avoid macros unless they significantly improve readability.

---

# Error Handling

Never panic for recoverable situations.

Prefer

```
Result<T, DatabaseError>
```

Errors should remain descriptive.

---

# Testing Strategy

Every subsystem should include

- unit tests
- integration tests

Storage components should be tested independently from SQL.

---

# Documentation

Every public module should include documentation explaining

- purpose
- responsibilities
- limitations

Complex algorithms should reference the corresponding database concept.

---

# Coding Style

Prioritize

- readability
- small functions
- descriptive naming
- low coupling
- high cohesion

Avoid large files.

Avoid functions exceeding roughly 100 lines whenever possible.

---

# Future Extensions

Potential future work

- joins
- aggregation
- GROUP BY
- ORDER BY
- B+ tree optimization
- query optimizer
- MVCC
- secondary indexes
- statistics
- cost estimation
- query planner
- vectorized execution
- columnar storage
- compression

---

# References

Recommended references while implementing:

- "Database System Concepts" (Silberschatz, Korth, Sudarshan)
- "Database Internals" (Alex Petrov)
- "Architecture of a Database System" (Hellerstein, Stonebraker, Hamilton)
- "Readings in Database Systems" ("Red Book")
- SQLite Architecture Documentation
- PostgreSQL Internals
- CMU 15-445 / 15-645: Database Systems
- MIT 6.830 Database Systems

---

# Guiding Philosophy

BasaltDB is designed to answer one question:

> **How does a relational database actually work internally?**

Every implementation decision should favor clarity, modularity, and educational value over completeness or raw performance.