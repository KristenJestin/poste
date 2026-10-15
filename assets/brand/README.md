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
| `social-preview.svg` | the social preview of the repository and the banner of the README: the mark, the name, the tagline, and the island unfolded into the launcher on the Record extension, its selected row framing its target window, drawn (no capture) |

`png/` holds the icon at 128, 256, 512 and 1024 pixels, `apple-touch-icon.png`, the favicon at 16
and 32, and `social-preview.png`.

The name is set in Unbounded SemiBold, the text in Open Sauce Sans and the commands in JetBrains
Mono (all three under the SIL Open Font License). They are not in the repository: with them
installed, the preview is rendered by librsvg:

```sh
rsvg-convert assets/brand/social-preview.svg -o assets/brand/png/social-preview.png
```

`png/social-preview.png` is uploaded by hand in the repository's settings on GitHub (Settings,
General, Social preview), as GitHub offers no other way.
