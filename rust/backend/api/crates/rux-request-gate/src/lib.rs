//! Fail semantics must not be "simplified": [`check`]/[`RateLimitLayer`] fail-CLOSED (503) on store error; [`dedup_nx`]/[`release_dedup`] fail-OPEN so a store outage never 5xx's the caller.

#![forbid(unsafe_code)]

mod abuse;
mod error;
mod hooks;
mod ip;
mod layer;
mod store;

pub use abuse::{
    check, credential_delay_secs, dedup_nx, release_dedup, AbuseLimiterConfig, BlockScope,
    FailureMode, GateDecision, LimiterDecision, CREDENTIAL_DELAY_CAP_SECS, IP_RETRY_CAP_SECS,
};
pub use error::GateError;
pub use hooks::{LimiterHooks, NoHooks};
pub use ip::{
    ClientIpIdentitySource, ClientIpSource, FnIpSource, IdentitySource, InternalServiceId,
    IpSource, RequestIdentity, SubjectIdentitySource, VerifiedSubject,
};
pub use layer::{
    blocked_by_path, scaled_max, BlockInfo, Escalation, PathKey, RateLimitLayer,
    RateLimitLayerBuilder, LOW_SUBJECT_SCORE,
};
pub use store::{BanStatus, BucketSnapshot, InMemoryStore, QualifyingCounts, RateLimitStore};
