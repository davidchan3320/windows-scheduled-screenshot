import Foundation
import Darwin

private let allWeekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]

struct AppSettings: Codable, Equatable {
    var schemaVersion: Int
    var paused: Bool
    var logging: LoggingSettings
    var tasks: [ScreenshotTask]

    init(schemaVersion: Int = 1, paused: Bool = false,
         logging: LoggingSettings = LoggingSettings(), tasks: [ScreenshotTask] = []) {
        self.schemaVersion = schemaVersion
        self.paused = paused
        self.logging = logging
        self.tasks = tasks
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try values.decodeIfPresent(Int.self, forKey: .schemaVersion) ?? 1
        paused = try values.decodeIfPresent(Bool.self, forKey: .paused) ?? false
        logging = values.contains(.logging) ? try values.decode(LoggingSettings.self, forKey: .logging) : LoggingSettings()
        tasks = values.contains(.tasks) ? try values.decode([ScreenshotTask].self, forKey: .tasks) : []
    }
}

struct LoggingSettings: Codable, Equatable {
    var level: String
    var directory: String
    var maxFileSizeMb: Int
    var retainedFiles: Int

    init(level: String = "info", directory: String = "logs", maxFileSizeMb: Int = 5, retainedFiles: Int = 5) {
        self.level = level
        self.directory = directory
        self.maxFileSizeMb = maxFileSizeMb
        self.retainedFiles = retainedFiles
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        level = try values.decodeIfPresent(String.self, forKey: .level) ?? "info"
        directory = try values.decodeIfPresent(String.self, forKey: .directory) ?? "logs"
        maxFileSizeMb = try values.decodeIfPresent(Int.self, forKey: .maxFileSizeMb) ?? 5
        retainedFiles = try values.decodeIfPresent(Int.self, forKey: .retainedFiles) ?? 5
    }
}

struct ScreenshotTask: Codable, Equatable, Identifiable {
    var id: String
    var name: String
    var enabled: Bool
    var capture: CaptureSettings
    var schedule: ScheduleSettings
    var stopCondition: StopConditionSettings

    init(id: String = UUID().uuidString.lowercased(), name: String = "New task", enabled: Bool = false,
         capture: CaptureSettings = CaptureSettings(), schedule: ScheduleSettings = ScheduleSettings(),
         stopCondition: StopConditionSettings = StopConditionSettings()) {
        self.id = id
        self.name = name
        self.enabled = enabled
        self.capture = capture
        self.schedule = schedule
        self.stopCondition = stopCondition
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(String.self, forKey: .id) ?? UUID().uuidString.lowercased()
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? "New task"
        enabled = try values.decodeIfPresent(Bool.self, forKey: .enabled) ?? false
        capture = values.contains(.capture) ? try values.decode(CaptureSettings.self, forKey: .capture) : CaptureSettings()
        schedule = values.contains(.schedule) ? try values.decode(ScheduleSettings.self, forKey: .schedule) : ScheduleSettings()
        stopCondition = values.contains(.stopCondition) ? try values.decode(StopConditionSettings.self, forKey: .stopCondition) : StopConditionSettings()
    }
}

struct CaptureSettings: Codable, Equatable {
    var outputFolder: String
    var fileNameTemplate: String
    var imageFormat: String
    var jpegQuality: Int
    var includeCursor: Bool

    init(outputFolder: String = "~/Pictures/Scheduled Screenshots",
         fileNameTemplate: String = "{timestamp}_{display}", imageFormat: String = "jpeg",
         jpegQuality: Int = 85, includeCursor: Bool = false) {
        self.outputFolder = outputFolder
        self.fileNameTemplate = fileNameTemplate
        self.imageFormat = imageFormat
        self.jpegQuality = jpegQuality
        self.includeCursor = includeCursor
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        outputFolder = try values.decodeIfPresent(String.self, forKey: .outputFolder) ?? "~/Pictures/Scheduled Screenshots"
        fileNameTemplate = try values.decodeIfPresent(String.self, forKey: .fileNameTemplate) ?? "{timestamp}_{display}"
        imageFormat = try values.decodeIfPresent(String.self, forKey: .imageFormat) ?? "jpeg"
        jpegQuality = try values.decodeIfPresent(Int.self, forKey: .jpegQuality) ?? 85
        includeCursor = try values.decodeIfPresent(Bool.self, forKey: .includeCursor) ?? false
    }
}

struct ScheduleSettings: Codable, Equatable {
    var type: String
    var intervalSeconds: Int
    var weekdays: [String]
    var activeHoursEnabled: Bool
    var activeStart: String
    var activeEnd: String
    var times: [String]

