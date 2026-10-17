# The logo of poste

The pill with three holes: the island at the top of the screen, folded, with its workspaces. Orange
`#fb9f44` on a dark background, `#c3610d` on a light one; the ground is `#14120f` and the light
text `#ece5d8`.

| File | For |
| --- | --- |
| `mark.svg` | the mark alone, for a dark background (the default) |
| `mark-on-light.svg` | the mark alone, for a light background |
| `icon.svg` | the icon: the mark on a dark rounded square |
| `icon-full.svg` | the full icon: an orange square with a dark pill |
| `favicon.svg`, `favicon.ico` | the favicon, drawn for 16 to 64 pixels |
| `screen.svg` | the island on a transparent ground, no wallpaper and no screen: what plays, the workspaces, the time and a timer, the launcher unfolded on a search, and the song playing, drawn (no capture); the header of the poste article on kristenjestin.fr |
| `social-preview.svg` | the social preview of the repository and the banner of the README: the mark, the name, the tagline, and `screen.svg` framed below them |

`png/` holds the icon at 128, 256, 512 and 1024 pixels, `apple-touch-icon.png`, the favicon at 16
and 32, `screen.png` and `social-preview.png`.

The name is set in Unbounded SemiBold, the text in Open Sauce Sans and the commands in JetBrains
Mono (all three under the SIL Open Font License). They are not in the repository: with them
installed, the preview is rendered by librsvg:

```sh
rsvg-convert assets/brand/screen.svg -o assets/brand/png/screen.png
rsvg-convert assets/brand/social-preview.svg -o assets/brand/png/social-preview.png
```

librsvg needs the TTF files: with the WOFF2 ones it quietly falls back to another typeface.

`png/social-preview.png` is uploaded by hand in the repository's settings on GitHub (Settings,
General, Social preview), as GitHub offers no other way.
