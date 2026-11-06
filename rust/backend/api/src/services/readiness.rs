//! Whether the app database answers. Kept out of `modules/quran_v1`, whose handlers
//! stay free of app-database access; only its readiness probe calls this.

use crate::AppState;

pub async fn database_ok(state: &AppState) -> bool {
    state.sea_db.ping().await.is_ok()
}
