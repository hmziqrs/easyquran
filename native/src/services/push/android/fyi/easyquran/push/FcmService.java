package fyi.easyquran.push;

import com.google.firebase.messaging.FirebaseMessagingService;
import com.google.firebase.messaging.RemoteMessage;

/**
 * Reports to Rust (services/push/android.rs) that FCM changed the token or delivered a message while the app runs. The
 * message itself stays here: a push shows only the app's fixed public text. Messages behind the app are shown by the
 * SDK. In a process the service started alone the app's library is not loaded: the app asks for its token again when
 * it opens, so there is nothing to keep.
 */
public final class FcmService extends FirebaseMessagingService {
    @Override
    public void onNewToken(String token) {
        try {
            nativeToken(token);
        } catch (UnsatisfiedLinkError notLoaded) {
            // The app is not running.
        }
    }

    @Override
    public void onMessageReceived(RemoteMessage message) {
        try {
            nativeMessage();
        } catch (UnsatisfiedLinkError notLoaded) {
            // The app is not running.
        }
    }

    private static native void nativeToken(String token);

    private static native void nativeMessage();
}
