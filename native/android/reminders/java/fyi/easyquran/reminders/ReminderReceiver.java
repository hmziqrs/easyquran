package fyi.easyquran.reminders;

import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.util.Log;

import java.util.Calendar;
import java.util.TimeZone;

/**
 * The daily goal reminder while the app is closed (PARITY G7, M1). The app's Rust code
 * ({@code src/services/notifications/android.rs}) calls {@link #schedule} and {@link #cancel} over JNI; the alarm and
 * the system's broadcasts (boot, update, clock and zone changes: {@code nsh.json} {@code android.application}) come
 * back here in a process of their own, without the engine.
 *
 * <p>One inexact alarm at a time, for the next occurrence: each sets the one after it. It needs no exact-alarm
 * permission; the system may deliver it up to an hour late. Every text is the app's (its catalogue), kept in this
 * class's preferences.
 *
 * <p>Rebuild {@code reminders.jar} with {@code build.sh} after a change here.
 */
public final class ReminderReceiver extends BroadcastReceiver {
    private static final String TAG = "easyquran";
    private static final String FIRE = "fyi.easyquran.reminders.FIRE";
    private static final String PREFERENCES = "fyi.easyquran.reminders";
    /** The number of every reminder notification, the reminder's id being its tag. */
    private static final int NOTIFICATION = 1;
    private static final int MINUTES_PER_DAY = 24 * 60;

    /**
     * Shows {@code title} and {@code body} every day at {@code minute} of the day in {@code zone}, outside the quiet
     * hours {@code [quietStart, quietEnd)} (-1: none); a press opens {@code link}. False when the package does not
     * declare this receiver (the shared development host): nothing would fire.
     */
    public static boolean schedule(Context context, String id, String title, String body, String link, int minute,
            String zone, int quietStart, int quietEnd) {
        try {
            context.getPackageManager().getReceiverInfo(new ComponentName(context, ReminderReceiver.class), 0);
        } catch (PackageManager.NameNotFoundException e) {
            return false;
        }
        SharedPreferences saved = preferences(context);
        boolean same = id.equals(saved.getString("id", null)) && title.equals(saved.getString("title", null))
                && body.equals(saved.getString("body", null)) && link.equals(saved.getString("link", null))
                && minute == saved.getInt("minute", -1) && zone.equals(saved.getString("zone", null))
                && quietStart == saved.getInt("quietStart", -1) && quietEnd == saved.getInt("quietEnd", -1);
        // An alarm already waiting for this schedule stays: one due now and not yet delivered is not put off a day.
        if (same && pending(context, PendingIntent.FLAG_NO_CREATE) != null) return true;
        saved.edit().putString("id", id).putString("title", title).putString("body", body).putString("link", link)
                .putInt("minute", minute).putString("zone", zone).putInt("quietStart", quietStart)
                .putInt("quietEnd", quietEnd).apply();
        return setNext(context);
    }

    /** Drops the schedule, its alarm and the notification it showed. */
    public static void cancel(Context context, String id) {
        preferences(context).edit().clear().apply();
        PendingIntent pending = pending(context, PendingIntent.FLAG_NO_CREATE);
        if (pending != null) {
            alarms(context).cancel(pending);
            pending.cancel();
        }
        notifications(context).cancel(id, NOTIFICATION);
    }

    @Override
    public void onReceive(Context context, Intent intent) {
        // The alarm, or a broadcast that cleared or moved it: the next one either way.
        setNext(context);
        if (FIRE.equals(intent.getAction())) show(context);
    }

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE);
    }

    private static AlarmManager alarms(Context context) {
        return (AlarmManager) context.getSystemService(Context.ALARM_SERVICE);
    }

    private static NotificationManager notifications(Context context) {
        return (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
    }

    private static PendingIntent pending(Context context, int flag) {
        Intent intent = new Intent(context, ReminderReceiver.class).setAction(FIRE);
        return PendingIntent.getBroadcast(context, 0, intent, flag | PendingIntent.FLAG_IMMUTABLE);
    }

    private static TimeZone zone(SharedPreferences saved) {
        String id = saved.getString("zone", "");
        TimeZone zone = TimeZone.getTimeZone(id);
        // An id the system does not know reads as GMT.
        return zone.getID().equals(id) ? zone : TimeZone.getDefault();
    }

    /** Sets the alarm of the next occurrence of the saved schedule; false without one. */
    private static boolean setNext(Context context) {
        SharedPreferences saved = preferences(context);
        int minute = saved.getInt("minute", -1);
        if (minute < 0 || minute >= MINUTES_PER_DAY) return false;
        Calendar next = Calendar.getInstance(zone(saved));
        long now = next.getTimeInMillis();
        next.set(Calendar.HOUR_OF_DAY, minute / 60);
        next.set(Calendar.MINUTE, minute % 60);
        next.set(Calendar.SECOND, 0);
        next.set(Calendar.MILLISECOND, 0);
        if (next.getTimeInMillis() <= now) next.add(Calendar.DAY_OF_MONTH, 1);
        alarms(context).setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next.getTimeInMillis(),
                pending(context, PendingIntent.FLAG_UPDATE_CURRENT));
        return true;
    }

    private static boolean quiet(SharedPreferences saved) {
        int start = saved.getInt("quietStart", -1);
        int end = saved.getInt("quietEnd", -1);
        if (start < 0 || end < 0 || start == end) return false;
        Calendar now = Calendar.getInstance(zone(saved));
        int minute = now.get(Calendar.HOUR_OF_DAY) * 60 + now.get(Calendar.MINUTE);
        if (start < end) return minute >= start && minute < end;
        return minute >= start || minute < end;
    }

    private static void show(Context context) {
        SharedPreferences saved = preferences(context);
        String id = saved.getString("id", null);
        if (id == null || quiet(saved)) return;
        NotificationManager manager = notifications(context);
        if (!manager.areNotificationsEnabled()) return;
        String title = saved.getString("title", "");
        // Named again on each show: the app's language may have changed.
        manager.createNotificationChannel(new NotificationChannel(id, title, NotificationManager.IMPORTANCE_DEFAULT));
        int icon = context.getApplicationInfo().icon;
        Notification.Builder builder = new Notification.Builder(context, id)
                .setSmallIcon(icon != 0 ? icon : android.R.drawable.ic_dialog_info)
                .setContentTitle(title)
                .setContentText(saved.getString("body", ""))
                .setCategory(Notification.CATEGORY_REMINDER)
                .setAutoCancel(true)
                .setShowWhen(true);
        Intent launch = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (launch != null) {
            // The app's own link, as a tap on one anywhere opens it.
            Intent open = new Intent(Intent.ACTION_VIEW, Uri.parse(saved.getString("link", "")))
                    .setComponent(launch.getComponent())
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
            builder.setContentIntent(PendingIntent.getActivity(context, 0, open,
                    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE));
        }
        try {
            manager.notify(id, NOTIFICATION, builder.build());
        } catch (RuntimeException e) {
            Log.w(TAG, "cannot show the reminder", e);
        }
    }
}
