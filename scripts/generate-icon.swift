// Reproducible vector-drawn app icon; no external image dependencies.
import AppKit
let side = 1024
let image = NSImage(size: NSSize(width: side, height: side))
image.lockFocus()
let ctx = NSGraphicsContext.current!.cgContext
ctx.setShouldAntialias(true)
let bounds = NSRect(x: 0, y: 0, width: side, height: side)
NSColor(calibratedRed: 0.018, green: 0.022, blue: 0.035, alpha: 1).setFill()
NSBezierPath(roundedRect: bounds.insetBy(dx: 28, dy: 28), xRadius: 220, yRadius: 220).fill()
for i in 0..<52 {
    let x = 105 + (i * 173 % 811), y = 115 + (i * 293 % 794)
    NSColor(white: 0.7 + CGFloat(i % 3) * 0.1, alpha: 0.3 + CGFloat(i % 4) * 0.12).setFill()
    let d = i % 9 == 0 ? 4 : 2
    NSBezierPath(ovalIn: NSRect(x: x, y: y, width: d, height: d)).fill()
}
// The far side of the disk is gravitationally folded above the shadow.
let arch = NSBezierPath()
arch.move(to: NSPoint(x: 282, y: 475))
arch.curve(to: NSPoint(x: 742, y: 475), controlPoint1: NSPoint(x: 265, y: 858), controlPoint2: NSPoint(x: 760, y: 858))
for spread in stride(from: 92, through: 12, by: -8) {
    NSColor(calibratedRed: 1, green: 0.25, blue: 0.025, alpha: 0.025).setStroke()
    arch.lineWidth = CGFloat(spread); arch.stroke()
}
NSColor(calibratedRed: 1, green: 0.57, blue: 0.15, alpha: 1).setStroke()
arch.lineWidth = 14; arch.stroke()
// Thin photon ring around the dark silhouette.
let ring = NSBezierPath(ovalIn: NSRect(x: 300, y: 310, width: 424, height: 424))
NSColor(calibratedRed: 0.003, green: 0.004, blue: 0.007, alpha: 1).setFill(); ring.fill()
NSColor(calibratedRed: 1, green: 0.68, blue: 0.27, alpha: 1).setStroke(); ring.lineWidth = 5; ring.stroke()
// Front-side accretion disk crosses the lower half of the shadow.
let disk = NSBezierPath()
disk.move(to: NSPoint(x: 135, y: 450))
disk.curve(to: NSPoint(x: 888, y: 476), controlPoint1: NSPoint(x: 300, y: 294), controlPoint2: NSPoint(x: 747, y: 338))
for spread in stride(from: 112, through: 20, by: -8) {
    NSColor(calibratedRed: 1, green: 0.25, blue: 0.01, alpha: 0.035).setStroke()
    disk.lineWidth = CGFloat(spread); disk.stroke()
}
NSColor(calibratedRed: 1, green: 0.49, blue: 0.07, alpha: 1).setStroke(); disk.lineWidth = 26; disk.stroke()
NSColor(calibratedRed: 1, green: 0.88, blue: 0.63, alpha: 1).setStroke(); disk.lineWidth = 8; disk.stroke()
image.unlockFocus()
let rep = NSBitmapImageRep(data: image.tiffRepresentation!)!
try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
