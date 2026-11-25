//! Passkeys on Android 14 and later over the framework's
//! `android.credentials.CredentialManager`, through JNI and NSH's interface
//! proxies (`nsh::proxy`): no Java class or library of our own. The request
//! types and bundle keys are the ones Jetpack's `androidx.credentials` sends
//! the same framework calls, which the credential providers read.
//!
//! The site must list the app's signing key in `/.well-known/assetlinks.json`
//! (`delegate_permission/common.get_login_creds`). The origin the server sees
//! is `android:apk-key-hash:<hash>` (`WEBAUTHN_ANDROID_APK_KEY_HASHES`).

use std::sync::Mutex;

use eq_core::passkey::{Assertion, Attestation, CreationOptions, RequestOptions};
use futures_channel::oneshot;
use jni::{
    JNIEnv,
    errors::Result as JniResult,
    objects::{JObject, JObjectArray, JString, JValue},
};
use nsh::proxy::{self, Proxy};
use nsh_core::prelude::run_on_main_thread;

use super::PasskeyError;

/// `Build.VERSION_CODES.UPSIDE_DOWN_CAKE`, the first with the framework API.
const ANDROID_14: i32 = 34;
/// `Context.CREDENTIAL_SERVICE`.
const CREDENTIAL_SERVICE: &str = "credential";

const TYPE: &str = "androidx.credentials.TYPE_PUBLIC_KEY_CREDENTIAL";
const SUBTYPE: &str = "androidx.credentials.BUNDLE_KEY_SUBTYPE";
const SUBTYPE_CREATE: &str =
    "androidx.credentials.BUNDLE_VALUE_SUBTYPE_CREATE_PUBLIC_KEY_CREDENTIAL_REQUEST";
const SUBTYPE_GET: &str =
    "androidx.credentials.BUNDLE_VALUE_SUBTYPE_GET_PUBLIC_KEY_CREDENTIAL_OPTION";
const REQUEST_JSON: &str = "androidx.credentials.BUNDLE_KEY_REQUEST_JSON";
const DISPLAY_INFO: &str = "androidx.credentials.BUNDLE_KEY_REQUEST_DISPLAY_INFO";
const USER_ID: &str = "androidx.credentials.BUNDLE_KEY_USER_ID";
const USER_DISPLAY_NAME: &str = "androidx.credentials.BUNDLE_KEY_USER_DISPLAY_NAME";
const REGISTRATION_JSON: &str = "androidx.credentials.BUNDLE_KEY_REGISTRATION_RESPONSE_JSON";
const AUTHENTICATION_JSON: &str = "androidx.credentials.BUNDLE_KEY_AUTHENTICATION_RESPONSE_JSON";

/// `{Get,Create}CredentialException.TYPE_USER_CANCELED` end with the first;
/// the others are the WebAuthn refusals a provider reports, which the web
/// treats as cancelled too.
const CANCELLED: [&str; 3] = [
    "TYPE_USER_CANCELED",
    "TYPE_NOT_ALLOWED_ERROR",
    "TYPE_ABORT_ERROR",
];

const REQUEST_SIGNATURE: &str = "(Ljava/lang/String;Landroid/os/Bundle;Landroid/os/Bundle;)V";

pub fn available() -> bool {
    nsh::with_env(|env| {
        let sdk = env
            .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
            .i()?;
        if sdk < ANDROID_14 {
            return Ok(false);
        }
        Ok(manager(env)?.is_some())
    })
    .unwrap_or(false)
}

pub async fn register(options: CreationOptions) -> Result<Attestation, PasskeyError> {
    let json = perform(Ask::Create(options)).await?;
    Attestation::from_json(&json).map_err(malformed)
}

pub async fn assert(options: RequestOptions) -> Result<Assertion, PasskeyError> {
    let json = perform(Ask::Get(options)).await?;
    Assertion::from_json(&json).map_err(malformed)
}

/// Names the field, never its value.
fn malformed(error: eq_core::passkey::CodecError) -> PasskeyError {
    log::warn!("passkeys: the provider's answer: {error}");
    PasskeyError::Failed
}

enum Ask {
    Create(CreationOptions),
    Get(RequestOptions),
}

type Reply = Result<String, PasskeyError>;

/// The receiver of the request showing: a proxy answers only while held.
static RUNNING: Mutex<Option<Proxy>> = Mutex::new(None);

fn running() -> std::sync::MutexGuard<'static, Option<Proxy>> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

async fn perform(ask: Ask) -> Reply {
    let (reply, answer) = oneshot::channel();
    run_on_main_thread(move || start(&ask, reply))
        .await
        .map_err(|_| PasskeyError::Failed)??;
    answer.await.map_err(|_| PasskeyError::Failed)?
}

