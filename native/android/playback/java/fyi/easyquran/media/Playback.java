package fyi.easyquran.media;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.pm.ServiceInfo;
import android.media.AudioManager;
import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;

import java.util.function.IntConsumer;

/**
 * The recitation player's media session, its notification and the foreground service that keeps the app running while
 * it plays ({@code src/services/media_session/android.rs} calls the static methods over JNI). Java because Android takes
 * the session's callback and the service as classes only. Everything after a call's hop runs on the UI thread.
 */
public final class Playback extends Service {
    private static final String TAG = "easyquran";
    private static final String CHANNEL = "recitation";
    private static final int NOTIFICATION = 0x4551;
    private static final String EXTRA_COMMAND = "command";

    // `Command`'s numbers in `media_session.rs`.
    private static final int PLAY = 0;
    private static final int PAUSE = 1;
    private static final int PREVIOUS = 3;
    private static final int NEXT = 4;
    private static final int SEEK_BACKWARD = 5;
    private static final int SEEK_FORWARD = 6;

    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static IntConsumer sink;
    /** Channel, play, pause, previous and next, in the app's language. */
    private static String[] labels;
    private static MediaSession session;
    private static Playback running;
    private static boolean playing;
    private static MediaMetadata metadata;

    private final BroadcastReceiver noisy = new BroadcastReceiver() {
        @Override
        public void onReceive(Context context, Intent intent) {
            send(PAUSE);
        }
    };

    /** Sets where the system's commands go. */
    public static void install(IntConsumer commands, String[] texts) {
        MAIN.post(() -> {
            sink = commands;
            labels = texts;
        });
    }

    /** Shows what plays. */
    public static void update(Context app, String title, String artist, String album, boolean isPlaying,
            long positionMs, long durationMs, float rate) {
        MAIN.post(() -> {
            try {
                show(app, title, artist, album, isPlaying, positionMs, durationMs, rate);
            } catch (RuntimeException e) {
                // Shown or not, playback goes on.
                Log.w(TAG, "media session", e);
            }
        });
    }

    /** Nothing plays: the session, the notification and the service go. */
    public static void clear(Context app) {
        MAIN.post(() -> {
            playing = false;
            if (session != null) {
                session.setActive(false);
                session.release();
                session = null;
            }
            if (running != null) running.end();
            manager(app).cancel(NOTIFICATION);
        });
    }

    private static void show(Context app, String title, String artist, String album, boolean isPlaying,
            long positionMs, long durationMs, float rate) {
        if (session == null) {
            session = new MediaSession(app, TAG);
            session.setCallback(new Commands(), MAIN);
            session.setActive(true);
        }
        playing = isPlaying;
        metadata = new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, title)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadata.METADATA_KEY_ALBUM, album)
                .putLong(MediaMetadata.METADATA_KEY_DURATION, durationMs > 0 ? durationMs : -1)
                .build();
        session.setMetadata(metadata);
        session.setPlaybackState(new PlaybackState.Builder()
                .setActions(PlaybackState.ACTION_PLAY | PlaybackState.ACTION_PAUSE | PlaybackState.ACTION_PLAY_PAUSE
                        | PlaybackState.ACTION_SKIP_TO_PREVIOUS | PlaybackState.ACTION_SKIP_TO_NEXT
                        | PlaybackState.ACTION_REWIND | PlaybackState.ACTION_FAST_FORWARD)
                .setState(isPlaying ? PlaybackState.STATE_PLAYING : PlaybackState.STATE_PAUSED, positionMs,
                        isPlaying ? rate : 0f)
                .build());
        if (isPlaying && running == null) {
            channel(app);
            // Refused in the background (Android 12): playing starts in the app.
            app.startForegroundService(new Intent(app, Playback.class));
        } else if (running != null) {
            running.refresh();
        }
    }

    private static void send(int command) {
        if (sink != null) sink.accept(command);
    }

    private static NotificationManager manager(Context context) {
        return (NotificationManager) context.getSystemService(NOTIFICATION_SERVICE);
    }

    private static String label(int index) {
        return labels != null && index < labels.length ? labels[index] : "";
    }

    private static void channel(Context context) {
        NotificationChannel channel =
                new NotificationChannel(CHANNEL, label(0), NotificationManager.IMPORTANCE_LOW);
        channel.setShowBadge(false);
        manager(context).createNotificationChannel(channel);
    }

    private static final class Commands extends MediaSession.Callback {
        @Override
        public void onPlay() {
            send(PLAY);
        }

        @Override
        public void onPause() {
            send(PAUSE);
        }

        @Override
        public void onSkipToPrevious() {
            send(PREVIOUS);
        }

        @Override
        public void onSkipToNext() {
            send(NEXT);
        }

        @Override
        public void onRewind() {
            send(SEEK_BACKWARD);
        }

        @Override
        public void onFastForward() {
            send(SEEK_FORWARD);
        }
    }

    @Override
    public void onCreate() {
        super.onCreate();
        running = this;
        registerReceiver(noisy, new IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY));
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        running = this;
        // Always first: Android ends an app whose started service shows no notification.
        channel(this);
        foreground();
        int command = intent != null ? intent.getIntExtra(EXTRA_COMMAND, -1) : -1;
        if (command >= 0) send(command);
        if (session == null) end();
        else refresh();
        return START_NOT_STICKY;
    }

    @Override
    public void onTaskRemoved(Intent root) {
        clear(getApplicationContext());
    }

    @Override
    public void onDestroy() {
        unregisterReceiver(noisy);
        if (running == this) running = null;
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void end() {
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        running = null;
    }

    private void foreground() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        } else {
            startForeground(NOTIFICATION, notification());
        }
    }

    /** Foreground while it plays; paused, a notification that can be swiped away. */
    private void refresh() {
        if (playing) {
            foreground();
            return;
        }
        stopForeground(STOP_FOREGROUND_DETACH);
        manager(this).notify(NOTIFICATION, notification());
    }

    private Notification notification() {
        Notification.MediaStyle style = new Notification.MediaStyle().setShowActionsInCompactView(0, 1, 2);
        if (session != null) style.setMediaSession(session.getSessionToken());
        Intent open = getPackageManager().getLaunchIntentForPackage(getPackageName());
        Notification.Builder builder = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_media_play)
                .setStyle(style)
                .setVisibility(Notification.VISIBILITY_PUBLIC)
                .setOnlyAlertOnce(true)
                .setOngoing(playing)
                .addAction(action(android.R.drawable.ic_media_previous, 3, PREVIOUS))
                .addAction(playing
                        ? action(android.R.drawable.ic_media_pause, 2, PAUSE)
                        : action(android.R.drawable.ic_media_play, 1, PLAY))
                .addAction(action(android.R.drawable.ic_media_next, 4, NEXT));
        if (metadata != null) {
            builder.setContentTitle(metadata.getString(MediaMetadata.METADATA_KEY_TITLE))
                    .setContentText(metadata.getString(MediaMetadata.METADATA_KEY_ARTIST));
        }
        if (open != null) {
            builder.setContentIntent(PendingIntent.getActivity(this, 0, open, PendingIntent.FLAG_IMMUTABLE));
        }
        return builder.build();
    }

    private Notification.Action action(int icon, int text, int command) {
        Intent intent = new Intent(this, Playback.class).putExtra(EXTRA_COMMAND, command);
        PendingIntent pending = PendingIntent.getForegroundService(this, command + 1, intent,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Action.Builder(
                android.graphics.drawable.Icon.createWithResource(this, icon), label(text), pending).build();
    }
}
