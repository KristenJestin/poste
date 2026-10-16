/* Zen in the island's style (poste.island): deep black backgrounds, neutral
 * surfaces, accent only for what is active or focused.
 * Personal template (~/.config/omarchy/themed), rendered by Omarchy into
 * ~/.local/state/omarchy/current/theme/zen.css at each theme change; the
 * userChrome.css of Zen's default profile is a link to it (step zen-userchrome).
 * Zen reads it at start: restart Zen after a theme change.
 * Variables: zen-styles/zen-theme.css in Zen's browser/omni.ja. Zen sets some of
 * them from JavaScript (inline styles): a user sheet needs !important to win.
 * Live editing: devtools.debugger.remote-enabled to true in about:config, then
 * Ctrl+Shift+Alt+I, Style Editor, filter "userChrome" (saving writes through the
 * link: copy the result into this template). */

/* Private windows keep Zen's own dark look, to stay recognizable. */
:root:not([zen-private-window='true']) {
  color-scheme: {{ mode }} !important;
  --zen-primary-color: {{ accent }} !important;
  --zen-branding-dark: {{ background }} !important;

  /* Surfaces: neutral greys above the island's black, never tinted by the accent */
  --zen-colors-primary: color-mix(in srgb, {{ foreground }} 10%, {{ background }}) !important;
  --zen-colors-secondary: color-mix(in srgb, {{ foreground }} 14%, {{ background }}) !important;
  --zen-colors-tertiary: {{ background }} !important;
  --zen-colors-hover-bg: color-mix(in srgb, {{ foreground }} 8%, {{ background }}) !important;
  --zen-colors-primary-foreground: {{ foreground }} !important;
  --zen-colors-border: color-mix(in srgb, {{ foreground }} 8%, transparent) !important;
  --zen-colors-border-contrast: color-mix(in srgb, {{ foreground }} 6%, transparent) !important;
  --zen-colors-input-bg: {{ lighter_background }} !important;
  --zen-input-border-color: color-mix(in srgb, {{ foreground }} 10%, transparent) !important;

  /* Window background: the island's black instead of the workspace gradient */
  --zen-main-browser-background: {{ background }} !important;
  --zen-main-browser-background-toolbar: {{ background }} !important;
  --zen-themed-toolbar-bg-transparent: {{ background }} !important;
  --zen-grainy-background-opacity: 0 !important;

  /* Text */
  --toolbox-textcolor: {{ foreground }} !important;
  --toolbar-color-scheme: {{ mode }} !important;
  --toolbar-field-color: {{ foreground }} !important;
  --link-color: {{ accent }} !important;

  /* URL bar, panels, dialogs */
  --zen-urlbar-background: {{ lighter_background }} !important;
  --toolbar-field-background-color: {{ lighter_background }} !important;
  --toolbar-field-focus-background-color: {{ lighter_background }} !important;
  --zen-dialog-background: {{ lighter_background }} !important;
  --zen-in-content-dialog-background: {{ lighter_background }} !important;
  --arrowpanel-background: {{ lighter_background }} !important;
  --arrowpanel-color: {{ foreground }} !important;
  --arrowpanel-border-color: color-mix(in srgb, {{ foreground }} 8%, transparent) !important;
  --panel-separator-color: color-mix(in srgb, {{ foreground }} 8%, transparent) !important;

  /* Tabs and buttons */
  --tab-selected-bgcolor: color-mix(in srgb, {{ foreground }} 10%, transparent) !important;
  --tab-background-color-hover: color-mix(in srgb, {{ foreground }} 6%, transparent) !important;
  --toolbarbutton-background-color-hover: color-mix(in srgb, {{ foreground }} 7%, transparent) !important;
  --toolbarbutton-background-color-active: color-mix(in srgb, {{ foreground }} 10%, transparent) !important;
  --focus-outline-color: {{ accent }} !important;
}

/* The theme decides light or dark, not the system (Zen's zen.view.window.scheme). */
:root:not([zen-private-window='true']) :is(panel, menupopup, .zen-browser-generic-background, #urlbar, #browser) {
  color-scheme: {{ mode }} !important;
}

/* Each workspace (zen-workspace) writes its own text color, accent and scheme (inline style). */
:root:not([zen-private-window='true']) zen-workspace {
  color-scheme: {{ mode }} !important;
  --toolbox-textcolor: {{ foreground }} !important;
  --zen-primary-color: {{ accent }} !important;
}

/* The gradient of the workspace is written on these two elements (inline style). */
:root:not([zen-private-window='true']) :is(#zen-browser-background, #zen-toolbar-background) {
  --zen-main-browser-background: {{ background }} !important;
  --zen-main-browser-background-toolbar: {{ background }} !important;
  --zen-main-browser-background-old: {{ background }} !important;
  --zen-main-browser-background-toolbar-old: {{ background }} !important;
}

:root:not([zen-private-window='true']) ::selection {
  background-color: {{ selection }} !important;
}
