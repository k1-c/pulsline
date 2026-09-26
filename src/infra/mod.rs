//! The systems Pulsline calls on: the spool on disk ([`spool`]), the SQLite
//! index built from it ([`index`]), and [`dispatch`], which carries out a
//! use case's request against them.

pub mod dispatch;
pub mod index;
pub mod spool;
