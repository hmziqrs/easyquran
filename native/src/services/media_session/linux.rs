//! MPRIS on the session bus, on a thread of its own: without a bus the
//! thread ends and nothing is shown.

use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
    time::Instant,
};

use zbus::{
    blocking::{Connection, connection},
    interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, Value},
};

use super::{Command, Labels, NowPlaying, Sink};

const NAME: &str = "org.mpris.MediaPlayer2.easyquran";
const PATH: &str = "/org/mpris/MediaPlayer2";
const TRACK: &str = "/fyi/easyquran/track";

static UPDATES: OnceLock<Sender<Option<NowPlaying>>> = OnceLock::new();

struct Root;

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {}

    fn quit(&self) {}

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        crate::APP_NAME
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

struct Player {
    sink: Arc<Sink>,
    now: Option<NowPlaying>,
    /// When `now` was shown: clients extrapolate from `Position`.
    since: Instant,
}

impl Player {
    fn position_now(&self) -> f64 {
        match &self.now {
            Some(now) if now.playing => {
                now.position + self.since.elapsed().as_secs_f64() * f64::from(now.rate)
            }
            Some(now) => now.position,
            None => 0.0,
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn play(&self) {
        (self.sink)(Command::Play);
    }

    fn pause(&self) {
        (self.sink)(Command::Pause);
    }

    fn play_pause(&self) {
        (self.sink)(Command::Toggle);
    }

    fn stop(&self) {
        (self.sink)(Command::Pause);
    }

    fn previous(&self) {
        (self.sink)(Command::Previous);
    }

    fn next(&self) {
        (self.sink)(Command::Next);
    }

    /// `offset` is in microseconds; the player's step is its own.
    fn seek(&self, offset: i64) {
        if offset < 0 {
            (self.sink)(Command::SeekBackward);
            return;
        }
        (self.sink)(Command::SeekForward);
    }

    fn set_position(&self, _track: ObjectPath<'_>, _position: i64) {}

    fn open_uri(&self, _uri: &str) {}

    #[zbus(property)]
    fn playback_status(&self) -> &str {
        match &self.now {
            Some(now) if now.playing => "Playing",
            Some(_) => "Paused",
            None => "Stopped",
        }
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.now.as_ref().map_or(1.0, |now| f64::from(now.rate))
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        let Some(now) = &self.now else {
            return HashMap::new();
        };
        let data = &now.metadata;
        let mut map = HashMap::from([
            (
                "mpris:trackid".to_owned(),
                Value::from(ObjectPath::from_static_str_unchecked(TRACK)),
            ),
            ("xesam:title".to_owned(), Value::from(data.title.clone())),
            (
                "xesam:artist".to_owned(),
                Value::from(vec![data.artist.clone()]),
            ),
            ("xesam:album".to_owned(), Value::from(data.album.clone())),
        ]);
        if now.duration > 0.0 {
            map.insert("mpris:length".to_owned(), Value::from(micros(now.duration)));
        }
        map
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        micros(self.position_now())
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    #[zbus(signal)]
    async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;
}

fn micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.0) as i64
}

pub fn install(sink: Sink, _: &Labels) {
    let (tx, rx) = channel();
    if UPDATES.set(tx).is_err() {
        return;
    }
    let spawned = thread::Builder::new()
        .name("eq-mpris".to_owned())
        .spawn(move || {
            if let Err(error) = serve(Arc::new(sink), &rx) {
                log::info!("audio: no MPRIS: {error}");
            }
        });
    if let Err(error) = spawned {
        log::warn!("audio: no MPRIS thread: {error}");
    }
}

/// On the bus while something plays: a player that shows nothing is not
/// listed.
fn serve(sink: Arc<Sink>, updates: &Receiver<Option<NowPlaying>>) -> zbus::Result<()> {
    let connection = connection::Builder::session()?
        .serve_at(PATH, Root)?
        .serve_at(
            PATH,
            Player {
                sink,
                now: None,
                since: Instant::now(),
            },
        )?
        .build()?;
    let mut named = false;
    while let Ok(now) = updates.recv() {
        if now.is_some() && !named {
            connection.request_name(NAME)?;
            named = true;
        }
        let gone = now.is_none();
        show(&connection, now)?;
        if gone && named {
            connection.release_name(NAME)?;
            named = false;
        }
    }
    Ok(())
}

fn show(connection: &Connection, now: Option<NowPlaying>) -> zbus::Result<()> {
    let player = connection.object_server().interface::<_, Player>(PATH)?;
    let position = now.as_ref().map_or(0, |now| micros(now.position));
    {
        let mut player = player.get_mut();
        player.now = now;
        player.since = Instant::now();
    }
    let emitter = player.signal_emitter();
    let player = player.get();
    zbus::block_on(async {
        player.playback_status_changed(emitter).await?;
        player.rate_changed(emitter).await?;
        player.metadata_changed(emitter).await?;
        Player::seeked(emitter, position).await
    })
}

pub fn update(now: Option<&NowPlaying>) {
    if let Some(updates) = UPDATES.get() {
        let _ = updates.send(now.cloned());
    }
}
