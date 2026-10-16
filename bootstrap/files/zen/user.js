// Prefs of Zen's default profile, put in place by poste (step zen-userchrome): a link to
// bootstrap/files/zen/user.js. Zen applies them at each start, over prefs.js.

// Read chrome/userChrome.css (the poste-theme colors, rendered by Omarchy).
user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
// The Browser Toolbox (Ctrl+Shift+Alt+I), to edit userChrome.css live. Its remote
// connection (devtools.debugger.remote-enabled) stays off: switched on in about:config
// only while editing.
user_pref("devtools.chrome.enabled", true);
