# Strata in the current Omarchy theme, the accent in grey: Strata uses one accent for hover,
# icons, breadcrumbs and the selection alike, so none of them takes the theme's color.
# Personal template (~/.config/omarchy/themed), rendered by Omarchy into
# ~/.local/state/omarchy/current/theme/strata.toml at each theme change;
# ~/.config/strata/themes/omarchy-poste.toml is a link to it (step strata-theme-link) and
# Strata's settings.toml (poste-data) selects it. Strata reads it at start: restart Strata after
# a theme change. Format: docs/themes.md of Strata. Only keys every Omarchy theme has.
name = "Omarchy (poste)"
background = "{{ background }}"
surface = "{{ lighter_background }}"
text = "{{ foreground }}"
accent = "{{ dark_foreground }}"
danger = "{{ red }}"
# Strata blends these from the text over the background: the text, translucent (10%, 5%, 36%).
muted = "{{ foreground }}1a"
highlight = "{{ foreground }}0d"
border = "{{ foreground }}5c"
dim_text = "{{ dark_foreground }}"
syntax_keyword = "{{ magenta }}"
syntax_string = "{{ green }}"
syntax_constant = "{{ yellow }}"
syntax_type = "{{ cyan }}"
syntax_preprocessor = "{{ bright_magenta }}"
