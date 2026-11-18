//! Local reminder notifications over NSH `notifications` (PARITY G7, M1);
//! remote push (M2) shows through here too ([`show_push_hint`]).

use nsh_core::{notifications::Notification, prelude::show_notification};

/// The web's `PUSH_TITLE` / `PUSH_BODY` / `PUSH_TAG` (`push-privacy.ts`):
/// the only text a push ever shows, one notification replaced by the next.
// i18n: web constants in push-privacy.ts, not catalogue keys
const PUSH_TITLE: &str = "EasyQuran";
const PUSH_BODY: &str = "Open EasyQuran to view notifications.";
const PUSH_TAG: &str = "easyquran-notifications";

/// Shows the fixed public push text. Nothing of the FCM payload (account,
/// notification details) is read or shown (`publicPushPayload`).
pub async fn show_push_hint() -> bool {
    show_notification(Notification::new(PUSH_TAG, PUSH_TITLE, PUSH_BODY)).await
}
