/* Omarchy, au style de l'île (kris.island) : fonds noirs profonds, Adwaita
 * Sans, accent seulement pour ce qui est actif ou cliquable, titres blancs
 * hiérarchisés par la taille, éléments arrondis en cartes, liserés discrets.
 * Modèle perso (~/.config/omarchy/themed), remplace celui d'Omarchy ; rendu
 * dans chaque coffre par omarchy-theme-set-obsidian (thème « Omarchy »). */

.theme-dark, .theme-light {
  /* Fonds : l'île (noir profond), les panneaux juste au-dessus */
  --background-primary: {{ background }};
  --background-primary-alt: {{ lighter_background }};
  --background-secondary: {{ dark_background }};
  --background-secondary-alt: {{ darker_background }};
  --background-modifier-hover: color-mix(in srgb, {{ foreground }} 6%, transparent);
  --background-modifier-active-hover: color-mix(in srgb, {{ accent }} 14%, transparent);
  --background-modifier-border: color-mix(in srgb, {{ foreground }} 8%, transparent);
  --background-modifier-border-hover: color-mix(in srgb, {{ foreground }} 14%, transparent);
  --background-modifier-border-focus: color-mix(in srgb, {{ accent }} 55%, transparent);
  --background-modifier-form-field: color-mix(in srgb, {{ foreground }} 5%, transparent);
  --divider-color: color-mix(in srgb, {{ foreground }} 7%, transparent);

  /* Texte */
  --text-normal: {{ foreground }};
  --text-muted: {{ dark_foreground }};
  --text-faint: {{ muted }};
  --text-on-accent: #0b0b0c;
  --text-selection: color-mix(in srgb, {{ accent }} 28%, transparent);
  --text-highlight-bg: color-mix(in srgb, {{ accent }} 22%, transparent);

  /* Accent : liens, actif, curseur */
  --text-accent: {{ accent }};
  --text-accent-hover: {{ accent }};
  --interactive-accent: {{ accent }};
  --interactive-accent-hover: color-mix(in srgb, {{ accent }} 85%, white);
  --interactive-normal: color-mix(in srgb, {{ foreground }} 7%, transparent);
  --interactive-hover: color-mix(in srgb, {{ foreground }} 12%, transparent);
  --text-link: {{ accent }};
  --link-color: {{ accent }};
  --link-external-color: {{ accent }};
  --link-unresolved-color: {{ dark_foreground }};
  --caret-color: {{ accent }};
  --checkbox-color: {{ accent }};
  --checkbox-color-hover: {{ accent }};
  --checkbox-marker-color: #0b0b0c;
  --checkbox-radius: 6px;

  /* Polices : celles de l'île */
  --font-interface-theme: "Adwaita Sans", "Inter", sans-serif;
  --font-text-theme: "Adwaita Sans", "Inter", sans-serif;
  --font-monospace-theme: "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
  --line-height-normal: 1.6;

  /* Titres : blancs, hiérarchie par la taille */
  --h1-color: {{ bright_foreground }};
  --h2-color: {{ bright_foreground }};
  --h3-color: {{ light_foreground }};
  --h4-color: {{ light_foreground }};
  --h5-color: {{ foreground }};
  --h6-color: {{ dark_foreground }};
  --h1-size: 1.85em;
  --h2-size: 1.45em;
  --h3-size: 1.2em;
  --h4-size: 1.05em;
  --h1-weight: 700;
  --h2-weight: 650;
  --h3-weight: 600;
  --inline-title-color: {{ bright_foreground }};
  --inline-title-weight: 700;

  /* Arrondis, comme les cartes de l'île */
  --radius-s: 6px;
  --radius-m: 10px;
  --radius-l: 14px;
  --radius-xl: 18px;

  /* Code */
  --code-background: color-mix(in srgb, {{ foreground }} 5%, transparent);
  --code-normal: {{ light_foreground }};
  --code-comment: {{ muted }};
  --code-keyword: {{ magenta }};
  --code-function: {{ blue }};
  --code-string: {{ green }};
  --code-value: {{ orange }};
  --code-property: {{ cyan }};
  --code-tag: {{ red }};
  --code-operator: {{ dark_foreground }};
  --code-punctuation: {{ dark_foreground }};

  /* Tags : pastilles teintées d'accent */
  --tag-color: {{ accent }};
  --tag-color-hover: {{ accent }};
  --tag-background: color-mix(in srgb, {{ accent }} 12%, transparent);
  --tag-background-hover: color-mix(in srgb, {{ accent }} 20%, transparent);
  --tag-border-width: 0px;
  --tag-radius: 999px;
  --tag-padding-x: 0.6em;

  /* Citations */
  --blockquote-border-color: {{ accent }};
  --blockquote-border-thickness: 3px;
  --blockquote-background-color: color-mix(in srgb, {{ foreground }} 3%, transparent);

  /* Tableaux */
  --table-border-color: color-mix(in srgb, {{ foreground }} 8%, transparent);
  --table-header-background: color-mix(in srgb, {{ foreground }} 4%, transparent);
  --table-header-weight: 600;

  /* Erreurs, succès */
  --text-error: {{ red }};
  --text-success: {{ green }};
  --text-warning: {{ yellow }};

  /* Graphe */
  --graph-line: color-mix(in srgb, {{ foreground }} 14%, transparent);
  --graph-node: {{ dark_foreground }};
  --graph-node-focused: {{ accent }};
  --graph-node-tag: {{ accent }};
  --graph-node-attachment: {{ muted }};
  --graph-node-unresolved: {{ muted }};

  /* Barres de défilement fines */
  --scrollbar-bg: transparent;
  --scrollbar-thumb-bg: color-mix(in srgb, {{ foreground }} 12%, transparent);
  --scrollbar-active-thumb-bg: color-mix(in srgb, {{ foreground }} 22%, transparent);
}