    init(type: String = "interval", intervalSeconds: Int = 60, weekdays: [String] = allWeekdays,
         activeHoursEnabled: Bool = false, activeStart: String = "09:00:00",
         activeEnd: String = "17:00:00", times: [String] = ["09:00:00"]) {
        self.type = type
        self.intervalSeconds = intervalSeconds
        self.weekdays = weekdays
        self.activeHoursEnabled = activeHoursEnabled
        self.activeStart = activeStart
        self.activeEnd = activeEnd
        self.times = times
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        type = try values.decodeIfPresent(String.self, forKey: .type) ?? "interval"
        intervalSeconds = try values.decodeIfPresent(Int.self, forKey: .intervalSeconds) ?? 60
        weekdays = try values.decodeIfPresent([String].self, forKey: .weekdays) ?? allWeekdays
        activeHoursEnabled = try values.decodeIfPresent(Bool.self, forKey: .activeHoursEnabled) ?? false
        activeStart = try values.decodeIfPresent(String.self, forKey: .activeStart) ?? "09:00:00"
        activeEnd = try values.decodeIfPresent(String.self, forKey: .activeEnd) ?? "17:00:00"
        times = try values.decodeIfPresent([String].self, forKey: .times) ?? ["09:00:00"]
    }
}

struct StopConditionSettings: Codable, Equatable {
    var mode: String
    var endAtLocal: String?
    var durationSeconds: Int?
    var captureCount: Int?

    init(mode: String = "none", endAtLocal: String? = nil, durationSeconds: Int? = nil, captureCount: Int? = nil) {
        self.mode = mode
        self.endAtLocal = endAtLocal
        self.durationSeconds = durationSeconds
        self.captureCount = captureCount
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        mode = try values.decodeIfPresent(String.self, forKey: .mode) ?? "none"
        endAtLocal = try values.decodeIfPresent(String.self, forKey: .endAtLocal)
        durationSeconds = try values.decodeIfPresent(Int.self, forKey: .durationSeconds)
        captureCount = try values.decodeIfPresent(Int.self, forKey: .captureCount)
    }
}

struct ValidationResult {
    var errors: [String] = []
    var warnings: [String] = []
    var isValid: Bool { errors.isEmpty }
}

enum SettingsValidator {
    private static let validDays = Set(allWeekdays)

    static func validate(_ settings: AppSettings, applicationDirectory: URL, verifyWritablePaths: Bool) -> ValidationResult {
        var result = ValidationResult()
        if settings.schemaVersion != 1 { result.errors.append("Only schemaVersion 1 is supported.") }
        validateLogging(settings.logging, applicationDirectory: applicationDirectory, result: &result)
        if settings.tasks.count > 100 { result.errors.append("No more than 100 tasks are allowed.") }

        var ids = Set<String>()
        var names = Set<String>()
        for (index, task) in settings.tasks.enumerated() {
            let prefix = "tasks[\(index)]"
            if UUID(uuidString: task.id) == nil {
                result.errors.append("\(prefix).id must be a UUID.")
            } else if !ids.insert(task.id.lowercased()).inserted {
                result.errors.append("\(prefix).id must be unique.")
            }
            let name = task.name.trimmingCharacters(in: .whitespacesAndNewlines)
            if name.isEmpty || name.count > 64 {
                result.errors.append("\(prefix).name must contain 1-64 characters.")
            } else if !names.insert(name.lowercased()).inserted {
                result.errors.append("\(prefix).name must be unique.")
            }
            validateCapture(task.capture, prefix: prefix, applicationDirectory: applicationDirectory,
                            verifyWritable: verifyWritablePaths && task.enabled, result: &result)
            validateSchedule(task.schedule, prefix: prefix, result: &result)
            validateStop(task.stopCondition, prefix: prefix, enabled: task.enabled, result: &result)
        }
        return result
    }

    static func resolveDirectory(_ value: String, relativeTo applicationDirectory: URL) throws -> URL {
        var expanded = NSString(string: value).expandingTildeInPath
        let expression = try NSRegularExpression(pattern: "%([A-Za-z_][A-Za-z0-9_]*)%")
        let source = expanded as NSString
        for match in expression.matches(in: expanded, range: NSRange(location: 0, length: source.length)).reversed() {
            let name = source.substring(with: match.range(at: 1))
            let replacement = name.caseInsensitiveCompare("USERPROFILE") == .orderedSame
                ? FileManager.default.homeDirectoryForCurrentUser.path
                : ProcessInfo.processInfo.environment[name]
            if let replacement {
                expanded = (expanded as NSString).replacingCharacters(in: match.range, with: replacement)
            }
        }
        expanded = expanded.replacingOccurrences(of: "\\", with: "/")
        if expanded.range(of: "^[A-Za-z]:/", options: .regularExpression) != nil {
            throw ConfigurationError.invalid("Windows drive-letter paths are not valid on macOS.")
        }
        let url: URL
        if NSString(string: expanded).isAbsolutePath {
            url = URL(fileURLWithPath: expanded, isDirectory: true)
        } else {
            url = applicationDirectory.appendingPathComponent(expanded, isDirectory: true)
        }
        return url.standardizedFileURL
    }

