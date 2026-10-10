import QtQuick

// Défilement à la molette, à poser dans une ListView ou une Flickable :
// `WheelScroll {}`. Qt fait défiler d'un cran par une animation qu'il
// relance à chaque événement ; une molette haute résolution (G502) envoie
// plusieurs petits événements par cran, et chaque relance perd ce qui restait
// à parcourir, d'où un défilement très lent. Ici chaque événement avance d'un
// pas fixe, cumulé jusqu'au bord.
WheelHandler {
  id: wheel

  // Pixels par cran (angleDelta de 120).
  property real step: 60

  // La ListView ou la Flickable : posé dedans, le gestionnaire se retrouve sur
  // son contentItem.
  readonly property Item view: parent && parent.contentY === undefined ? parent.parent : parent

  // Le gestionnaire passe avant la Flickable et accepte l'événement : elle ne
  // le voit plus.
  acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
  target: null

  function scrollBy(dy) {
    const v = wheel.view
    if (!v) return
    const top = v.originY - v.topMargin
    const bottom = v.originY + v.contentHeight + v.bottomMargin - v.height
    v.cancelFlick()
    v.contentY = Math.max(top, Math.min(Math.max(top, bottom), v.contentY + dy))
  }

  onWheel: function(event) {
    // Pavé tactile : déplacement au pixel ; molette : en crans.
    const dy = event.pixelDelta.y !== 0 ? -event.pixelDelta.y : -event.angleDelta.y / 120 * wheel.step
    if (dy !== 0) wheel.scrollBy(dy)
  }
}
