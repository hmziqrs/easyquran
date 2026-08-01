//! Fail semantics must not be "simplified": [`check`]/[`RateLimitLayer`] fail-CLOSED (503) on store error; [`dedup_nx`]/[`release_dedup`] fail-OPEN so a store outage never 5xx's the caller.

#![forbid(unsafe_code)]

mod abuse;
mod error;
mod hooks;
mod ip;
mod layer;
mod store;

pub use abuse::{check, dedup_nx, release_dedup, AbuseLimiterConfig, BlockScope, LimiterDecision};
pub use error::GateError;
pub use hooks::{LimiterHooks, NoHooks};
pub use ip::{ClientIpSource, FnIpSource, IpSource};
pub use layer::{BlockInfo, PathKey, RateLimitLayer, RateLimitLayerBuilder};
pub use store::{BucketSnapshot, InMemoryStore, RateLimitStore};
