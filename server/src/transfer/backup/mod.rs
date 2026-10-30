// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON backup and restore (PLAN.md §14 M5).
//!
//! `GET /api/backup` writes one self-contained document for a user: profile, profile versions,
//! days, live meals with their items, every food those rows reference, aliases, portions and
//! saved products. Credentials, providers, sync bookkeeping and tombstones never leave.
//! `POST /api/restore` replaces exactly the same set of rows in one transaction.

mod doc;
mod export;
mod restore;
mod validate;

pub use export::export;
pub use restore::restore;
