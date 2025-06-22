self.addEventListener('install', event => {
  console.log('Service worker installed');
  self.skipWaiting(); // Optional: skip waiting to activate right away
});

self.addEventListener('activate', event => {
  console.log('Service worker activated');
});

self.addEventListener('fetch', event => {
  // Optionally add caching logic here
});