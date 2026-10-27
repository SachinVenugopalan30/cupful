-- Proves the bundled SQLite has FTS5 (PLAN.md M0). M2's foods_fts depends on it.
CREATE VIRTUAL TABLE fts5_check USING fts5(name);
DROP TABLE fts5_check;
