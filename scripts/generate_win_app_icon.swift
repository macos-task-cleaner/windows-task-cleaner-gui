import AppKit
import Foundation

// Windows 11 Fluent App Icon Generator
// Derived from macOS Task Cleaner Terminal-Craft Icon
// Adapted for Windows 11 Icon Grid, Centering & Multi-scale LOD (Level of Detail)

func renderWinIcon(size: CGFloat) -> NSImage {
    return NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
        guard let ctx = NSGraphicsContext.current?.cgContext else { return false }
        let s = size / 1024.0

        // Windows 11 Fluent proportions:
        // Centered squircle, occupying ~86.5% of canvas (balanced padding for Windows desktop grid)
        let baseSize: CGFloat = (size <= 24 ? 940.0 : (size <= 32 ? 900.0 : 886.0)) * s
        let baseInset: CGFloat = (size - baseSize) / 2.0
        // Vertical optical balance: slight upward bias (+4px in 1024) to accommodate elevation shadow
        let yOffset: CGFloat = (size <= 32 ? 0.0 : 6.0 * s)
        let baseRect = NSRect(x: baseInset, y: baseInset + yOffset, width: baseSize, height: baseSize)
        let baseRadius: CGFloat = baseSize * 0.224 // Matches Apple/Fluent squircle curvature ratio

        // 1. Windows 11 Fluent Elevation Shadow
        if size >= 32 {
            ctx.saveGState()
            let shadow = NSShadow()
            shadow.shadowColor = NSColor.black.withAlphaComponent(size >= 128 ? 0.32 : 0.42)
            shadow.shadowOffset = NSSize(width: 0, height: -10.0 * s)
            shadow.shadowBlurRadius = (size >= 128 ? 22.0 : 6.0) * s
            shadow.set()

            let squirclePath = NSBezierPath(roundedRect: baseRect, xRadius: baseRadius, yRadius: baseRadius)
            NSColor(white: 0.08, alpha: 1.0).setFill()
            squirclePath.fill()
            ctx.restoreGState()
        }

        let squirclePath = NSBezierPath(roundedRect: baseRect, xRadius: baseRadius, yRadius: baseRadius)

        // 2. Official Activity Monitor / Terminal Metallic Chassis Gradient
        ctx.saveGState()
        squirclePath.addClip()

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let gradColors = [
            NSColor(red: 0.24, green: 0.24, blue: 0.26, alpha: 1.0).cgColor,
            NSColor(red: 0.14, green: 0.14, blue: 0.15, alpha: 1.0).cgColor,
            NSColor(red: 0.07, green: 0.07, blue: 0.08, alpha: 1.0).cgColor
        ] as CFArray
        if let grad = CGGradient(colorsSpace: colorSpace, colors: gradColors, locations: [0.0, 0.48, 1.0]) {
            ctx.drawLinearGradient(
                grad,
                start: CGPoint(x: baseRect.midX, y: baseRect.maxY),
                end: CGPoint(x: baseRect.midX, y: baseRect.minY),
                options: []
            )
        }

        // 3. Technical Matrix Grid (Activity Monitor telemetry mesh)
        // Adaptive Level-of-Detail (LOD):
        // >= 64px: Full 6x6 grid
        // 32-48px: Simplified 4x4 subtle grid
        // <= 24px: Omit grid lines to prevent blurry noise at small scales
        if size >= 48 {
            let cols = (size >= 64 ? 6 : 4)
            let rows = cols
            let stepX = baseRect.width / CGFloat(cols)
            let stepY = baseRect.height / CGFloat(rows)
            let gridAlpha: CGFloat = (size >= 128 ? 0.38 : 0.24)
            NSColor(white: 0.28, alpha: gridAlpha).setStroke()

            for i in 1..<cols {
                let x = baseRect.minX + CGFloat(i) * stepX
                let p = NSBezierPath()
                p.lineWidth = max(1.0, 1.0 * s)
                p.move(to: NSPoint(x: x, y: baseRect.minY))
                p.line(to: NSPoint(x: x, y: baseRect.maxY))
                p.stroke()
            }
            for j in 1..<rows {
                let y = baseRect.minY + CGFloat(j) * stepY
                let p = NSBezierPath()
                p.lineWidth = max(1.0, 1.0 * s)
                p.move(to: NSPoint(x: baseRect.minX, y: y))
                p.line(to: NSPoint(x: baseRect.maxX, y: y))
                p.stroke()
            }
        }

        // 4. Specular Top Rim Highlight (Windows Fluent lighting)
        if size >= 32 {
            let rimPath = CGPath(
                roundedRect: baseRect.insetBy(dx: 1.0 * s, dy: 1.0 * s),
                cornerWidth: baseRadius - 1.0 * s,
                cornerHeight: baseRadius - 1.0 * s,
                transform: nil
            )
            let strokedRim = rimPath.copy(
                strokingWithWidth: max(1.0, 1.5 * s),
                lineCap: .round,
                lineJoin: .round,
                miterLimit: 10.0
            )
            ctx.addPath(strokedRim)
            ctx.clip()

            let rimColors = [
                NSColor.white.withAlphaComponent(0.26).cgColor,
                NSColor.white.withAlphaComponent(0.04).cgColor
            ] as CFArray
            if let rimGrad = CGGradient(colorsSpace: colorSpace, colors: rimColors, locations: [0.0, 1.0]) {
                ctx.drawLinearGradient(
                    rimGrad,
                    start: CGPoint(x: baseRect.midX, y: baseRect.maxY),
                    end: CGPoint(x: baseRect.midX, y: baseRect.minY),
                    options: []
                )
            }
        }
        ctx.restoreGState()

        // 5. Embossed Relief X Glyph
        let centerX = baseRect.midX
        let centerY = baseRect.midY
        
        // Multi-resolution calibrated glyph proportions
        let span: CGFloat = (size <= 16 ? 560.0 : (size <= 24 ? 500.0 : (size <= 32 ? 430.0 : 320.0))) * s
        let half = span / 2.0
        let strokeW: CGFloat = (size <= 16 ? 140.0 : (size <= 24 ? 120.0 : (size <= 32 ? 95.0 : 70.0))) * s

        let cgPath = CGMutablePath()
        cgPath.move(to: CGPoint(x: centerX - half, y: centerY - half))
        cgPath.addLine(to: CGPoint(x: centerX + half, y: centerY + half))
        cgPath.move(to: CGPoint(x: centerX - half, y: centerY + half))
        cgPath.addLine(to: CGPoint(x: centerX + half, y: centerY - half))

        let strokedGlyph = cgPath.copy(
            strokingWithWidth: strokeW,
            lineCap: .round,
            lineJoin: .round,
            miterLimit: 10.0
        )

        // Drop shadow for glyph
        if size >= 32 {
            ctx.saveGState()
            let glyphShadow = NSShadow()
            glyphShadow.shadowColor = NSColor.black.withAlphaComponent(0.55)
            glyphShadow.shadowOffset = NSSize(width: 0, height: -5.0 * s)
            glyphShadow.shadowBlurRadius = (size >= 128 ? 12.0 : 3.0) * s
            glyphShadow.set()
            ctx.addPath(strokedGlyph)
            ctx.setFillColor(NSColor.black.withAlphaComponent(0.55).cgColor)
            ctx.fillPath()
            ctx.restoreGState()
        }

        // 3D Keycap Micro-Relief Bevel
        if size >= 32 {
            ctx.saveGState()
            ctx.translateBy(x: 0, y: -2.0 * s)
            ctx.addPath(strokedGlyph)
            ctx.setFillColor(NSColor(red: 0.48, green: 0.49, blue: 0.51, alpha: 1.0).cgColor)
            ctx.fillPath()
            ctx.restoreGState()
        }

        // Crisp White Face
        ctx.saveGState()
        ctx.addPath(strokedGlyph)
        ctx.clip()

        let faceColors = [
            NSColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0).cgColor,
            NSColor(red: 0.94, green: 0.95, blue: 0.96, alpha: 1.0).cgColor
        ] as CFArray
        if let faceGrad = CGGradient(colorsSpace: colorSpace, colors: faceColors, locations: [0.0, 1.0]) {
            ctx.drawLinearGradient(
                faceGrad,
                start: CGPoint(x: centerX, y: centerY + half + strokeW / 2),
                end: CGPoint(x: centerX, y: centerY - half - strokeW / 2),
                options: []
            )
        }
        ctx.restoreGState()

        return true
    }
}

let args = CommandLine.arguments
let outputDir = args.count > 1 ? args[1] : "assets/raw_icons"

try? FileManager.default.createDirectory(atPath: outputDir, withIntermediateDirectories: true)

let allSizes: [CGFloat] = [1024, 512, 256, 128, 64, 48, 32, 24, 16]

for sz in allSizes {
    let img = renderWinIcon(size: sz)
    let rep = NSBitmapImageRep(data: img.tiffRepresentation!)!
    let pngData = rep.representation(using: .png, properties: [:])!
    let filePath = "\(outputDir)/icon_\(Int(sz)).png"
    try! pngData.write(to: URL(fileURLWithPath: filePath))
    print("[INFO] Rendered icon layer: \(filePath) (\(Int(sz))x\(Int(sz)))")
}

print("[SUCCESS] All icon layers successfully generated in \(outputDir)")
