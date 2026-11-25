//! No push service: the desktops, the web, Mac Catalyst, and iOS and Android
//! builds without the `fcm` feature. [`configured`] is false, so nothing else
//! here runs.

use futures_channel::oneshot;
use nsh_extra::push::DeviceToken;

use super::DeleteGuard;

pub(super) fn configured() -> bool {
    false
}

pub(super) fn install() {}

pub(super) fn registered(_: &DeviceToken) {}

pub(super) fn fcm_token() -> oneshot::Receiver<Option<String>> {
    oneshot::channel().1
}

pub(super) fn delete_token(_: DeleteGuard) {}
