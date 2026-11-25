//! `MediaSession` and the playback service, through `Playback.java`
//! (`android/playback/playback.jar`, packaged by `[package.metadata.nsh.android]`):
//! the session callback and service are classes, which `nsh::proxy` cannot make.

use std::sync::OnceLock;

use nsh::{
    jni::{
        JNIEnv,
        errors::Result as JniResult,
        objects::{GlobalRef, JClass, JObject, JValue},
    },
    proxy::{self, Proxy},
};

use super::{Command, Labels, NowPlaying, Sink};

const CLASS: &str = "fyi.easyquran.media.Playback";

struct Bridge {
    class: GlobalRef,
    /// Kept: dropping it would silence the commands.
    _commands: Proxy,
}

static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// The application `Context`.
pub(in crate::services) fn context() -> JObject<'static> {
    // SAFETY: NSH sets the process's application context, a global
    // reference it never deletes, when the first `NshView` is made, before
    // an app's code runs.
    unsafe { JObject::from_raw(ndk_context::android_context().context().cast()) }
}

/// The app's class `name`: native threads' own lookup sees the system's only.
fn load_class<'local>(env: &mut JNIEnv<'local>, name: &str) -> JniResult<JObject<'local>> {
    let loader = env
        .call_method(
            context(),
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )?
        .l()?;
    let name = env.new_string(name)?;
    env.call_method(
        &loader,
        "loadClass",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        &[JValue::Object(&name)],
    )?
    .l()
}

pub fn install(sink: Sink, labels: &Labels) {
    if BRIDGE.get().is_some() {
        return;
    }
    let made = nsh::with_env(|env| {
        let class = load_class(env, CLASS)?;
        let commands = proxy::implement(
            env,
            &["java.util.function.IntConsumer"],
            move |env, _, args| {
                let number = env.get_object_array_element(args, 0)?;
                let number = env.call_method(&number, "intValue", "()I", &[])?.i()?;
                if let Some(command) = Command::from_number(number) {
                    sink(command);
                }
                Ok(None)
            },
        )?;
        let texts = [
            &labels.channel,
            &labels.play,
            &labels.pause,
            &labels.previous,
            &labels.next,
        ];
        let array =
            env.new_object_array(texts.len() as i32, "java/lang/String", JObject::null())?;
        for (i, text) in texts.iter().enumerate() {
            let text = env.new_string(text)?;
            env.set_object_array_element(&array, i as i32, text)?;
        }
        let class = JClass::from(class);
        env.call_static_method(
            &class,
            "install",
            "(Ljava/util/function/IntConsumer;[Ljava/lang/String;)V",
            &[JValue::Object(commands.as_obj()), JValue::Object(&array)],
        )?;
        Ok(Bridge {
            class: env.new_global_ref(&class)?,
            _commands: commands,
        })
    });
    match made {
        Ok(bridge) => {
            let _ = BRIDGE.set(bridge);
        }
        Err(error) => log::warn!("audio: no media session: {error}"),
    }
}

pub fn update(now: Option<&NowPlaying>) {
    let Some(bridge) = BRIDGE.get() else {
        return;
    };
    let shown = nsh::with_env(|env| {
        let class = <&JClass<'_>>::from(bridge.class.as_obj());
        let Some(now) = now else {
            env.call_static_method(
                class,
                "clear",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&context())],
            )?;
            return Ok(());
        };
        let title = env.new_string(&now.metadata.title)?;
        let artist = env.new_string(&now.metadata.artist)?;
        let album = env.new_string(&now.metadata.album)?;
        env.call_static_method(
            class,
            "update",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZJJF)V",
            &[
                JValue::Object(&context()),
                JValue::Object(&title),
                JValue::Object(&artist),
                JValue::Object(&album),
                JValue::Bool(u8::from(now.playing)),
                JValue::Long(millis(now.position)),
                JValue::Long(millis(now.duration)),
                JValue::Float(now.rate),
            ],
        )?;
        Ok(())
    });
    if let Err(error) = shown {
        log::warn!("audio: media session: {error}");
    }
}

fn millis(seconds: f64) -> i64 {
    (seconds * 1000.0) as i64
}
