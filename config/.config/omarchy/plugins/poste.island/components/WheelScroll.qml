import QtQuick

// Wheel scrolling, to put in a ListView or a Flickable:
// `WheelScroll {}`. Qt scrolls by one notch with an animation that it
// restarts at each event; a high-resolution wheel (G502) sends
// several small events per notch, and each restart loses what was left
// to travel, hence a very slow scrolling. Here each event advances by a
// fixed step, accumulated up to the edge.
WheelHandler {
  id: wheel

  // Pixels per notch (angleDelta of 120).
  property real step: 50

  // The ListView or the Flickable: put inside it, the handler ends up on
  // its contentItem.
  readonly property Item view: parent && parent.contentY === undefined ? parent.parent : parent

  // The handler goes before the Flickable and accepts the event: it no
  // longer sees it.
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
    // Touchpad: pixel movement; wheel: in notches.
    const dy = event.pixelDelta.y !== 0 ? -event.pixelDelta.y : -event.angleDelta.y / 120 * wheel.step
    if (dy !== 0) wheel.scrollBy(dy)
  }
}