    static func parseClock(_ value: String) -> DateComponents? {
        let pieces = value.split(separator: ":", omittingEmptySubsequences: false)
        guard pieces.count == 3, pieces.allSatisfy({ $0.count == 2 }),
              let hour = Int(pieces[0]), let minute = Int(pieces[1]), let second = Int(pieces[2]),
              (0...23).contains(hour), (0...59).contains(minute), (0...59).contains(second) else { return nil }
        return DateComponents(hour: hour, minute: minute, second: second)
    }

    static func parseLocalDateTime(_ value: String?, calendar: Calendar = .autoupdatingCurrent) -> Date? {
        guard let value else { return nil }
        let pattern = try? NSRegularExpression(pattern: "^([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})$")
        let range = NSRange(value.startIndex..<value.endIndex, in: value)
        guard let match = pattern?.firstMatch(in: value, range: range) else { return nil }
        func number(_ index: Int) -> Int? {
            guard let swiftRange = Range(match.range(at: index), in: value) else { return nil }
            return Int(value[swiftRange])
        }
        guard let year = number(1), let month = number(2), let day = number(3),
              let hour = number(4), let minute = number(5), let second = number(6),
              (0...23).contains(hour), (0...59).contains(minute), (0...59).contains(second) else { return nil }
        var startComponents = DateComponents(year: year, month: month, day: day)
        startComponents.hour = 0
        guard let roughStart = calendar.date(from: startComponents),
              let start = calendar.date(byAdding: .second, value: -1, to: roughStart) else { return nil }
        let matching = DateComponents(hour: hour, minute: minute, second: second)
        guard let date = calendar.nextDate(after: start, matching: matching, matchingPolicy: .strict,
                                           repeatedTimePolicy: .last, direction: .forward) else { return nil }
        let resolved = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        guard resolved.year == year, resolved.month == month, resolved.day == day,
              resolved.hour == hour, resolved.minute == minute, resolved.second == second else { return nil }
        return date
    }

    private static func validateLogging(_ logging: LoggingSettings, applicationDirectory: URL,
                                        result: inout ValidationResult) {
        if !["error", "info", "debug"].contains(logging.level) {
            result.errors.append("logging.level must be error, info, or debug.")
        }
        if !(1...100).contains(logging.maxFileSizeMb) {
            result.errors.append("logging.maxFileSizeMb must be between 1 and 100.")
        }
        if !(1...20).contains(logging.retainedFiles) {
            result.errors.append("logging.retainedFiles must be between 1 and 20.")
        }
        let directory = logging.directory.trimmingCharacters(in: .whitespacesAndNewlines)
        let segments = directory.split(whereSeparator: { $0 == "/" || $0 == "\\" }).map(String.init)
        guard !directory.isEmpty, !NSString(string: directory).isAbsolutePath, !segments.contains("..") else {
            result.errors.append("logging.directory must be a relative child directory without parent traversal.")
            return
        }
        let base = applicationDirectory.standardizedFileURL.path
        let candidate = applicationDirectory.appendingPathComponent(directory, isDirectory: true).standardizedFileURL.path
        if candidate == base || !candidate.hasPrefix(base + "/") {
            result.errors.append("logging.directory must name a child of the application directory.")
        }
    }

