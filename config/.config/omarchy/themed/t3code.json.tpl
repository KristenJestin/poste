{
  "name": "Omarchy",
  "appearance": "{{ mode }}",
  "canvas": "{{ background }}",
  "accent": "{{ accent }}",
  "colors": {
    "chrome": "{{ background }}",
    "toolbar": "{{ background }}",
    "toolbarForeground": "{{ foreground }}",
    "toolbarBorder": "{{ mix background foreground 10% }}",
    "toolbarControl": "{{ lighter_background }}",
    "toolbarControlForeground": "{{ foreground }}",
    "toolbarControlHover": "{{ mix lighter_background foreground 6% }}",

    "surface": "{{ lighter_background }}",
    "surfaceRaised": "{{ mix lighter_background foreground 4% }}",
    "surfaceOverlay": "{{ mix lighter_background foreground 7% }}",

    "text": "{{ foreground }}",
    "textMuted": "{{ dark_foreground }}",
    "border": "{{ mix background foreground 12% }}",
    "input": "{{ mix background foreground 14% }}",
    "focus": "{{ accent }}",

    "accentForeground": "{{ darker_background }}",
    "secondary": "{{ lighter_background }}",
    "secondaryForeground": "{{ foreground }}",
    "muted": "{{ lighter_background }}",
    "mutedForeground": "{{ dark_foreground }}",
    "placeholder": "{{ muted }}",
    "secondaryLabel": "{{ dark_foreground }}",
    "iconMuted": "{{ dark_foreground }}",

    "error": "{{ mix red foreground 35% }}",
    "errorForeground": "{{ bright_foreground }}",
    "errorSurface": "{{ mix background red 12% }}",
    "warning": "{{ mix yellow foreground 35% }}",
    "warningForeground": "{{ darker_background }}",
    "warningSurface": "{{ mix background yellow 12% }}",
    "update": "{{ blue }}",
    "updateForeground": "{{ darker_background }}",
    "updateSurface": "{{ mix background blue 12% }}",

    "accentSurface": "{{ mix background accent 10% }}",
    "accentSurfaceForeground": "{{ bright_foreground }}",

    "messageSurface": "{{ mix background foreground 8% }}",
    "messageForeground": "{{ bright_foreground }}",
    "messageAction": "{{ accent }}",
    "messageActionForeground": "{{ darker_background }}",
    "messageActionHover": "{{ mix accent bright_foreground 15% }}",

    "codeBackground": "{{ dark_background }}",
    "codeForeground": "{{ foreground }}",

    "sidebar": "{{ dark_background }}",
    "sidebarForeground": "{{ foreground }}",
    "sidebarMutedForeground": "{{ dark_foreground }}",
    "sidebarControlSurface": "{{ lighter_background }}",
    "sidebarBorder": "{{ mix dark_background foreground 8% }}",
    "sidebarRowHover": "{{ mix dark_background foreground 6% }}",
    "sidebarRowActive": "{{ mix dark_background accent 12% }}",
    "sidebarRowSelected": "{{ selection }}",

    "terminalBackground": "{{ background }}",
    "terminalForeground": "{{ foreground }}",
    "terminalCursor": "{{ bright_foreground }}",
    "terminalSelection": "{{ selection }}",
    "terminalScrollbar": "{{ muted }}",
    "terminalScrollbarHover": "{{ dark_foreground }}"
  }
}
