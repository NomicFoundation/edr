//! Phase 0 spike: revm@41 ↔ revm@38 conversion layer for op-revm.
//!
//! Proves that op-revm@20 (which speaks revm@38) can execute on top of a
//! revm@41 `Database`, with results converted back to revm@41 types, without
//! altering EVM semantics. See `docs/revm-41-op-compat-plan.md`.
//!
//! Throwaway code: validated pieces get copied into `edr_op::revm_compat`;
//! this crate is never merged.

pub mod convert;
pub mod db_bridge;
pub mod hardfork;
