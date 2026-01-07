-- Migration 01 (down): Drop all tables
--
-- Drop order: tables with FK references first, then referenced tables.
-- (SQLite ignores FK constraints on DROP TABLE, but this order is clearer.)

DROP TABLE IF EXISTS formulas;
DROP TABLE IF EXISTS sheet_metadata;
DROP TABLE IF EXISTS transactions;
DROP TABLE IF EXISTS autocat;
DROP TABLE IF EXISTS categories;