/// Shows the system's sheet for `ask` (UI thread). A request showing already
/// loses its receiver, which ends its caller's wait.
fn start(ask: &Ask, reply: oneshot::Sender<Reply>) -> Result<(), PasskeyError> {
    let earlier = running().take();
    drop(earlier);
    nsh::with_env(|env| {
        let Some(manager) = manager(env)? else {
            return Ok(Err(PasskeyError::Unsupported));
        };
        let receiver = receiver(env, reply)?;
        let context = activity(env);
        let executor = env
            .call_method(
                application(),
                "getMainExecutor",
                "()Ljava/util/concurrent/Executor;",
                &[],
            )?
            .l()?;
        let (request, method, signature) = match ask {
            Ask::Create(options) => (
                create_request(env, options)?,
                "createCredential",
                "(Landroid/content/Context;Landroid/credentials/CreateCredentialRequest;Landroid/os/CancellationSignal;Ljava/util/concurrent/Executor;Landroid/os/OutcomeReceiver;)V",
            ),
            Ask::Get(options) => (
                get_request(env, options)?,
                "getCredential",
                "(Landroid/content/Context;Landroid/credentials/GetCredentialRequest;Landroid/os/CancellationSignal;Ljava/util/concurrent/Executor;Landroid/os/OutcomeReceiver;)V",
            ),
        };
        env.call_method(
            &manager,
            method,
            signature,
            &[
                JValue::Object(&context),
                JValue::Object(&request),
                JValue::Object(&JObject::null()),
                JValue::Object(&executor),
                JValue::Object(receiver.as_obj()),
            ],
        )?;
        // The answer is posted to this thread, so it comes after this.
        *running() = Some(receiver);
        Ok(Ok(()))
    })
    .unwrap_or_else(|error| {
        log::warn!("passkeys: the request could not start: {error}");
        Err(PasskeyError::Failed)
    })
}

/// The application `Context`.
fn application() -> JObject<'static> {
    // SAFETY: NSH sets the process's application context, a global reference
    // it never deletes, when the first view is made, before an app's code
    // runs.
    unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) }
}

/// The activity in front, which the sheet opens over, as the framework asks.
/// NSH gives it to Rust nowhere, so it is read from the runtime's own class;
/// without it the application starts the sheet in a task of its own.
fn activity<'local>(env: &mut JNIEnv<'local>) -> JObject<'local> {
    match front(env) {
        Ok(activity) if !activity.is_null() => activity,
        _ => {
            let _ = env.exception_clear();
            log::debug!("passkeys: no activity in front; using the application");
            env.new_local_ref(application())
                .unwrap_or_else(|_| JObject::null())
        }
    }
}

/// `com.longbridge.nsh.Permissions.front()` (UI thread), by the app's class
/// loader: a native thread's own finds framework classes only.
fn front<'local>(env: &mut JNIEnv<'local>) -> JniResult<JObject<'local>> {
    let loader = env
        .call_method(
            application(),
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )?
        .l()?;
    let name = env.new_string("com.longbridge.nsh.Permissions")?;
    let class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    env.call_static_method(
        jni::objects::JClass::from(class),
        "front",
        "()Landroid/app/Activity;",
        &[],
    )?
    .l()
}

/// `CredentialManager`, or `None` on a device without it.
fn manager<'local>(env: &mut JNIEnv<'local>) -> JniResult<Option<JObject<'local>>> {
    let name = env.new_string(CREDENTIAL_SERVICE)?;
    let manager = env
        .call_method(
            application(),
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&name)],
        )?
        .l()?;
    Ok((!manager.is_null()).then_some(manager))
}

fn bundle<'local>(env: &mut JNIEnv<'local>) -> JniResult<JObject<'local>> {
    env.new_object("android/os/Bundle", "()V", &[])
}

fn put_string(env: &mut JNIEnv<'_>, bundle: &JObject<'_>, key: &str, value: &str) -> JniResult<()> {
    let key = env.new_string(key)?;
    let value = env.new_string(value)?;
    env.call_method(
        bundle,
        "putString",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&key), JValue::Object(&value)],
    )?;
    Ok(())
}

fn request_bundle<'local>(
    env: &mut JNIEnv<'local>,
    subtype: &str,
    json: &str,
) -> JniResult<JObject<'local>> {
    let data = bundle(env)?;
    put_string(env, &data, SUBTYPE, subtype)?;
    put_string(env, &data, REQUEST_JSON, json)?;
    Ok(data)
}

/// `android.credentials.CreateCredentialRequest`. The display info is what
/// the system's sheet names the passkey with.
fn create_request<'local>(
    env: &mut JNIEnv<'local>,
    options: &CreationOptions,
) -> JniResult<JObject<'local>> {
    let json = options.to_json();
    let data = request_bundle(env, SUBTYPE_CREATE, &json)?;
    let info = bundle(env)?;
    // `getCharSequence` reads a string.
    put_string(env, &info, USER_ID, &options.user_name)?;
    put_string(env, &info, USER_DISPLAY_NAME, &options.user_display_name)?;
    let key = env.new_string(DISPLAY_INFO)?;
    env.call_method(
        &data,
        "putBundle",
        "(Ljava/lang/String;Landroid/os/Bundle;)V",
        &[JValue::Object(&key), JValue::Object(&info)],
    )?;
    let query = request_bundle(env, SUBTYPE_CREATE, &json)?;
    let kind = env.new_string(TYPE)?;
    let builder = env.new_object(
        "android/credentials/CreateCredentialRequest$Builder",
        REQUEST_SIGNATURE,
        &[
            JValue::Object(&kind),
            JValue::Object(&data),
            JValue::Object(&query),
        ],
    )?;
    env.call_method(
        &builder,
        "build",
        "()Landroid/credentials/CreateCredentialRequest;",
        &[],
    )?
    .l()
}

