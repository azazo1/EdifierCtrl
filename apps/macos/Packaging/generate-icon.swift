#!/usr/bin/env swift
import AppKit
import Darwin
import Foundation
import os

// 用法: swift generate-icon.swift <输出路径/AppIcon.icns>
// 纯离屏绘图, 不创建窗口, 同时保留 1024 PNG 和完整 iconset 供打包使用.

private enum IconFailure: LocalizedError {
    case arguments
    case outputExtension
    case bitmap(Int)
    case encoding
    case gradient
    case iconutil(Int32)

    var errorDescription: String? {
        switch self {
        case .arguments:
            return "需要一个输出 .icns 路径. 用法: swift generate-icon.swift <输出路径/AppIcon.icns>"
        case .outputExtension:
            return "输出文件必须使用 .icns 扩展名."
        case let .bitmap(size):
            return "无法创建 \(size)x\(size) 离屏位图."
        case .encoding:
            return "图标无法编码为 PNG."
        case .gradient:
            return "无法创建图标渐变."
        case let .iconutil(status):
            return "iconutil 生成失败, 退出码 \(status). PNG 和 iconset 已保留."
        }
    }
}

private enum IconLog {
    private static let logger = Logger(subsystem: "EdifierCtrl.packaging", category: "icon")

    static func info(_ message: String) {
        logger.info("\(message, privacy: .public)")
        FileHandle.standardError.write(Data((message + "\n").utf8))
    }

    static func error(_ message: String) {
        logger.error("\(message, privacy: .public)")
        FileHandle.standardError.write(Data(("错误: " + message + "\n").utf8))
    }
}

private enum IconGenerator {
    private static let masterSize = 1024

    static func run() throws {
        let arguments = Array(CommandLine.arguments.dropFirst())
        if arguments == ["--help"] || arguments == ["-h"] {
            let usage = "用法: swift generate-icon.swift <输出路径/AppIcon.icns>\n输出同名 .png, .iconset 和 .icns.\n"
            FileHandle.standardOutput.write(Data(usage.utf8))
            return
        }
        guard arguments.count == 1, let argument = arguments.first, !argument.isEmpty else {
            throw IconFailure.arguments
        }
        let output = URL(fileURLWithPath: (argument as NSString).expandingTildeInPath).standardizedFileURL
        guard output.pathExtension.lowercased() == "icns" else { throw IconFailure.outputExtension }
        let stem = output.deletingPathExtension()
        let png = stem.appendingPathExtension("png")
        let iconset = stem.appendingPathExtension("iconset")
        try FileManager.default.createDirectory(at: output.deletingLastPathComponent(), withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

        IconLog.info("绘制 1024x1024 EdifierCtrl 图标.")
        let master = try renderMaster()
        try writePNG(master, to: png)
        guard let image = master.cgImage else { throw IconFailure.encoding }

        // 每种逻辑尺寸都包含 1x 和 2x, 相同像素尺寸也保留各自标准文件名.
        let logicalSizes = [16, 32, 128, 256, 512]
        for size in logicalSizes {
            for scale in [1, 2] {
                let pixels = size * scale
                let filename = "icon_\(size)x\(size)" + (scale == 2 ? "@2x" : "") + ".png"
                let bitmap: NSBitmapImageRep
                if pixels == masterSize { bitmap = master }
                else { bitmap = try resize(image, pixels: pixels) }
                try writePNG(bitmap, to: iconset.appendingPathComponent(filename))
            }
        }
        IconLog.info("已生成完整 iconset, 正在打包 icns.")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
        process.arguments = ["-c", "icns", "-o", output.path, iconset.path]
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.standardOutput
        process.standardError = FileHandle.standardError
        try process.run()
        process.waitUntilExit()
        guard process.terminationReason == .exit, process.terminationStatus == 0 else {
            throw IconFailure.iconutil(process.terminationStatus)
        }
        IconLog.info("图标已生成: \(output.path)")
    }

    private static func bitmap(pixels: Int) throws -> NSBitmapImageRep {
        guard let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: pixels,
            pixelsHigh: pixels,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ) else { throw IconFailure.bitmap(pixels) }
        bitmap.size = NSSize(width: CGFloat(pixels), height: CGFloat(pixels))
        return bitmap
    }

    private static func renderMaster() throws -> NSBitmapImageRep {
        let bitmap = try bitmap(pixels: masterSize)
        guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
            throw IconFailure.bitmap(masterSize)
        }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        defer { NSGraphicsContext.restoreGraphicsState() }
        context.cgContext.clear(CGRect(x: 0, y: 0, width: CGFloat(masterSize), height: CGFloat(masterSize)))
        context.cgContext.setShouldAntialias(true)
        context.imageInterpolation = .high

        let face = NSBezierPath(roundedRect: NSRect(x: 92, y: 92, width: 840, height: 840), xRadius: 190, yRadius: 190)
        drawShadow(of: face, color: rgb(0.025, 0.09, 0.25, alpha: 0.30), blur: 24, offset: NSSize(width: 0, height: -12))
        let background = try gradient([
            rgb(0.045, 0.17, 0.51),
            rgb(0.12, 0.39, 0.85),
            rgb(0.23, 0.69, 0.96),
        ])
        background.draw(in: face, angle: 62)