    private static func validateCapture(_ capture: CaptureSettings, prefix: String, applicationDirectory: URL,
                                        verifyWritable: Bool, result: inout ValidationResult) {
        if !["jpeg", "png"].contains(capture.imageFormat) {
            result.errors.append("\(prefix).capture.imageFormat must be jpeg or png.")
        }
        if !(50...100).contains(capture.jpegQuality) {
            result.errors.append("\(prefix).capture.jpegQuality must be between 50 and 100.")
        }
        result.errors.append(contentsOf: FileNameTemplate.validate(capture.fileNameTemplate)
            .map { "\(prefix).capture.fileNameTemplate: \($0)" })
        guard !capture.outputFolder.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            result.errors.append("\(prefix).capture.outputFolder is required.")
            return
        }
        do {
            let output = try resolveDirectory(capture.outputFolder, relativeTo: applicationDirectory)
            let sample = FileNameTemplate.expand(capture.fileNameTemplate,
                task: ScreenshotTask(id: UUID().uuidString, name: "Sample task"),
                localTimestamp: Date(), displayName: "Display 1", displayIndex: 1)
            let ext = capture.imageFormat == "png" ? "png" : "jpg"
            let day = localDayFormatter.string(from: Date())
            if output.appendingPathComponent(day).appendingPathComponent("\(sample).\(ext)").path.count > 240 {
                result.errors.append("\(prefix).capture produces a path longer than 240 characters.")
            }
            if verifyWritable { try verifyWritableDirectory(output) }
        } catch {
            result.errors.append("\(prefix).capture.outputFolder is not writable: \(error.localizedDescription)")
        }
    }

    private static func validateSchedule(_ schedule: ScheduleSettings, prefix: String,
                                         result: inout ValidationResult) {
        if !["interval", "fixed", "fixedOnce"].contains(schedule.type) {
            result.errors.append("\(prefix).schedule.type must be interval, fixed, or fixedOnce.")
        }
        if schedule.weekdays.isEmpty || schedule.weekdays.contains(where: { !validDays.contains($0) })
            || Set(schedule.weekdays).count != schedule.weekdays.count {
            result.errors.append("\(prefix).schedule.weekdays must contain unique valid weekday tokens.")
        }
        if schedule.type == "interval" {
            if !(1...86_400).contains(schedule.intervalSeconds) {
                result.errors.append("\(prefix).schedule.intervalSeconds must be between 1 and 86400.")
            }
            if schedule.activeHoursEnabled {
                guard let start = parseClock(schedule.activeStart), let end = parseClock(schedule.activeEnd),
                      clockSeconds(start) < clockSeconds(end) else {
                    result.errors.append("\(prefix).schedule active hours must be valid same-day HH:mm:ss values with start before end.")
                    return
                }
            }
        } else if ["fixed", "fixedOnce"].contains(schedule.type) {
            if schedule.times.isEmpty {
                result.errors.append("\(prefix).schedule.times must contain at least one time.")
            } else {
                if schedule.times.contains(where: { parseClock($0) == nil }) {
                    result.errors.append("\(prefix).schedule.times must use HH:mm:ss.")
                }
                if Set(schedule.times).count != schedule.times.count {
                    result.errors.append("\(prefix).schedule.times must be unique.")
                }
            }
        }
    }

    private static func validateStop(_ stop: StopConditionSettings, prefix: String, enabled: Bool,
                                     result: inout ValidationResult) {
        if !["none", "at", "duration", "count"].contains(stop.mode) {
            result.errors.append("\(prefix).stopCondition.mode must be none, at, duration, or count.")
        }
        if stop.mode == "at" {
            guard let end = parseLocalDateTime(stop.endAtLocal), !enabled || end > Date() else {
                result.errors.append("\(prefix).stopCondition.endAtLocal must be a future yyyy-MM-ddTHH:mm:ss value for an enabled task.")
                return
            }
        }
        if stop.mode == "duration", !(1...31_536_000).contains(stop.durationSeconds ?? 0) {
            result.errors.append("\(prefix).stopCondition.durationSeconds must be between 1 and 31536000.")
        }
        if stop.mode == "count", !(1...2_147_483_647).contains(stop.captureCount ?? 0) {
            result.errors.append("\(prefix).stopCondition.captureCount must be between 1 and 2147483647.")
        }
    }

    private static func clockSeconds(_ value: DateComponents) -> Int {
        (value.hour ?? 0) * 3600 + (value.minute ?? 0) * 60 + (value.second ?? 0)
    }

    private static func verifyWritableDirectory(_ url: URL) throws {
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        let probe = url.appendingPathComponent(".scheduled-screenshot-write-test-\(UUID().uuidString).tmp")
        guard FileManager.default.createFile(atPath: probe.path, contents: Data()) else {
            throw CocoaError(.fileWriteNoPermission)
        }
        try FileManager.default.removeItem(at: probe)
    }

    private static let localDayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = .autoupdatingCurrent
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()
}

enum FileNameTemplate {
    private static let tokens = Set(["timestamp", "date", "time", "task", "taskid", "taskid8", "display", "displayindex"])
    private static let reserved = Set(["CON", "PRN", "AUX", "NUL"])
        .union((1...9).map { "COM\($0)" }).union((1...9).map { "LPT\($0)" })
    private static let tokenExpression = try! NSRegularExpression(pattern: "\\{([A-Za-z0-9]+)\\}")

