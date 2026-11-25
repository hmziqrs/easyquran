//! firebase-messaging on Android, over JNI: the SDK's classes are looked up
//! at run time, so this compiles without it, and finds them only when the
//! owner's package carries its libraries. Firebase itself is configured by
//! the SDK's own provider, from the `google-services.json` values in
//! `android/res/values`.
//!
//! `nsh-extra`'s `push` has no service of its own on Android: [`Fcm`] is its
//! provider. `FcmService.java` reports what the SDK delivers while the app
//! runs (a new token, a message); a message behind the app is shown by the
//! SDK, and its press opens the app's activity, which NSH reports.

use std::time::Duration;

use futures_channel::oneshot;
use nsh::jni::{
    JNIEnv,
    errors::Result as JniResult,
    objects::{JClass, JObject, JString, JValue},
};
use nsh_core::JsonValue;
use nsh_extra::push::{DeviceToken, PushProvider, RemoteNotification};

use super::{DeleteGuard, Event};

/// How long the service has to answer.
const WAIT: Duration = Duration::from_secs(60);

/// The provider `nsh-extra` registers with.
struct Fcm;

impl PushProvider for Fcm {
    fn register(&self) {
        worker(|| match fetch() {
            Ok(Some(token)) => {
                nsh_extra::push::provide_token(DeviceToken::from_string(token));
            }
            // Not the SDK's message: it may quote what it was given.
            _ => nsh_extra::push::provide_registration_failure("FCM gave no token"),
        });
    }
}

/// The SDK is in the app, and its provider configured the default app.
pub(super) fn configured() -> bool {
    nsh::with_env(|env| {
        let app = load(env, "com.google.firebase.FirebaseApp")?;
        let context = context();
        let apps = env
            .call_static_method(
                &app,
                "getApps",
                "(Landroid/content/Context;)Ljava/util/List;",
                &[JValue::Object(&context)],
            )?
            .l()?;
        let empty = env.call_method(&apps, "isEmpty", "()Z", &[])?.z()?;
        // The messaging library too, not only Firebase's common one.
        load(env, "com.google.firebase.messaging.FirebaseMessaging")?;
        Ok(!empty)
    })
    .unwrap_or(false)
}

pub(super) fn install() {
    nsh_extra::push::set_provider(Some(std::sync::Arc::new(Fcm)));
}

/// A registration's token, or a new one of the service: both are FCM's.
pub(super) fn registered(token: &DeviceToken) {
    super::emit(Event::Token(token.as_str().to_owned()));
}

/// The FCM token [`Fcm`] registered.
pub(super) fn fcm_token() -> oneshot::Receiver<Option<String>> {
    let (answer, token) = oneshot::channel();
    let _ = answer.send(nsh_extra::push::token().map(|token| token.as_str().to_owned()));
    token
}

pub(super) fn delete_token(guard: DeleteGuard) {
    worker(move || {
        let _guard = guard;
        let _ = nsh::with_env(|env| {
            let messaging = messaging(env)?;
            let task = task(env, &messaging, "deleteToken")?;
            wait(env, &task).map(drop)
        });
    });
}

/// Runs `job` on a thread of its own: waiting for the service blocks it.
fn worker(job: impl FnOnce() + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("push-fcm".into())
        .spawn(job);
    if let Err(error) = spawned {
        log::warn!("push: no thread to ask FCM on: {error}");
    }
}

/// `FirebaseMessaging.getInstance().getToken()`, awaited.
fn fetch() -> JniResult<Option<String>> {
    nsh::with_env(|env| {
        let messaging = messaging(env)?;
        let task = task(env, &messaging, "getToken")?;
        let token = wait(env, &task)?;
        if token.is_null() {
            return Ok(None);
        }
        Ok(Some(env.get_string(&JString::from(token))?.into()))
    })
}

fn messaging<'local>(env: &mut JNIEnv<'local>) -> JniResult<JObject<'local>> {
    let class = load(env, "com.google.firebase.messaging.FirebaseMessaging")?;
    env.call_static_method(
        &class,
        "getInstance",
        "()Lcom/google/firebase/messaging/FirebaseMessaging;",
        &[],
    )?
    .l()
}

fn task<'local>(
    env: &mut JNIEnv<'local>,
    messaging: &JObject<'_>,
    method: &str,
) -> JniResult<JObject<'local>> {
    env.call_method(
        messaging,
        method,
        "()Lcom/google/android/gms/tasks/Task;",
        &[],
    )?
    .l()
}

/// `Tasks.await(task, 60, SECONDS)`: the task's result, or the exception it
/// ended with. Off the UI thread.
fn wait<'local>(env: &mut JNIEnv<'local>, task: &JObject<'_>) -> JniResult<JObject<'local>> {
    let tasks = load(env, "com.google.android.gms.tasks.Tasks")?;
    let seconds = env
        .get_static_field(
            "java/util/concurrent/TimeUnit",
            "SECONDS",
            "Ljava/util/concurrent/TimeUnit;",
        )?
        .l()?;
    let limit = i64::try_from(WAIT.as_secs()).unwrap_or(i64::MAX);
    env.call_static_method(
        &tasks,
        "await",
        "(Lcom/google/android/gms/tasks/Task;JLjava/util/concurrent/TimeUnit;)Ljava/lang/Object;",
        &[
            JValue::Object(task),
            JValue::Long(limit),
            JValue::Object(&seconds),
        ],
    )?
    .l()
}

/// The application context.
fn context() -> JObject<'static> {
    // SAFETY: NSH sets the process's application context, a global reference
    // it never deletes, when the first `NshView` is made, before an app's
    // code runs.
    unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) }
}

/// The app's class `name`, from its own class loader: a thread Rust started
/// finds only the system's classes through `FindClass`.
fn load<'local>(env: &mut JNIEnv<'local>, name: &str) -> JniResult<JClass<'local>> {
    let context = context();
    let loader = env
        .call_method(&context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
        .l()?;
    let name = env.new_string(name)?;
    let class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    Ok(JClass::from(class))
}

/// `FcmService.nativeToken`: the service changed the token.
#[unsafe(no_mangle)]
extern "system" fn Java_fyi_easyquran_push_FcmService_nativeToken(
    mut env: JNIEnv<'_>,
    _: JClass<'_>,
    token: JString<'_>,
) {
    if token.is_null() {
        return;
    }
    if let Ok(token) = env.get_string(&token) {
        nsh_extra::push::provide_token(DeviceToken::from_string(String::from(token)));
    }
}

/// `FcmService.nativeMessage`: a message arrived. Nothing of it comes along;
/// the answer does not matter, as the service shows nothing itself.
#[unsafe(no_mangle)]
extern "system" fn Java_fyi_easyquran_push_FcmService_nativeMessage(_: JNIEnv<'_>, _: JClass<'_>) {
    let _ = nsh_extra::push::provide_notification(RemoteNotification::new("", "", JsonValue::Null));
}
