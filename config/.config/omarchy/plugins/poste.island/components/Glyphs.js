.pragma library
// Island Nerd Font glyphs -> Tabler icons (assets/tabler, MIT).
// Same table as the shell's 'Poste Nerd Font' font (bootstrap/icons/glyphs.tsv):
// change an icon here, change it there too.
// Glyph.qml shows the icon when it is here, otherwise the original glyph. A line
// only appears here if its SVG (and its -filled variant if needed) is in assets/tabler.
var icons = {
  "": "layout-grid",               // E62A custom-windows
  "": "volume",                    // E638 seti-audio
  "": "terminal-2",                // E795 dev-terminal
  "": "file",                      // EA7B cod-file
  "": "world",                     // EB01 cod-globe
  "": "microphone",                // EC12 cod-mic
  "": "volume-3",                  // EEE8 fa-volume_xmark
  "": "mouse",                     // EFBA fa-computer_mouse
  "": "heart",                     // F004 fa-heart
  "": "star",                      // F005 fa-star
  "": "check",                     // F00C fa-check
  "": "power",                     // F011 fa-power_off
  "": "lock",                      // F023 fa-lock
  "": "volume-4",                  // F026 fa-volume_off
  "": "volume-2",                  // F027 fa-volume_low
  "": "volume",                    // F028 fa-volume_up
  "": "video",                     // F03D fa-video_camera
  "": "photo",                     // F03E fa-picture_o
  "": "chevron-left",              // F053 fa-chevron_left
  "": "folder-open",               // F07C fa-folder_open
  "": "clipboard",                 // F07F fa-clipboard_alt
  "": "pin",                       // F08D fa-thumbtack
  "": "folder",                    // F114 fa-folder_o
  "": "keyboard",                  // F11C fa-keyboard_o
  "": "app-window",                // F2D0 fa-window_maximize
  "": "terminal-2",                // F489 oct-terminal
  "󰀻": "layout-grid",               // F003B md-apps
  "󰁍": "arrow-left",                // F004D md-arrow_left
  "󰁔": "arrow-right",               // F0054 md-arrow_right
  "󰁹": "battery-4",                 // F0079 md-battery
  "󰁺": "battery-1",                 // F007A md-battery_10
  "󰁻": "battery-1",                 // F007B md-battery_20
  "󰁼": "battery-1",                 // F007C md-battery_30
  "󰁽": "battery-2",                 // F007D md-battery_40
  "󰁾": "battery-2",                 // F007E md-battery_50
  "󰁿": "battery-2",                 // F007F md-battery_60
  "󰂀": "battery-4",                 // F0080 md-battery_70
  "󰂁": "battery-4",                 // F0081 md-battery_80
  "󰂂": "battery-4",                 // F0082 md-battery_90
  "󰂄": "battery-charging",          // F0084 md-battery_charging
  "󰂎": "battery",                   // F008E md-battery_outline
  "󰂚": "bell",                      // F009A md-bell
  "󰂞": "bell",                      // F009E md-bell_ring
  "󰂯": "bluetooth",                 // F00AF md-bluetooth
  "󰂱": "bluetooth-connected",       // F00B1 md-bluetooth_connect
  "󰂲": "bluetooth-off",             // F00B2 md-bluetooth_off
  "󰃬": "calculator",                // F00EC md-calculator
  "󰄬": "check",                     // F012C md-check
  "󰅁": "chevron-left",              // F0141 md-chevron_left
  "󰅂": "chevron-right",             // F0142 md-chevron_right
  "󰅌": "clipboard",                 // F014C md-clipboard_outline
  "󰅍": "clipboard",                 // F014D md-clipboard_text
  "󰅖": "x",                         // F0156 md-close
  "󰆍": "terminal-2",                // F018D md-console
  "󰆏": "copy",                      // F018F md-content_copy
  "󰆒": "clipboard-text",            // F0192 md-content_paste
  "󰆴": "trash",                     // F01B4 md-delete
  "󰇅": "device-desktop",            // F01C5 md-desktop_tower
  "󰇚": "download",                  // F01DA md-download
  "󰇮": "mail",                      // F01EE md-email
  "󰈀": "network",                   // F0200 md-ethernet
  "󰈉": "eye-off",                   // F0209 md-eye_off
  "󰈑": "player-track-next",         // F0211 md-fast_forward
  "󰈔": "file",                      // F0214 md-file
  "󰉋": "folder",                    // F024B md-folder
  "󰉍": "folder-open",               // F024D md-folder_download
  "󰉖": "folder-open",               // F0256 md-folder_outline
  "󰊗": "device-gamepad-2",          // F0297 md-gamepad_variant
  "󰊚": "scale",                     // F029A md-gauge
  "󰊴": "device-gamepad-2",          // F02B4 md-google_controller
  "󰋋": "headphones",                // F02CB md-headphones
  "󰋎": "headset",                   // F02CE md-headset
  "󰋑": "heart",                     // F02D1 md-heart
  "󰋕": "heart",                     // F02D5 md-heart_outline
  "󰋩": "photo",                     // F02E9 md-image
  "󰌌": "keyboard",                  // F030C md-keyboard
  "󰌪": "leaf",                      // F032A md-leaf
  "󰌾": "lock",                      // F033E md-lock
  "󰍂": "login",                     // F0342 md-login
  "󰍉": "search",                    // F0349 md-magnify
  "󰍬": "microphone",                // F036C md-microphone
  "󰍭": "microphone-off",            // F036D md-microphone_off
  "󰍹": "device-desktop",            // F0379 md-monitor
  "󰍽": "mouse",                     // F037D md-mouse
  "󰏌": "external-link",             // F03CC md-open_in_new
  "󰏤": "player-pause",              // F03E4 md-pause
  "󰏲": "device-mobile",             // F03F2 md-phone
  "󰐃": "pin",                       // F0403 md-pin
  "󰐊": "player-play",               // F040A md-play
  "󰐕": "plus",                      // F0415 md-plus
  "󰐥": "power",                     // F0425 md-power
  "󰑋": "circle-dot",                // F044B md-record_rec
  "󰑐": "rotate",                    // F0450 md-refresh
  "󰑟": "player-track-prev",         // F045F md-rewind
  "󰒃": "shield-x",                  // F0483 md-security
  "󰒊": "send",                      // F048A md-send
  "󰒘": "shield-check",              // F0498 md-shield
  "󰒭": "player-skip-forward",       // F04AD md-skip_next
  "󰒮": "player-skip-back",          // F04AE md-skip_previous
  "󰒲": "moon",                      // F04B2 md-sleep
  "󰓃": "device-speaker",            // F04C3 md-speaker
  "󰓅": "gauge",                     // F04C5 md-speedometer
  "󰓎": "star",                      // F04CE md-star
  "󰓛": "player-stop",               // F04DB md-stop
  "󰔛": "stopwatch",                 // F051B md-timer_outline
  "󰔟": "hourglass",                 // F051F md-timer_sand
  "󰕧": "video",                     // F0567 md-video
  "󰕾": "volume",                    // F057E md-volume_high
  "󰕿": "volume-4",                  // F057F md-volume_low
  "󰖀": "volume-2",                  // F0580 md-volume_medium
  "󰖁": "volume-off",                // F0581 md-volume_off
  "󰖂": "plug-connected-x",          // F0582 md-vpn
  "󰖟": "world",                     // F059F md-web
  "󰖪": "wifi-off",                  // F05AA md-wifi_off
  "󰖯": "app-window",                // F05AF md-window_maximize
  "󰗄": "file-zip",                  // F05C4 md-zip_box
  "󰘳": "command",                   // F0633 md-apple_keyboard_command
  "󰚩": "robot",                     // F06A9 md-robot
  "󰛳": "network",                   // F06F3 md-network
  "󰜉": "rotate-clockwise",          // F0709 md-restart
  "󰝚": "music",                     // F075A md-music
  "󰝟": "volume-off",                // F075F md-volume_mute
  "󰟀": "device-desktop",            // F07C0 md-desktop_classic
  "󰣀": "server",                    // F08C0 md-ssh
  "󰤁": "moon",                      // F0901 md-power_cycle
  "󰤟": "wifi-0",                    // F091F md-wifi_strength_1
  "󰤢": "wifi-1",                    // F0922 md-wifi_strength_2
  "󰤥": "wifi-2",                    // F0925 md-wifi_strength_3
  "󰤨": "wifi",                      // F0928 md-wifi_strength_4
  "󰤮": "wifi-off",                  // F092E md-wifi_strength_off_outline
  "󰤯": "wifi-0",                    // F092F md-wifi_strength_outline
  "󰤰": "pinned-off",                // F0930 md-pin_off_outline
  "󰤱": "pinned-off",                // F0931 md-pin_outline
  "󰦝": "shield-cog",                // F099D md-shield_lock
  "󰦞": "shield-off",                // F099E md-shield_off
  "󰩟": "network",                   // F0A5F md-ip
  "󰩭": "scan",                      // F0A6D md-selection_drag
  "󰩹": "trash",                     // F0A79 md-trash_can
  "󰪹": "folder-symlink",            // F0AB9 md-file_move
  "󰴠": "clipboard-copy",            // F0D20 md-map_marker_path
  "󱐋": "bolt",                      // F140B md-lightning_bolt
  "󱚣": "robot",                     // F16A3 md-robot_excited
  "󱚤": "robot",                     // F16A4 md-robot_excited_outline
  "󱚾": "wifi"                       // F16BE md-wifi_cog
}

// Filled by default (media controls, filled heart); the outline alone looks too thin there.
var solid = { "󰈑": 1, "󰋑": 1, "󰏤": 1, "󰐊": 1, "󰑟": 1, "󰒭": 1, "󰒮": 1, "󰓛": 1 }

// Path of the icon under assets/ ('tabler/microphone'), or "".
function iconFor(glyph) {
  var name = icons[String(glyph || "")] || ""
  return name !== "" ? "tabler/" + name : ""
}

function prefersFilled(glyph) {
  return !!solid[String(glyph || "")]
}