    static func validate(_ template: String) -> [String] {
        var errors: [String] = []
        if template.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return ["Filename template is required."]
        }
        if template.count > 180 { errors.append("Filename template cannot exceed 180 characters.") }
        if template.rangeOfCharacter(from: CharacterSet(charactersIn: "\\/:<>\"|?*")) != nil {
            errors.append("Filename template contains an invalid filename character or path separator.")
        }
        let nsTemplate = template as NSString
        let matches = tokenExpression.matches(in: template, range: NSRange(location: 0, length: nsTemplate.length))
        for match in matches {
            let token = nsTemplate.substring(with: match.range(at: 1)).lowercased()
            if !tokens.contains(token) { errors.append("Unknown filename token: \(nsTemplate.substring(with: match.range)).") }
        }
        let stripped = tokenExpression.stringByReplacingMatches(in: template,
            range: NSRange(location: 0, length: nsTemplate.length), withTemplate: "")
        if stripped.contains("{") || stripped.contains("}") { errors.append("Filename template contains a malformed token.") }
        let lower = template.lowercased()
        if !lower.contains("{display}") && !lower.contains("{displayindex}") {
            errors.append("Filename template must include {display} or {displayIndex}.")
        }
        return errors
    }

    static func resolveTaskSpecificTemplate(_ template: String, task: ScreenshotTask) -> String {
        let id = UUID(uuidString: task.id)?.uuidString.lowercased() ?? UUID.zeroString
        return replaceTokens(in: template, values: [
            "task": sanitize(task.name), "taskid": id,
            "taskid8": String(id.replacingOccurrences(of: "-", with: "").prefix(8))
        ])
    }

    static func expand(_ template: String, task: ScreenshotTask, localTimestamp: Date,
                       displayName: String, displayIndex: Int) -> String {
        let id = UUID(uuidString: task.id)?.uuidString.lowercased() ?? UUID.zeroString
        let values = [
            "timestamp": timestampFormatter.string(from: localTimestamp),
            "date": dateFormatter.string(from: localTimestamp),
            "time": timeFormatter.string(from: localTimestamp),
            "task": sanitize(task.name),
            "taskid": id,
            "taskid8": String(id.replacingOccurrences(of: "-", with: "").prefix(8)),
            "display": sanitize(normalizeDisplayName(displayName)),
            "displayindex": String(displayIndex)
        ]
        var result = replaceTokens(in: template, values: values)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        while result.last == "." || result.last == " " { result.removeLast() }
        if result.isEmpty { result = "screenshot" }
        let stem = URL(fileURLWithPath: result).deletingPathExtension().lastPathComponent.uppercased()
        if reserved.contains(stem) { result = "_" + result }
        return result
    }

    static func sanitize(_ value: String) -> String {
        let invalid = CharacterSet(charactersIn: "\\/:<>\"|?*").union(.controlCharacters)
        let scalars = value.unicodeScalars.map { invalid.contains($0) ? "_" : String($0) }
        var result = scalars.joined().trimmingCharacters(in: .whitespacesAndNewlines)
        while result.last == "." || result.last == " " { result.removeLast() }
        return result.isEmpty ? "unnamed" : result
    }

    private static func normalizeDisplayName(_ value: String) -> String {
        value.hasPrefix("\\\\.\\") ? String(value.dropFirst(4)) : value
    }

    private static func replaceTokens(in template: String, values: [String: String]) -> String {
        let source = template as NSString
        var result = template
        for match in tokenExpression.matches(in: template,
            range: NSRange(location: 0, length: source.length)).reversed() {
            let key = source.substring(with: match.range(at: 1)).lowercased()
            if let value = values[key] { result = (result as NSString).replacingCharacters(in: match.range, with: value) }
        }
        return result
    }

    private static func formatter(_ format: String) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = .autoupdatingCurrent
        formatter.dateFormat = format
        return formatter
    }

    private static let timestampFormatter = formatter("yyyyMMdd_HHmmss_SSS")
    private static let dateFormatter = formatter("yyyyMMdd")
    private static let timeFormatter = formatter("HHmmss")
}

private extension UUID {
    static let zeroString = "00000000-0000-0000-0000-000000000000"
}

struct LogContext {
    var taskId: String?
    var taskName: String?
    var batchId: String?
    var display: String?
    var filePath: String?
    var durationMs: Int?
    var values: [String: String]

    init(taskId: String? = nil, taskName: String? = nil, batchId: String? = nil,
         display: String? = nil, filePath: String? = nil, durationMs: Int? = nil,
         values: [String: String] = [:]) {
        self.taskId = taskId; self.taskName = taskName; self.batchId = batchId
        self.display = display; self.filePath = filePath; self.durationMs = durationMs; self.values = values
    }

    static func forTask(_ task: ScreenshotTask) -> LogContext {
        LogContext(taskId: task.id, taskName: task.name)
    }
}

