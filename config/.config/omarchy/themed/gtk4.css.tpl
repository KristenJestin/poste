/* Généré par Omarchy depuis ~/.config/omarchy/themed/gtk4.css.tpl (thème courant).
   Chargé par GTK4 / libadwaita (Nautilus…) via ~/.config/gtk-4.0/gtk.css. */

/* Accent : uniquement pour l'accent (boutons suggérés, liens, focus). */
@define-color accent_color {{ accent }};
@define-color accent_bg_color {{ accent }};
@define-color accent_fg_color {{ darker_background }};

/* Fenêtres et vues : fonds sombres neutres. */
@define-color window_bg_color {{ background }};
@define-color window_fg_color {{ foreground }};
@define-color view_bg_color {{ background }};
@define-color view_fg_color {{ foreground }};

/* Barre d'en-tête et barre latérale un cran plus sombres. */
@define-color headerbar_bg_color {{ dark_background }};
@define-color headerbar_fg_color {{ foreground }};
@define-color headerbar_border_color {{ mix dark_background foreground 8% }};
@define-color headerbar_backdrop_color {{ dark_background }};
@define-color headerbar_shade_color rgba(0, 0, 0, 0.36);
@define-color sidebar_bg_color {{ dark_background }};
@define-color sidebar_fg_color {{ foreground }};
@define-color sidebar_backdrop_color {{ dark_background }};
@define-color sidebar_border_color {{ mix dark_background foreground 8% }};
@define-color sidebar_shade_color rgba(0, 0, 0, 0.36);
@define-color secondary_sidebar_bg_color {{ dark_background }};
@define-color secondary_sidebar_fg_color {{ foreground }};

/* Cartes, popovers, dialogues : surfaces un peu plus claires. */
@define-color card_bg_color {{ lighter_background }};
@define-color card_fg_color {{ foreground }};
@define-color card_shade_color rgba(0, 0, 0, 0.36);
@define-color popover_bg_color {{ mix background foreground 6% }};
@define-color popover_fg_color {{ foreground }};
@define-color popover_shade_color rgba(0, 0, 0, 0.36);
@define-color dialog_bg_color {{ mix background foreground 4% }};
@define-color dialog_fg_color {{ foreground }};
@define-color thumbnail_bg_color {{ lighter_background }};
@define-color thumbnail_fg_color {{ foreground }};

/* Divers. */
@define-color shade_color rgba(0, 0, 0, 0.36);
@define-color scrollbar_outline_color rgba(0, 0, 0, 0.5);
@define-color destructive_color {{ red }};
@define-color destructive_bg_color {{ red }};
@define-color success_color {{ green }};
@define-color warning_color {{ yellow }};
@define-color error_color {{ red }};

/* Sélection grise, pas orange (fichiers, lignes de la barre latérale). */
:selected,
.navigation-sidebar > row:selected,
gridview > child:selected,
columnview > listview > row:selected {
  background-color: {{ mix background foreground 12% }};
  color: {{ bright_foreground }};
}