/* Blocs de code : cartes arrondies avec liseré */
.markdown-rendered pre,
.markdown-source-view.mod-cm6 .HyperMD-codeblock-bg {
  background: color-mix(in srgb, {{ foreground }} 4%, transparent);
}
.markdown-rendered pre {
  border: 1px solid color-mix(in srgb, {{ foreground }} 8%, transparent);
  border-radius: var(--radius-m);
}
.markdown-rendered :not(pre) > code,
.cm-s-obsidian span.cm-inline-code {
  border-radius: 6px;
  padding: 0.12em 0.4em;
}

/* Citations : barre d'accent arrondie, fond léger */
.markdown-rendered blockquote {
  border-radius: 0 var(--radius-m) var(--radius-m) 0;
  padding: 0.6em 1em;
}

/* Encadrés (callouts) : cartes */
.callout {
  border-radius: var(--radius-l);
  border: 1px solid color-mix(in srgb, rgb(var(--callout-color)) 25%, transparent);
  background: color-mix(in srgb, rgb(var(--callout-color)) 7%, transparent);
}

/* Barre latérale : la note ouverte en pastille d'accent */
.nav-file-title, .nav-folder-title, .tree-item-self {
  border-radius: 8px;
}
.nav-file-title.is-active, .tree-item-self.is-active {
  background: color-mix(in srgb, {{ accent }} 14%, transparent);
  color: {{ accent }};
}

/* Onglets */
.workspace-tab-header {
  border-radius: 10px 10px 0 0;
}
.workspace-tab-header.is-active .workspace-tab-header-inner-title {
  color: {{ bright_foreground }};
}
.workspace-leaf.mod-active .workspace-tab-header.is-active .workspace-tab-header-inner-title {
  color: {{ accent }};
}

/* Palette de commandes, recherche rapide, fenêtres : l'île */
.prompt, .modal, .suggestion-container, .menu {
  background: {{ background }};
  border: 1px solid color-mix(in srgb, {{ foreground }} 9%, transparent);
  border-radius: var(--radius-xl);
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.55);
}
.suggestion-item.is-selected, .menu-item.selected:not(.is-disabled):not(.is-label) {
  background: color-mix(in srgb, {{ foreground }} 8%, transparent);
  border-radius: 10px;
}
.prompt-input {
  font-size: 1.05em;
}

/* Propriétés (en-tête YAML) : discrètes */
.metadata-container {
  border-radius: var(--radius-m);
  background: color-mix(in srgb, {{ foreground }} 3%, transparent);
  padding: 0.4em 0.6em;
}

/* Surlignage */
.markdown-rendered mark, .cm-highlight {
  border-radius: 4px;
  padding: 0 0.15em;
}