final class DiagnosticLogger {
    private let lock = NSLock()
    private let applicationDirectory: URL
    private var settings = LoggingSettings()
    private var handle: FileHandle?
    private var currentDate: String?
    private var currentURL: URL?
    private var retryAfter = Date.distantPast
    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = .autoupdatingCurrent
        formatter.dateFormat = "yyyyMMdd"
        return formatter
    }()
    var onWriteFailure: ((String) -> Void)?

    init(applicationDirectory: URL) { self.applicationDirectory = applicationDirectory }

    var logDirectory: URL {
        lock.withLock { applicationDirectory.appendingPathComponent(settings.directory, isDirectory: true).standardizedFileURL }
    }

    func configure(_ settings: LoggingSettings) {
        lock.withLock {
            closeLocked()
            self.settings = settings
            retryAfter = .distantPast
        }
    }

    func error(_ eventId: String, _ message: String, error: Error? = nil, context: LogContext? = nil) {
        write(level: "error", eventId: eventId, message: message, error: error, context: context, flush: true)
    }

    func info(_ eventId: String, _ message: String, context: LogContext? = nil, flush: Bool = false) {
        write(level: "info", eventId: eventId, message: message, error: nil, context: context, flush: flush)
    }

    func debug(_ eventId: String, _ message: String, context: LogContext? = nil) {
        write(level: "debug", eventId: eventId, message: message, error: nil, context: context, flush: false)
    }

    func flush() { lock.withLock { try? handle?.synchronize() } }
    func shutdown() { lock.withLock { closeLocked() } }

    private func write(level: String, eventId: String, message: String, error: Error?,
                       context: LogContext?, flush: Bool) {
        var failure: String?
        lock.withLock {
            guard shouldWrite(level), Date() >= retryAfter else { return }
            do {
                var entry: [String: Any] = [
                    "timestampUtc": ISO8601.string(from: Date()), "level": level,
                    "eventId": eventId, "message": message
                ]
                if let value = context?.taskId { entry["taskId"] = value }
                if let value = context?.taskName { entry["taskName"] = value }
                if let value = context?.batchId { entry["batchId"] = value }
                if let value = context?.display { entry["display"] = value }
                if let value = context?.filePath { entry["filePath"] = value }
                if let value = context?.durationMs { entry["durationMs"] = value }
                if let values = context?.values, !values.isEmpty { entry["context"] = values }
                if let error {
                    entry["exceptionType"] = String(reflecting: type(of: error))
                    entry["stackTrace"] = String(describing: error)
                    entry["hResult"] = (error as NSError).code
                }
                var data = try JSONSerialization.data(withJSONObject: entry, options: [.sortedKeys])
                data.append(0x0A)
                try ensureWriterLocked(incomingBytes: data.count)
                try handle?.write(contentsOf: data)
                if flush || level == "error" { try handle?.synchronize() }
            } catch {
                closeLocked()
                retryAfter = Date().addingTimeInterval(60)
                failure = "Diagnostic logging failed: \(error.localizedDescription)"
            }
        }
        if let failure { onWriteFailure?(failure) }
    }

    private func shouldWrite(_ level: String) -> Bool {
        level == "error" || (settings.level != "error" && (level != "debug" || settings.level == "debug"))
    }

    private func ensureWriterLocked(incomingBytes: Int) throws {
        let day = Self.dayFormatter.string(from: Date())
        let maximum = Int64(max(1, settings.maxFileSizeMb)) * 1_048_576
        if let url = currentURL, handle != nil {
            let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? 0
            if currentDate != day || size + Int64(incomingBytes) > maximum { closeLocked() }
        }
        guard handle == nil else { return }
        let directory = applicationDirectory.appendingPathComponent(settings.directory, isDirectory: true).standardizedFileURL
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var sequence = 1
        while true {
            let candidate = directory.appendingPathComponent(String(format: "screen-capture-%@-%03d.jsonl", day, sequence))
            let size = (try? FileManager.default.attributesOfItem(atPath: candidate.path)[.size] as? NSNumber)?.int64Value ?? 0
            if !FileManager.default.fileExists(atPath: candidate.path) || size + Int64(incomingBytes) <= maximum {
                if !FileManager.default.fileExists(atPath: candidate.path) {
                    guard FileManager.default.createFile(atPath: candidate.path, contents: nil) else { throw CocoaError(.fileWriteUnknown) }
                }
                currentURL = candidate
                currentDate = day
                handle = try FileHandle(forWritingTo: candidate)
                try handle?.seekToEnd()
                break
            }
            sequence += 1
        }
        removeOldLogsLocked(in: directory)
    }

    private func removeOldLogsLocked(in directory: URL) {
        let files = (try? FileManager.default.contentsOfDirectory(at: directory,
            includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
        let logs = files.filter { $0.lastPathComponent.hasPrefix("screen-capture-") && $0.pathExtension == "jsonl" }
            .sorted { $0.lastPathComponent > $1.lastPathComponent }
        for file in logs.dropFirst(max(1, settings.retainedFiles)) where file != currentURL { try? FileManager.default.removeItem(at: file) }
    }

    private func closeLocked() {
        try? handle?.synchronize()
        try? handle?.close()
        handle = nil; currentURL = nil; currentDate = nil
    }
}

@MainActor
final class Configuration {
    let directory: URL
    let settingsURL: URL
    let logger: DiagnosticLogger
    private(set) var settings: AppSettings
    var onChange: (() -> Void)?
    var onError: ((String) -> Void)?
    private(set) var lastError: String?

    private var rawSettings: Any
    private var signature: Data
    private var observers: [UUID: (AppSettings) -> Void] = [:]
    private let watcherQueue = DispatchQueue(label: "ScheduledScreenshot.configuration-watcher")
    private var directorySource: DispatchSourceFileSystemObject?
    private var directoryDescriptor: Int32 = -1
    private var debounce: DispatchWorkItem?
    private var stopped = false

    init(directory: URL) {
        self.directory = directory.standardizedFileURL
        settingsURL = self.directory.appendingPathComponent("settings.json")
        logger = DiagnosticLogger(applicationDirectory: self.directory)
        settings = AppSettings(paused: true)
        rawSettings = [:]
        signature = Data()
        lastError = nil

        do {
            try FileManager.default.createDirectory(at: self.directory, withIntermediateDirectories: true)
            if !FileManager.default.fileExists(atPath: settingsURL.path) {
                var defaults = AppSettings()
                defaults.tasks = [ScreenshotTask(name: "Workday capture")]
                let encoded = try JSONCodec.encode(defaults, pretty: true)
                try AtomicFile.write(encoded, to: settingsURL)
            }
            let loaded = try Self.readCandidate(from: settingsURL, directory: self.directory)
            settings = loaded.settings; rawSettings = loaded.raw; signature = loaded.signature
            logger.configure(settings.logging)
            logger.info("CONFIG_ACCEPTED", "Initial configuration was accepted.", flush: true)
        } catch {
            settings = AppSettings(paused: true)
            rawSettings = (try? JSONSerialization.jsonObject(with: JSONCodec.encode(settings))) ?? [:]
            signature = (try? JSONCodec.canonicalData(rawSettings)) ?? Data()
            logger.configure(settings.logging)
            logger.error("CONFIG_REJECTED", "Initial configuration is invalid; all tasks are disabled.", error: error)
            lastError = error.localizedDescription
        }
        logger.onWriteFailure = { [weak self] message in Task { @MainActor in self?.publishError(message) } }
        startWatching()
    }

    @discardableResult
    func setPaused(_ paused: Bool) -> Bool { update { $0.paused = paused; return true } }

    @discardableResult
    func disable(taskID: String) -> Bool {
        disable(taskIDs: [taskID])
    }

    @discardableResult
    func disable(taskIDs: [String]) -> Bool {
        let targets = Set(taskIDs.map { $0.lowercased() })
        return update { settings in
            var changed = false
            for index in settings.tasks.indices
                where targets.contains(settings.tasks[index].id.lowercased()) && settings.tasks[index].enabled {
                settings.tasks[index].enabled = false
                changed = true
            }
            return changed
        }
    }

    @discardableResult
    func disable(tasksIfUnchanged tasks: [ScreenshotTask]) -> Bool {
        let expected = Dictionary(uniqueKeysWithValues: tasks.map { ($0.id.lowercased(), $0) })
        return update { settings in
            var changed = false
            for index in settings.tasks.indices {
                let task = settings.tasks[index]
                guard task.enabled, let previous = expected[task.id.lowercased()],
                      task.schedule == previous.schedule, task.stopCondition == previous.stopCondition else { continue }
                settings.tasks[index].enabled = false
                changed = true
            }
            return changed
        }
    }

    func resolveDirectory(_ path: String) -> URL {
        (try? SettingsValidator.resolveDirectory(path, relativeTo: directory)) ?? directory
    }

    @discardableResult
    func addChangeObserver(_ observer: @escaping (AppSettings) -> Void) -> UUID {
        let token = UUID(); observers[token] = observer; return token
    }

    func removeChangeObserver(_ token: UUID) { observers.removeValue(forKey: token) }

    func shutdown() {
        guard !stopped else { return }
        stopped = true
        debounce?.cancel(); debounce = nil
        directorySource?.cancel(); directorySource = nil
        logger.shutdown()
    }

    private func update(_ mutation: (inout AppSettings) -> Bool) -> Bool {
        do {
            var baseSettings = settings
            var baseRaw = rawSettings
            if let latest = try? Self.readCandidate(from: settingsURL, directory: directory) {
                baseSettings = latest.settings; baseRaw = latest.raw
            }
            guard mutation(&baseSettings) else { return false }
            let validation = SettingsValidator.validate(baseSettings, applicationDirectory: directory, verifyWritablePaths: true)
            guard validation.isValid else { throw ConfigurationError.invalid(validation.errors.joined(separator: "\n")) }
            let known = try JSONSerialization.jsonObject(with: JSONCodec.encode(baseSettings))
            let merged = JSONCodec.merge(known: known, into: baseRaw)
            let data = try JSONSerialization.data(withJSONObject: merged, options: [.prettyPrinted, .sortedKeys])
            try AtomicFile.write(data, to: settingsURL)
            settings = baseSettings; rawSettings = merged; signature = try JSONCodec.canonicalData(merged)
            lastError = nil
            logger.configure(settings.logging)
            logger.info("CONFIG_UPDATED", "Configuration was updated by the application.", flush: true)
            publishChange()
            return true
        } catch {
            logger.error("CONFIG_REJECTED", "Configuration update was rejected; the last valid settings remain active.", error: error)
            publishError(error.localizedDescription)
            return false
        }
    }

    private func startWatching() {
        directoryDescriptor = open(directory.path, O_EVTONLY)
        guard directoryDescriptor >= 0 else {
            publishError("Unable to watch the configuration directory.")
            return
        }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: directoryDescriptor,
            eventMask: [.write, .rename, .delete, .extend, .attrib, .link], queue: watcherQueue)
        source.setEventHandler { [weak self] in
            guard let self else { return }
            self.debounce?.cancel()
            let item = DispatchWorkItem { [weak self] in Task { @MainActor in self?.reloadFromDisk() } }
            self.debounce = item
            self.watcherQueue.asyncAfter(deadline: .now() + .milliseconds(500), execute: item)
        }
        let descriptor = directoryDescriptor
        source.setCancelHandler { close(descriptor) }
        directorySource = source
        source.resume()
    }

    private func reloadFromDisk() {
        guard !stopped else { return }
        do {
            let rawCandidate = try Self.readRaw(from: settingsURL)
            guard rawCandidate.signature != signature else { return }
            let loaded = try Self.decodeCandidate(data: rawCandidate.data, raw: rawCandidate.raw,
                                                   signature: rawCandidate.signature, directory: directory)
            settings = loaded.settings; rawSettings = loaded.raw; signature = loaded.signature
            lastError = nil
            logger.configure(settings.logging)
            logger.info("CONFIG_ACCEPTED", "Configuration changes were accepted.", flush: true)
            publishChange()
        } catch {
            logger.error("CONFIG_REJECTED", "Configuration changes were rejected; the last valid settings remain active.", error: error)
            publishError(error.localizedDescription)
        }
    }

    private func publishChange() {
        let snapshot = settings
        onChange?()
        for observer in observers.values { observer(snapshot) }
    }

    private func publishError(_ message: String) { lastError = message; onError?(message) }

    private static func readCandidate(from url: URL, directory: URL) throws -> (settings: AppSettings, raw: Any, signature: Data) {
        let rawCandidate = try readRaw(from: url)
        return try decodeCandidate(data: rawCandidate.data, raw: rawCandidate.raw,
                                   signature: rawCandidate.signature, directory: directory)
    }

    private static func readRaw(from url: URL) throws -> (data: Data, raw: Any, signature: Data) {
        let data = try Data(contentsOf: url, options: [.mappedIfSafe])
        guard data.count <= 4 * 1024 * 1024 else { throw ConfigurationError.invalid("settings.json exceeds 4 MiB.") }
        let raw = try JSONSerialization.jsonObject(with: data)
        guard raw is [String: Any] else { throw ConfigurationError.invalid("Configuration root must be an object.") }
        return (data, raw, try JSONCodec.canonicalData(raw))
    }

    private static func decodeCandidate(data: Data, raw: Any, signature: Data,
                                        directory: URL) throws -> (settings: AppSettings, raw: Any, signature: Data) {
        let decoded = try JSONDecoder().decode(AppSettings.self, from: data)
        let validation = SettingsValidator.validate(decoded, applicationDirectory: directory, verifyWritablePaths: true)
        guard validation.isValid else { throw ConfigurationError.invalid(validation.errors.joined(separator: "\n")) }
        return (decoded, raw, signature)
    }
}

