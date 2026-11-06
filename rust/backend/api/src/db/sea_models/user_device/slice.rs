/// Live devices per account. Registration past this retires the oldest idle
/// device (never the caller).
pub const LIVE_DEVICE_CAP: usize = 20;

/// Days a device row survives after its last register. The sweeper (A32) deletes
/// rows past `expires_at`; thread labels resolve until then.
pub const DEVICE_RETENTION_DAYS: i64 = 90;

pub const DEVICE_RETENTION_CLASS: &str = "device_registry";

/// Fields a client sends on register. Every enum is validated before this exists.
#[derive(Clone, Debug)]
pub struct NewUserDevice {
    pub user_id: i32,
    pub id: String,
    pub platform: String,
    pub client: String,
    pub form_factor: String,
    pub browser: Option<String>,
    pub os_major: Option<i32>,
    pub app_version: Option<String>,
}
