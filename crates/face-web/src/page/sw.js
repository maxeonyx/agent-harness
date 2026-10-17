// The page's service worker, for notifications alone (`chrome.js`): it
// controls no page and serves nothing. A click on one brings forward a tab
// of this face, and tells it which chat to open, or opens one on that chat.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("notificationclick", (e) => {
  e.notification.close();
  const id = e.notification.data;
  e.waitUntil((async () => {
    const tabs = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const tab = tabs.find((c) => c.focused) || tabs[0];
    if (!tab) return self.clients.openWindow(`/#${id}`);
    await tab.focus();
    tab.postMessage({ open: id });
  })());
});