enum AtomicFile {
    static func write(_ data: Data, to destination: URL) throws {
        let manager = FileManager.default
        try manager.createDirectory(at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        let temporary = destination.deletingLastPathComponent()
            .appendingPathComponent(".\(destination.lastPathComponent).\(UUID().uuidString).tmp")
        do {
            guard manager.createFile(atPath: temporary.path, contents: nil) else { throw CocoaError(.fileWriteUnknown) }
            let handle = try FileHandle(forWritingTo: temporary)
            do {
                try handle.write(contentsOf: data)
                try handle.synchronize()
                try handle.close()
            } catch {
                try? handle.close()
                throw error
            }
            if rename(temporary.path, destination.path) != 0 {
                throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
            }
        } catch {
            try? manager.removeItem(at: temporary)
            throw error
        }
    }
}

enum JSONCodec {
    static func encode<T: Encodable>(_ value: T, pretty: Bool = false) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = pretty ? [.prettyPrinted, .sortedKeys] : [.sortedKeys]
        return try encoder.encode(value)
    }

    static func canonicalData(_ object: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
    }

    static func merge(known: Any, into original: Any) -> Any {
        if let known = known as? [String: Any], let original = original as? [String: Any] {
            var merged = original
            for (key, value) in known { merged[key] = merge(known: value, into: original[key] as Any) }
            return merged
        }
        if let known = known as? [Any], let original = original as? [Any] {
            return known.enumerated().map { index, value in
                index < original.count ? merge(known: value, into: original[index]) : value
            }
        }
        return known
    }
}

enum ISO8601 {
    private static let formatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
    static func string(from date: Date) -> String { formatter.string(from: date) }
    static func date(from value: String?) -> Date? {
        guard let value else { return nil }
        return formatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    }
}

private enum ConfigurationError: LocalizedError {
    case invalid(String)
    var errorDescription: String? { if case .invalid(let message) = self { return message }; return nil }
}

private extension NSLock {
    func withLock<T>(_ action: () throws -> T) rethrows -> T {
        lock(); defer { unlock() }; return try action()
    }
}
