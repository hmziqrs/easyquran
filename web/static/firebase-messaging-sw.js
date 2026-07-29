/* ════════════════════════════════════════════════════════════════════════
   firebase-messaging-sw.js — Firebase Cloud Messaging background worker.

   A *classic* service worker (importScripts, not ESM) so it can load the
   Firebase compat SDK + config synchronously at evaluation time — which is what
   reliable background push needs: a push can wake this worker in a fresh
   context with no page open, so the config must be available immediately.

   The web config is injected same-origin via importScripts("/firebase-config.js")
   (a prerendered endpoint that writes self.__easyquranFirebase). That keeps this
   static file free of the config and keeps the build working with no config set
   (the endpoint then writes an empty object and this worker no-ops).

   The SDK is loaded from gstatic at the exact version pinned below — keep it in
   sync with `firebase` in package.json. (Modular ESM in a worker would avoid the
   CDN, but SvelteKit's worker env can't read runtime env, and the static build
   must still succeed with no config; this is the documented FCM web pattern.)
   ════════════════════════════════════════════════════════════════════════ */

// NOTE: bump together with `firebase` in package.json.
importScripts("https://www.gstatic.com/firebasejs/12.16.0/firebase-app-compat.js");
importScripts("https://www.gstatic.com/firebasejs/12.16.0/firebase-messaging-compat.js");
importScripts("/firebase-config.js");

var CFG = self.__easyquranFirebase;
if (CFG && CFG.apiKey && CFG.appId && !firebase.apps.length) {
  firebase.initializeApp(CFG);
  var messaging = firebase.messaging();

  // Background messages. If the payload carries a top-level `notification`, the
  // SDK displays it automatically — showing again would duplicate. For data-only
  // pushes we build a notification ourselves (and forward any `url` for clicks).
  messaging.onBackgroundMessage(function (payload) {
    if (payload.notification) return;
    var data = payload.data || {};
    var title = data.title || "EasyQuran";
    var url = data.url || "/";
    self.registration.showNotification(title, {
      body: data.body || "",
      icon: "/icons/icon-192.png",
      badge: "/icons/icon-192.png",
      tag: data.tag || undefined,
      data: Object.assign({ url: url }, data),
    });
  });
}

// Open/focus the right tab on click — works for SDK-displayed and manual
// notifications alike (the destination lives in notification.data.url).
self.addEventListener("notificationclick", function (event) {
  event.notification.close();
  var data = (event.notification && event.notification.data) || {};
  var target = data.url || "/";
  event.waitUntil(
    (async function () {
      var all = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
      for (var i = 0; i < all.length; i++) {
        var client = all[i];
        if ("focus" in client) {
          client.focus();
          if ("navigate" in client) {
            try {
              await client.navigate(target);
            } catch {
              /* ignore — focus is enough */
            }
          }
          return;
        }
      }
      if (self.clients.openWindow) await self.clients.openWindow(target);
    })(),
  );
});