/// `android.credentials.GetCredentialRequest` with the one option.
fn get_request<'local>(
    env: &mut JNIEnv<'local>,
    options: &RequestOptions,
) -> JniResult<JObject<'local>> {
    let json = options.to_json();
    let data = request_bundle(env, SUBTYPE_GET, &json)?;
    let query = request_bundle(env, SUBTYPE_GET, &json)?;
    let kind = env.new_string(TYPE)?;
    let builder = env.new_object(
        "android/credentials/CredentialOption$Builder",
        REQUEST_SIGNATURE,
        &[
            JValue::Object(&kind),
            JValue::Object(&data),
            JValue::Object(&query),
        ],
    )?;
    let option = env
        .call_method(
            &builder,
            "build",
            "()Landroid/credentials/CredentialOption;",
            &[],
        )?
        .l()?;
    let extra = bundle(env)?;
    let builder = env.new_object(
        "android/credentials/GetCredentialRequest$Builder",
        "(Landroid/os/Bundle;)V",
        &[JValue::Object(&extra)],
    )?;
    env.call_method(
        &builder,
        "addCredentialOption",
        "(Landroid/credentials/CredentialOption;)Landroid/credentials/GetCredentialRequest$Builder;",
        &[JValue::Object(&option)],
    )?;
    env.call_method(
        &builder,
        "build",
        "()Landroid/credentials/GetCredentialRequest;",
        &[],
    )?
    .l()
}

/// The `OutcomeReceiver` that answers `reply` once.
fn receiver(env: &mut JNIEnv<'_>, reply: oneshot::Sender<Reply>) -> JniResult<Proxy> {
    let reply = Mutex::new(Some(reply));
    proxy::implement(
        env,
        &["android.os.OutcomeReceiver"],
        move |env, method, args| {
            let outcome = outcome(env, method, args);
            let sender = reply.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(sender) = sender {
                // A request started since has replaced this proxy already and
                // ended its calls, so the one held is this one.
                let done = running().take();
                drop(done);
                // The caller may have stopped waiting.
                let _ = sender.send(outcome);
            }
            Ok(None)
        },
    )
}

fn outcome(env: &mut JNIEnv<'_>, method: &str, args: &JObjectArray<'_>) -> Reply {
    let read = match method {
        "onResult" => result_json(env, args).map(Ok),
        _ => error_type(env, args).map(Err),
    };
    match read {
        Ok(Ok(json)) => Ok(json),
        Ok(Err(kind)) => Err(error_of(&kind)),
        Err(error) => {
            let _ = env.exception_clear();
            log::warn!("passkeys: the answer could not be read: {error}");
            Err(PasskeyError::Failed)
        }
    }
}

fn error_of(kind: &str) -> PasskeyError {
    if CANCELLED.iter().any(|end| kind.ends_with(end)) {
        return PasskeyError::Cancelled;
    }
    log::warn!("passkeys: request failed with {kind}");
    PasskeyError::Failed
}

/// `{Get,Create}CredentialException.getType()`.
fn error_type(env: &mut JNIEnv<'_>, args: &JObjectArray<'_>) -> JniResult<String> {
    let error = env.get_object_array_element(args, 0)?;
    let kind: JString<'_> = env
        .call_method(&error, "getType", "()Ljava/lang/String;", &[])?
        .l()?
        .into();
    Ok(env.get_string(&kind)?.into())
}

/// The response JSON of a `CreateCredentialResponse` or of the credential of
/// a `GetCredentialResponse`.
fn result_json(env: &mut JNIEnv<'_>, args: &JObjectArray<'_>) -> JniResult<String> {
    let response = env.get_object_array_element(args, 0)?;
    let created = env.is_instance_of(&response, "android/credentials/CreateCredentialResponse")?;
    let (holder, key) = match created {
        true => (response, REGISTRATION_JSON),
        false => (
            env.call_method(
                &response,
                "getCredential",
                "()Landroid/credentials/Credential;",
                &[],
            )?
            .l()?,
            AUTHENTICATION_JSON,
        ),
    };
    let data = env
        .call_method(&holder, "getData", "()Landroid/os/Bundle;", &[])?
        .l()?;
    let key = env.new_string(key)?;
    let json: JString<'_> = env
        .call_method(
            &data,
            "getString",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&key)],
        )?
        .l()?
        .into();
    Ok(env.get_string(&json)?.into())
}
