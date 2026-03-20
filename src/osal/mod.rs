/// OSAL-inspired abstractions for hardware not yet covered by veecle-osal-std.
///
/// These traits follow the VeecleOS white paper's extensibility principle:
/// "OSAL grows by introducing a new trait with a narrowly scoped responsibility."
///
/// When veecle-meta becomes available, these can be replaced by generated adapters
/// driven by a model.toml hardware definition.
pub mod gpio;
pub mod mic;
pub mod speaker;
