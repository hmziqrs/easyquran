use crate::abuse::BlockScope;

pub trait LimiterHooks: Send + Sync {
    fn on_check(&self) {}
    #[allow(unused_variables)]
    fn on_allowed(&self, short_count: u64, long_count: u64) {}
    #[allow(unused_variables)]
    fn on_blocked(
        &self,
        scope: BlockScope,
        retry_after_secs: u64,
        short_count: u64,
        long_count: u64,
    ) {
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct NoHooks;

impl LimiterHooks for NoHooks {}
