-- Migration 01 (down): Drop transactions, categories, and autocat tables

DROP TABLE IF EXISTS autocat;
DROP TABLE IF EXISTS categories;
DROP TABLE IF EXISTS transactions;
