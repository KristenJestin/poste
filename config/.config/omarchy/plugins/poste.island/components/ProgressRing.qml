import QtQuick

// Progress ring (island activities): discreet track, and the arc of
// `progress` (0 → 1) that starts from the top, clockwise.
Canvas {
  id: ring
  property real progress: 0
  property color color: "white"
  property color track: Qt.rgba(1, 1, 1, 0.16)
  property real lineWidth: 2.6
  onProgressChanged: requestPaint()
  onColorChanged: requestPaint()
  onTrackChanged: requestPaint()
  onWidthChanged: requestPaint()
  onPaint: {
    var ctx = getContext("2d")
    ctx.reset()
    var r = Math.min(width, height) / 2 - lineWidth / 2
    var cx = width / 2, cy = height / 2
    ctx.lineWidth = lineWidth
    ctx.lineCap = "round"
    ctx.strokeStyle = track
    ctx.beginPath(); ctx.arc(cx, cy, r, 0, 2 * Math.PI); ctx.stroke()
    var p = Math.max(0, Math.min(1, progress))
    if (p > 0.001) {
      ctx.strokeStyle = color
      ctx.beginPath(); ctx.arc(cx, cy, r, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * p, false); ctx.stroke()
    }
  }
}
