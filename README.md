# 🪨 BasaltDB

> **A relational database engine built from first principles.**

BasaltDB is an educational database engine written in **Rust** whose goal is to explore how relational database management systems work internally.

Rather than focusing on production performance, BasaltDB aims to implement the core components of a modern RDBMS step by step, making the internals understandable and extensible.

---

## ✨ Goals

- 📖 Learn how relational databases actually work
- 🦀 Build everything in Rust
- 🗃️ Implement storage from scratch
- 🌳 Explore indexing structures
- ⚙️ Design a simple query execution engine
- 🔒 Understand transactions and recovery
- 📈 Build a complete SQL execution pipeline

---

## 🛠️ Planned Features

- [ ] Page-based storage engine
- [ ] Buffer pool
- [ ] Heap files
- [ ] B+ Tree indexes
- [ ] Catalog and metadata
- [ ] SQL parser
- [ ] Logical query planner
- [ ] Physical execution engine
- [ ] Transactions
- [ ] Write-Ahead Logging (WAL)
- [ ] Recovery

---

## 🧱 Architecture (planned)

```
          SQL
           │
           ▼
      SQL Parser
           │
           ▼
      Query Planner
           │
           ▼
     Execution Engine
           │
 ┌─────────┴─────────┐
 ▼                   ▼
Buffer Pool      Catalog
 │
 ▼
Storage Engine
 │
 ▼
 Disk Pages
```

---

## 📚 Why?

The objective of BasaltDB is not to compete with PostgreSQL or SQLite.

Instead, it serves as a learning project that explores the internal architecture of relational database systems by implementing them from the ground up.

---

## 🚧 Project Status

Early development.

The implementation will be built incrementally, starting with the storage engine and progressively adding higher-level database functionality.

---

> *"Strong systems are built on solid foundations."* 🪨