        // 上方的柔光只留在圆角底内, 为缩小后的 Dock 图标保留清晰轮廓.
        NSGraphicsContext.saveGraphicsState()
        face.addClip()
        rgb(1, 1, 1, alpha: 0.055).setFill()
        NSBezierPath(ovalIn: NSRect(x: 230, y: 560, width: 850, height: 590)).fill()
        NSGraphicsContext.restoreGraphicsState()
        let rim = NSBezierPath(roundedRect: NSRect(x: 94, y: 94, width: 836, height: 836), xRadius: 188, yRadius: 188)
        rim.lineWidth = 2
        rgb(1, 1, 1, alpha: 0.20).setStroke()
        rim.stroke()

        // 封闭的头梁轮廓保持恒定厚度, 大小图标共用同一份几何.
        let band = NSBezierPath()
        band.move(to: NSPoint(x: 294, y: 485))
        band.line(to: NSPoint(x: 294, y: 567))
        band.curve(to: NSPoint(x: 512, y: 792), controlPoint1: NSPoint(x: 294, y: 691), controlPoint2: NSPoint(x: 391, y: 792))
        band.curve(to: NSPoint(x: 730, y: 567), controlPoint1: NSPoint(x: 633, y: 792), controlPoint2: NSPoint(x: 730, y: 691))
        band.line(to: NSPoint(x: 730, y: 485))
        band.line(to: NSPoint(x: 674, y: 485))
        band.line(to: NSPoint(x: 674, y: 567))
        band.curve(to: NSPoint(x: 512, y: 736), controlPoint1: NSPoint(x: 674, y: 660), controlPoint2: NSPoint(x: 602, y: 736))
        band.curve(to: NSPoint(x: 350, y: 567), controlPoint1: NSPoint(x: 422, y: 736), controlPoint2: NSPoint(x: 350, y: 660))
        band.line(to: NSPoint(x: 350, y: 485))
        band.close()

        let porcelain = try gradient([rgb(0.83, 0.94, 1), rgb(1, 1, 1)])
        drawShadow(of: band, color: rgb(0.025, 0.10, 0.34, alpha: 0.24), blur: 16, offset: NSSize(width: 0, height: -10))
        porcelain.draw(in: band, angle: 90)

        let leftCup = NSBezierPath(roundedRect: NSRect(x: 258, y: 330, width: 132, height: 240), xRadius: 58, yRadius: 58)
        let rightCup = NSBezierPath(roundedRect: NSRect(x: 634, y: 330, width: 132, height: 240), xRadius: 58, yRadius: 58)
        for cup in [leftCup, rightCup] {
            drawShadow(of: cup, color: rgb(0.025, 0.10, 0.34, alpha: 0.22), blur: 16, offset: NSSize(width: 0, height: -10))
            porcelain.draw(in: cup, angle: 90)
        }

        // 耳罩内缘的小幅明暗变化提供体积感, 不增加小尺寸图标的视觉噪声.
        rgb(0.19, 0.42, 0.70, alpha: 0.13).setFill()
        for x in [362.0, 650.0] {
            NSBezierPath(roundedRect: NSRect(x: x, y: 365, width: 12, height: 170), xRadius: 6, yRadius: 6).fill()
        }
        rgb(1, 1, 1, alpha: 0.55).setFill()
        for x in [277.0, 735.0] {
            NSBezierPath(roundedRect: NSRect(x: x, y: 383, width: 9, height: 124), xRadius: 4.5, yRadius: 4.5).fill()
        }
        context.cgContext.flush()
        return bitmap
    }

    private static func resize(_ image: CGImage, pixels: Int) throws -> NSBitmapImageRep {
        let bitmap = try bitmap(pixels: pixels)
        guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else { throw IconFailure.bitmap(pixels) }
        context.cgContext.clear(CGRect(x: 0, y: 0, width: CGFloat(pixels), height: CGFloat(pixels)))
        context.cgContext.interpolationQuality = .high
        context.cgContext.draw(image, in: CGRect(x: 0, y: 0, width: CGFloat(pixels), height: CGFloat(pixels)))
        context.cgContext.flush()
        return bitmap
    }

    private static func writePNG(_ bitmap: NSBitmapImageRep, to output: URL) throws {
        guard let data = bitmap.representation(using: .png, properties: [:]) else { throw IconFailure.encoding }
        try data.write(to: output, options: .atomic)
    }

    private static func gradient(_ colors: [NSColor]) throws -> NSGradient {
        guard let gradient = NSGradient(colors: colors) else { throw IconFailure.gradient }
        return gradient
    }

    private static func rgb(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat, alpha: CGFloat = 1) -> NSColor {
        NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
    }

    private static func drawShadow(of path: NSBezierPath, color: NSColor, blur: CGFloat, offset: NSSize) {
        NSGraphicsContext.saveGraphicsState()
        defer { NSGraphicsContext.restoreGraphicsState() }
        let shadow = NSShadow()
        shadow.shadowColor = color
        shadow.shadowBlurRadius = blur
        shadow.shadowOffset = offset
        shadow.set()
        NSColor.white.setFill()
        path.fill()
    }
}

do {
    try IconGenerator.run()
} catch {
    IconLog.error(error.localizedDescription)
    exit(EXIT_FAILURE)
}
