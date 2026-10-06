import AppKit
import CoreGraphics
import Darwin
import Foundation
import ImageIO
@preconcurrency import ScreenCaptureKit
import UniformTypeIdentifiers

struct CaptureBatchResult {
    var files: [URL] = []
    var successfulTaskIDs: [String] = []
    var skipped = false
}

@available(macOS 14.0, *)
@MainActor
final class CaptureCoordinator {
    private let directory: URL
    private let logger: DiagnosticLogger

    private(set) var isBusy = false
    var isSessionAvailable: () -> Bool = { true }
    var onError: ((String) -> Void)?

    init(directory: URL, logger: DiagnosticLogger) {
        self.directory = directory.standardizedFileURL
        self.logger = logger
    }

    func capture(_ tasks: [ScreenshotTask], reason: String) async -> CaptureBatchResult {
        guard !tasks.isEmpty else { return CaptureBatchResult() }

        guard !isBusy else {
            for task in tasks {
                logger.info(
                    "TASK_SKIPPED_BUSY",
                    "Capture was skipped because another batch is active.",
                    context: .forTask(task)
                )
            }
            return CaptureBatchResult(skipped: true)
        }

        isBusy = true
        defer { isBusy = false }

        let batchID = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let started = ContinuousClock.now
        let timestamp = Date()
        var files: [URL] = []
        var successfulTaskIDs: [String] = []

        logger.info(
            "BATCH_START",
            "Screenshot batch started.",
            context: LogContext(
                batchId: batchID,
                values: ["reason": reason, "taskCount": String(tasks.count)]
            )
        )

        guard CGPreflightScreenCaptureAccess() else {
            onError?("Screen Recording permission is required. Use the app's permission action, then enable access in System Settings.")
            logger.error(
                "SCREEN_RECORDING_PERMISSION_REQUIRED",
                "Screen Recording permission is required. Capture did not request permission.",
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        guard isSessionAvailable() else {
            logger.info(
                "BATCH_SKIPPED_SESSION_UNAVAILABLE",
                "Capture was skipped because the user session is unavailable.",
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        let content: SCShareableContent
        do {
            content = try await SCShareableContent.current
        } catch {
            onError?("Available displays could not be loaded: \(error.localizedDescription)")
            logger.error(
                "SHAREABLE_CONTENT_FAILED",
                "Available displays could not be loaded.",
                error: error,
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        guard isSessionAvailable() else {
            logger.info(
                "BATCH_SKIPPED_SESSION_UNAVAILABLE",
                "Capture stopped because the user session became unavailable.",
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        let displays = content.displays.sorted {
            if $0.frame.minX == $1.frame.minX {
                return $0.frame.minY < $1.frame.minY
            }
            return $0.frame.minX < $1.frame.minX
        }
        guard !displays.isEmpty else {
            onError?("No displays are available for capture.")
            logger.error(
                "NO_DISPLAYS_AVAILABLE",
                "No displays are available for capture.",
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        let profileGroups: [[ScreenshotTask]]
        do {
            profileGroups = try groupedProfiles(tasks)
        } catch {
            onError?("Capture output folders could not be resolved: \(error.localizedDescription)")
            logger.error(
                "CAPTURE_PROFILE_CONFIGURATION_FAILED",
                "Capture output folders could not be resolved.",
                error: error,
                context: LogContext(batchId: batchID)
            )
            logBatchComplete(batchID: batchID, started: started, filesWritten: 0)
            return CaptureBatchResult()
        }

        for group in profileGroups {
            let task = group[0]
            var profileFiles: [URL] = []

            for (offset, display) in displays.enumerated() {
                guard isSessionAvailable() else {
                    logger.info(
                        "MONITOR_CAPTURE_SKIPPED_SESSION_UNAVAILABLE",
                        "Display capture was skipped because the user session is unavailable.",
                        context: context(for: task, batchID: batchID, display: displayName(display))
                    )
                    continue
                }

                let displayLabel = displayName(display)
                let displayStarted = ContinuousClock.now
                do {
                    let image = try await capture(display: display, settings: task.capture)
                    guard isSessionAvailable() else {
                        logger.info(
                            "MONITOR_CAPTURE_SKIPPED_SESSION_UNAVAILABLE",
                            "The captured image was discarded because the user session became unavailable.",
                            context: context(for: task, batchID: batchID, display: displayLabel)
                        )
                        continue
                    }

                    let destination = try write(
                        image,
                        task: task,
                        timestamp: timestamp,
                        displayName: displayLabel,
                        displayIndex: offset + 1
                    )
                    profileFiles.append(destination)
                    logger.debug(
                        "MONITOR_CAPTURE_SUCCESS",
                        "Display screenshot was written.",
                        context: context(
                            for: task,
                            batchID: batchID,
                            display: displayLabel,
                            filePath: destination.path,
                            duration: displayStarted.duration(to: .now)
                        )
                    )
                } catch {
                    onError?("Could not capture \(displayLabel) for \(task.name): \(error.localizedDescription)")
                    logger.error(
                        "MONITOR_CAPTURE_FAILED",
                        "A display could not be captured.",
                        error: error,
                        context: context(
                            for: task,
                            batchID: batchID,
                            display: displayLabel,
                            duration: displayStarted.duration(to: .now)
                        )
                    )
                }
            }

            if profileFiles.isEmpty {
                logger.error(
                    "CAPTURE_PROFILE_FAILED",
                    "A capture profile produced no screenshots; other profiles will continue.",
                    context: context(for: task, batchID: batchID)
                )
            } else {
                files.append(contentsOf: profileFiles)
                successfulTaskIDs.append(contentsOf: group.map(\.id))
            }
        }

        logBatchComplete(batchID: batchID, started: started, filesWritten: files.count)
        return CaptureBatchResult(files: files, successfulTaskIDs: successfulTaskIDs)
    }

    private func capture(display: SCDisplay, settings: CaptureSettings) async throws -> CGImage {
        let configuration = SCStreamConfiguration()
        configuration.width = CGDisplayPixelsWide(display.displayID)
        configuration.height = CGDisplayPixelsHigh(display.displayID)
        configuration.showsCursor = settings.includeCursor

        let filter = SCContentFilter(display: display, excludingWindows: [])
        if #available(macOS 14.2, *) {
            filter.includeMenuBar = true
        }
        return try await SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        )
    }

    private func write(
        _ image: CGImage,
        task: ScreenshotTask,
        timestamp: Date,
        displayName: String,
        displayIndex: Int
    ) throws -> URL {
        let output = try resolveDirectory(task.capture.outputFolder)
        let dateDirectory = output.appendingPathComponent(Self.dateFormatter.string(from: timestamp), isDirectory: true)
        try FileManager.default.createDirectory(at: dateDirectory, withIntermediateDirectories: true)

        let stem = FileNameTemplate.expand(
            task.capture.fileNameTemplate,
            task: task,
            localTimestamp: timestamp,
            displayName: displayName,
            displayIndex: displayIndex
        )
        let encoding = try Encoding(settings: task.capture)
        let temporary = dateDirectory.appendingPathComponent(".capture-\(UUID().uuidString).tmp")
        try encode(image, to: temporary, encoding: encoding)
        defer { try? FileManager.default.removeItem(at: temporary) }

        for collision in 0..<100_000 {
            let suffix = collision == 0 ? "" : String(format: "_%03d", collision)
            let destination = dateDirectory.appendingPathComponent(stem + suffix + encoding.fileExtension)

            if Darwin.link(temporary.path, destination.path) == 0 {
                if collision > 0 {
                    logger.debug(
                        "FILENAME_COLLISION",
                        "A filename collision suffix was applied.",
                        context: LogContext(
                            taskId: task.id,
                            taskName: task.name,
                            filePath: destination.path
                        )
                    )
                }
                return destination
            }

            let linkError = errno
            if linkError == EEXIST { continue }
            throw POSIXError(POSIXErrorCode(rawValue: linkError) ?? .EIO)
        }

        throw CaptureError.noAvailableFilename
    }

    private func encode(_ image: CGImage, to url: URL, encoding: Encoding) throws {
        guard let destination = CGImageDestinationCreateWithURL(
            url as CFURL,
            encoding.type.identifier as CFString,
            1,
            nil
        ) else {
            throw CaptureError.cannotCreateImageDestination
        }

        let properties: CFDictionary?
        switch encoding {
        case .png:
            properties = nil
        case let .jpeg(quality):
            properties = [
                kCGImageDestinationLossyCompressionQuality: Double(quality) / 100.0
            ] as CFDictionary
        }

        CGImageDestinationAddImage(destination, image, properties)
        guard CGImageDestinationFinalize(destination) else {
            throw CaptureError.cannotEncodeImage
        }
    }

    private func groupedProfiles(_ tasks: [ScreenshotTask]) throws -> [[ScreenshotTask]] {
        var groups: [ProfileKey: [ScreenshotTask]] = [:]
        var order: [ProfileKey] = []

        for task in tasks {
            let format = task.capture.imageFormat.lowercased()
            let key = ProfileKey(
                outputDirectory: try resolveDirectoryForComparison(task.capture.outputFolder),
                template: FileNameTemplate.resolveTaskSpecificTemplate(task.capture.fileNameTemplate, task: task),
                format: format,
                jpegQuality: format == "png" ? 0 : min(100, max(50, task.capture.jpegQuality)),
                includeCursor: task.capture.includeCursor
            )
            if groups[key] == nil { order.append(key) }
            groups[key, default: []].append(task)
        }

        return order.compactMap { groups[$0] }
    }

    private func resolveDirectory(_ path: String) throws -> URL {
        try SettingsValidator.resolveDirectory(path, relativeTo: directory)
    }

    private func resolveDirectoryForComparison(_ path: String) throws -> String {
        try resolveDirectory(path).resolvingSymlinksInPath().path
    }

    private func displayName(_ display: SCDisplay) -> String {
        NSScreen.screens.first {
            ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value == display.displayID
        }.map { "\($0.localizedName)-\(display.displayID)" } ?? "Display-\(display.displayID)"
    }

    private func context(
        for task: ScreenshotTask,
        batchID: String,
        display: String? = nil,
        filePath: String? = nil,
        duration: Duration? = nil
    ) -> LogContext {
        LogContext(
            taskId: task.id,
            taskName: task.name,
            batchId: batchID,
            display: display,
            filePath: filePath,
            durationMs: duration.map(Self.milliseconds)
        )
    }

    private func logBatchComplete(batchID: String, started: ContinuousClock.Instant, filesWritten: Int) {
        logger.info(
            "BATCH_COMPLETE",
            "Screenshot batch completed.",
            context: LogContext(
                batchId: batchID,
                durationMs: Self.milliseconds(started.duration(to: .now)),
                values: ["filesWritten": String(filesWritten)]
            ),
            flush: true
        )
    }

    private static func milliseconds(_ duration: Duration) -> Int {
        let components = duration.components
        return Int(components.seconds * 1_000 + Int64(components.attoseconds / 1_000_000_000_000_000))
    }

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = .autoupdatingCurrent
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()
}

@available(macOS 14.0, *)
private extension CaptureCoordinator {
    struct ProfileKey: Hashable {
        let outputDirectory: String
        let template: String
        let format: String
        let jpegQuality: Int
        let includeCursor: Bool
    }

    enum Encoding {
        case png
        case jpeg(quality: Int)

        init(settings: CaptureSettings) throws {
            switch settings.imageFormat.lowercased() {
            case "png":
                self = .png
            case "jpeg", "jpg":
                self = .jpeg(quality: min(100, max(50, settings.jpegQuality)))
            default:
                throw CaptureError.unsupportedImageFormat(settings.imageFormat)
            }
        }

        var type: UTType {
            switch self {
            case .png: .png
            case .jpeg: .jpeg
            }
        }

        var fileExtension: String {
            switch self {
            case .png: ".png"
            case .jpeg: ".jpg"
            }
        }
    }

    enum CaptureError: LocalizedError {
        case unsupportedImageFormat(String)
        case cannotCreateImageDestination
        case cannotEncodeImage
        case noAvailableFilename

        var errorDescription: String? {
            switch self {
            case let .unsupportedImageFormat(format):
                "Unsupported image format: \(format)"
            case .cannotCreateImageDestination:
                "The image encoder could not create its destination."
            case .cannotEncodeImage:
                "The image encoder could not finalize the screenshot."
            case .noAvailableFilename:
                "No unique screenshot filename was available."
            }
        }
    }
}
