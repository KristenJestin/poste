import QtQuick

// Progression qui fait le tour du bord d'une carte arrondie (minuteur
// déplié) : une piste discrète, et la part restante qui part du milieu du
// haut, dans le sens des aiguilles.
Canvas {
  id: border
  property real progress: 0
  property real radius: 26
  property color color: "orange"
  property color track: Qt.rgba(1, 1, 1, 0.12)
  property real lineWidth: 4
  onProgressChanged: requestPaint()
  onColorChanged: requestPaint()
  onWidthChanged: requestPaint()
  onHeightChanged: requestPaint()

  // Trace le contour sur une longueur `len` (tout le tour si len >= périmètre).
  function trace(ctx, len) {
    var lw = lineWidth / 2, w = width - lineWidth, h = height - lineWidth
    var r = Math.min(radius, w / 2, h / 2)
    var x0 = lw, y0 = lw
    var arc = Math.PI * r / 2
    // Segments : [longueur, fonction de tracé partiel]
    var segs = [
      [w / 2 - r, function(f) { ctx.lineTo(x0 + w / 2 + (w / 2 - r) * f, y0) }],
      [arc, function(f) { ctx.arc(x0 + w - r, y0 + r, r, -Math.PI / 2, -Math.PI / 2 + Math.PI / 2 * f) }],
      [h - 2 * r, function(f) { ctx.lineTo(x0 + w, y0 + r + (h - 2 * r) * f) }],
      [arc, function(f) { ctx.arc(x0 + w - r, y0 + h - r, r, 0, Math.PI / 2 * f) }],
      [w - 2 * r, function(f) { ctx.lineTo(x0 + w - r - (w - 2 * r) * f, y0 + h) }],
      [arc, function(f) { ctx.arc(x0 + r, y0 + h - r, r, Math.PI / 2, Math.PI / 2 + Math.PI / 2 * f) }],
      [h - 2 * r, function(f) { ctx.lineTo(x0, y0 + h - r - (h - 2 * r) * f) }],
      [arc, function(f) { ctx.arc(x0 + r, y0 + r, r, Math.PI, Math.PI + Math.PI / 2 * f) }],
      [w / 2 - r, function(f) { ctx.lineTo(x0 + r + (w / 2 - r) * f, y0) }]
    ]
    ctx.beginPath()
    ctx.moveTo(x0 + w / 2, y0)
    for (var i = 0; i < segs.length && len > 0; i++) {
      var f = Math.min(1, len / Math.max(0.0001, segs[i][0]))
      segs[i][1](f)
      len -= segs[i][0]
    }
    ctx.stroke()
  }
  onPaint: {
    var ctx = getContext("2d")
    ctx.reset()
    ctx.lineWidth = lineWidth
    ctx.lineCap = "round"
    var w = width - lineWidth, h = height - lineWidth
    var r = Math.min(radius, w / 2, h / 2)
    var total = 2 * (w - 2 * r) + 2 * (h - 2 * r) + 2 * Math.PI * r
    ctx.strokeStyle = track
    trace(ctx, total + 1)
    var p = Math.max(0, Math.min(1, progress))
    if (p > 0.001) { ctx.strokeStyle = color; trace(ctx, total * p) }
  }
}
