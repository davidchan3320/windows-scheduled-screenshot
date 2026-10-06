import Foundation

private struct RuntimeState: Codable {
    var tasks: [String: TaskRuntimeState] = [:]
}

private struct TaskRuntimeState: Codable, Equatable {
    var intervalAnchorUtc: String?
    var scheduleSignature: String?
    var durationDeadlineUtc: String?
    var durationSignature: String?
    var lastFixedOccurrence: String?
    var completedCaptures: Int?
    var countSignature: String?
    var completedFixedTimes: [String]?

    func reachedCaptureLimit(for task: ScreenshotTask) -> Bool {
        (task.stopCondition.mode == "count" && (completedCaptures ?? 0) >= (task.stopCondition.captureCount ?? 1))
            || (task.schedule.type == "fixedOnce"
                && task.schedule.times.allSatisfy { (completedFixedTimes ?? []).contains($0) })
    }
}

private struct TaskOccurrence {
    let task: ScreenshotTask
    let due: Date
    let fixedOccurrenceKey: String?
}

private enum ScheduleCalculator {
    static func nextOccurrence(for task: ScreenshotTask, state: TaskRuntimeState, after now: Date,
                               calendar: Calendar = .autoupdatingCurrent) -> TaskOccurrence? {
        guard task.enabled, !state.reachedCaptureLimit(for: task) else { return nil }
        let occurrence = ["fixed", "fixedOnce"].contains(task.schedule.type)
            ? nextFixed(for: task, state: state, after: now, calendar: calendar)
            : nextInterval(for: task, state: state, after: now, calendar: calendar)
        if let occurrence, let deadline = stopDate(for: task, state: state, calendar: calendar), occurrence.due >= deadline {
            return nil
        }
        return occurrence
    }

    static func stopDate(for task: ScreenshotTask, state: TaskRuntimeState,
                         calendar: Calendar = .autoupdatingCurrent) -> Date? {
        switch task.stopCondition.mode {
        case "duration": return ISO8601.date(from: state.durationDeadlineUtc)
        case "at": return SettingsValidator.parseLocalDateTime(task.stopCondition.endAtLocal, calendar: calendar)
        default: return nil
        }
    }

    private static func nextInterval(for task: ScreenshotTask, state: TaskRuntimeState, after now: Date,
                                     calendar: Calendar) -> TaskOccurrence? {
        guard let anchor = ISO8601.date(from: state.intervalAnchorUtc) else { return nil }
        let interval = TimeInterval(task.schedule.intervalSeconds)
        let today = calendar.startOfDay(for: now)
        for offset in 0...8 {
            guard let day = calendar.date(byAdding: .day, value: offset, to: today),
                  task.schedule.weekdays.contains(dayToken(for: day, calendar: calendar)) else { continue }
            let windowStart: Date
            let windowEnd: Date
            if task.schedule.activeHoursEnabled {
                guard let startClock = SettingsValidator.parseClock(task.schedule.activeStart),
                      let endClock = SettingsValidator.parseClock(task.schedule.activeEnd),
                      let start = localTime(startClock, on: day, calendar: calendar),
                      let end = localTime(endClock, on: day, calendar: calendar) else { continue }
                windowStart = start; windowEnd = end
            } else {
                windowStart = day
                guard let end = calendar.date(byAdding: .day, value: 1, to: day) else { continue }
                windowEnd = end
            }
            guard windowEnd > now else { continue }
            let lowerBound = max(windowStart, now.addingTimeInterval(0.001))
            let delta = lowerBound.timeIntervalSince(anchor)
            let steps = delta <= 0 ? 0 : ceil(delta / interval)
            var candidate = anchor.addingTimeInterval(steps * interval)
            if candidate <= now { candidate = candidate.addingTimeInterval(interval) }
            if candidate < windowStart {
                candidate = candidate.addingTimeInterval(ceil(windowStart.timeIntervalSince(candidate) / interval) * interval)
            }
            if candidate < windowEnd { return TaskOccurrence(task: task, due: candidate, fixedOccurrenceKey: nil) }
        }
        return nil
    }

    private static func nextFixed(for task: ScreenshotTask, state: TaskRuntimeState, after now: Date,
                                  calendar: Calendar) -> TaskOccurrence? {
        let today = calendar.startOfDay(for: now)
        let clocks = task.schedule.times.compactMap { value -> (String, DateComponents, Int)? in
            guard let components = SettingsValidator.parseClock(value) else { return nil }
            let seconds = (components.hour ?? 0) * 3600 + (components.minute ?? 0) * 60 + (components.second ?? 0)
            return (value, components, seconds)
        }.sorted { $0.2 < $1.2 }
        for offset in 0...8 {
            guard let day = calendar.date(byAdding: .day, value: offset, to: today),
                  task.schedule.weekdays.contains(dayToken(for: day, calendar: calendar)) else { continue }
            for (text, clock, _) in clocks {
                if task.schedule.type == "fixedOnce" && (state.completedFixedTimes ?? []).contains(text) { continue }
                guard let candidate = localTime(clock, on: day, calendar: calendar), candidate > now else { continue }
                let key = localKey(candidate, calendar: calendar)
                if key != state.lastFixedOccurrence {
                    return TaskOccurrence(task: task, due: candidate, fixedOccurrenceKey: key)
                }
            }
        }
        return nil
    }

    private static func localTime(_ clock: DateComponents, on day: Date, calendar: Calendar) -> Date? {
        guard let start = calendar.date(byAdding: .second, value: -1, to: calendar.startOfDay(for: day)) else { return nil }
        guard let candidate = calendar.nextDate(after: start, matching: clock, matchingPolicy: .strict,
                                                repeatedTimePolicy: .last, direction: .forward) else { return nil }
        let wantedDay = calendar.dateComponents([.year, .month, .day], from: day)
        let actualDay = calendar.dateComponents([.year, .month, .day], from: candidate)
        return wantedDay == actualDay ? candidate : nil
    }

    private static func dayToken(for date: Date, calendar: Calendar) -> String {
        switch calendar.component(.weekday, from: date) {
        case 2: return "Mon"; case 3: return "Tue"; case 4: return "Wed"
        case 5: return "Thu"; case 6: return "Fri"; case 7: return "Sat"
        default: return "Sun"
        }
    }

    private static func localKey(_ date: Date, calendar: Calendar) -> String {
        let values = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: date)
        return String(format: "%04d-%02d-%02dT%02d:%02d:%02d", values.year ?? 0, values.month ?? 0,
                      values.day ?? 0, values.hour ?? 0, values.minute ?? 0, values.second ?? 0)
    }
}

private final class RuntimeStateStore {
    private let url: URL
    private let logger: DiagnosticLogger
    private(set) var state: RuntimeState

    init(directory: URL, logger: DiagnosticLogger) {
        url = directory.appendingPathComponent("runtime-state.json")
        self.logger = logger
        do {
            state = try JSONDecoder().decode(RuntimeState.self, from: Data(contentsOf: url))
        } catch where (error as NSError).code == NSFileReadNoSuchFileError {
            state = RuntimeState()
        } catch {
            state = RuntimeState()
            logger.error("RUNTIME_STATE_REJECTED", "Runtime state could not be read; safe new state will be used.", error: error)
        }
    }

    func save(_ state: RuntimeState) {
        self.state = state
        do { try AtomicFile.write(JSONCodec.encode(state, pretty: true), to: url) }
        catch { logger.error("RUNTIME_STATE_WRITE_FAILED", "Runtime state could not be persisted.", error: error) }
    }
}

@MainActor
final class Scheduler {
    private let configuration: Configuration
    private let capture: ([ScreenshotTask]) async -> CaptureBatchResult
    private let stateStore: RuntimeStateStore
    private var state: RuntimeState
    private var settings: AppSettings
    private var timer: Timer?
    private var captureTask: Task<Void, Never>?
    private var observerToken: UUID?
    private var captureInProgress = false
    private var stopped = false

    var onChange: (() -> Void)?
    var available: Bool = true {
        didSet {
            guard available != oldValue else { return }
            configuration.logger.info(available ? "CAPTURE_RESUMED" : "CAPTURE_SUSPENDED",
                available ? "Scheduled capture is available." : "Scheduled capture is temporarily unavailable.", flush: true)
            reschedule()
        }
    }
    private(set) var nextDate: Date?
    private(set) var nextName: String?

    init(configuration: Configuration, capture: @escaping ([ScreenshotTask]) async -> CaptureBatchResult) {
        self.configuration = configuration
        self.capture = capture
        settings = configuration.settings
        stateStore = RuntimeStateStore(directory: configuration.directory, logger: configuration.logger)
        state = stateStore.state
        observerToken = configuration.addChangeObserver { [weak self] newSettings in
            self?.apply(newSettings)
        }
        reconcileState(at: Date())
        reschedule()
    }

    func reschedule() {
        guard !stopped else { return }
        timer?.invalidate(); timer = nil
        let now = Date()
        var earliestOccurrence: TaskOccurrence?
        var earliestDeadline: Date?
        for task in settings.tasks where task.enabled {
            guard let runtime = state.tasks[stateKey(task.id)] else { continue }
            if runtime.reachedCaptureLimit(for: task) { earliestDeadline = now }
            if let deadline = ScheduleCalculator.stopDate(for: task, state: runtime),
               earliestDeadline == nil || deadline < earliestDeadline! { earliestDeadline = deadline }
            if !settings.paused && available,
               let occurrence = ScheduleCalculator.nextOccurrence(for: task, state: runtime, after: now),
               earliestOccurrence == nil || occurrence.due < earliestOccurrence!.due { earliestOccurrence = occurrence }
        }
        nextDate = earliestOccurrence?.due
        nextName = earliestOccurrence?.task.name
        var nextEvent = earliestOccurrence?.due
        if let deadline = earliestDeadline, nextEvent == nil || deadline < nextEvent! { nextEvent = deadline }
        guard let nextEvent else { onChange?(); return }
        let interval = max(0.001, nextEvent.timeIntervalSince(now))
        let newTimer = Timer(timeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor in self?.processTimer() }
        }
        timer = newTimer
        RunLoop.main.add(newTimer, forMode: .common)
        configuration.logger.debug("NEXT_OCCURRENCE_CALCULATED", "The scheduler timer was armed.",
            context: LogContext(taskId: earliestOccurrence?.task.id, taskName: earliestOccurrence?.task.name,
                                values: ["nextEventUtc": ISO8601.string(from: nextEvent)]))
        onChange?()
    }

    func notifyClockChanged() {
        configuration.logger.info("CLOCK_CHANGED", "The system clock or timezone changed; schedules were recalculated.", flush: true)
        reschedule()
    }

    func shutdown() {
        guard !stopped else { return }
        stopped = true
        timer?.invalidate(); timer = nil
        captureTask?.cancel(); captureTask = nil
        if let observerToken { configuration.removeChangeObserver(observerToken) }
        observerToken = nil
    }

    private func apply(_ newSettings: AppSettings) {
        guard !stopped else { return }
        settings = newSettings
        reconcileState(at: Date())
        reschedule()
    }

    private func reconcileState(at now: Date) {
        var changed = false
        let enabledKeys = Set(settings.tasks.filter(\.enabled).map { stateKey($0.id) })
        for key in state.tasks.keys where !enabledKeys.contains(key) {
            state.tasks.removeValue(forKey: key); changed = true
        }
        for task in settings.tasks where task.enabled {
            let key = stateKey(task.id)
            var runtime = state.tasks[key] ?? TaskRuntimeState()
            if state.tasks[key] == nil { changed = true }
            let scheduleSignature = signature(of: task.schedule)
            if runtime.scheduleSignature != scheduleSignature {
                runtime.scheduleSignature = scheduleSignature
                runtime.intervalAnchorUtc = ISO8601.string(from: now)
                runtime.lastFixedOccurrence = nil
                runtime.completedFixedTimes = []
                changed = true
            }
            let countSignature = task.stopCondition.mode == "count"
                ? "count:\(task.stopCondition.captureCount ?? 0)" : nil
            if runtime.countSignature != countSignature {
                runtime.countSignature = countSignature
                runtime.completedCaptures = 0
                changed = true
            }
            let durationSignature = signature(of: task.stopCondition)
            if task.stopCondition.mode == "duration" {
                if runtime.durationSignature != durationSignature || ISO8601.date(from: runtime.durationDeadlineUtc) == nil {
                    runtime.durationSignature = durationSignature
                    runtime.durationDeadlineUtc = ISO8601.string(from: now.addingTimeInterval(TimeInterval(task.stopCondition.durationSeconds ?? 0)))
                    changed = true
                }
            } else if runtime.durationDeadlineUtc != nil || runtime.durationSignature != durationSignature {
                runtime.durationSignature = durationSignature
                runtime.durationDeadlineUtc = nil
                changed = true
            }
            if state.tasks[key] != runtime { state.tasks[key] = runtime; changed = true }
        }
        if changed { stateStore.save(state) }
    }

    private func processTimer() {
        guard !stopped else { return }
        timer?.invalidate(); timer = nil
        let now = Date()
        let expired = settings.tasks.filter { task in
            guard task.enabled, let runtime = state.tasks[stateKey(task.id)] else { return false }
            return runtime.reachedCaptureLimit(for: task)
                || ScheduleCalculator.stopDate(for: task, state: runtime).map { $0 <= now } == true
        }
        for task in expired {
            configuration.logger.info("TASK_ENDED", "A task reached its configured end condition.",
                                      context: .forTask(task), flush: true)
        }
        if !expired.isEmpty { _ = configuration.disable(tasksIfUnchanged: expired) }

        let expiredIDs = Set(expired.map { $0.id.lowercased() })
        var due: [TaskOccurrence] = []
        if !settings.paused && available {
            for task in settings.tasks where task.enabled && !expiredIDs.contains(task.id.lowercased()) {
                guard let runtime = state.tasks[stateKey(task.id)],
                      let occurrence = ScheduleCalculator.nextOccurrence(for: task, state: runtime,
                                                                         after: now.addingTimeInterval(-0.5)),
                      occurrence.due <= now.addingTimeInterval(0.15) else { continue }
                due.append(occurrence)
            }
        }
        recordFixedOccurrences(due)
        if !due.isEmpty {
            let tasks = due.map(\.task)
            if captureInProgress {
                for task in tasks {
                    configuration.logger.info("TASK_SKIPPED_BUSY", "A scheduled occurrence was skipped because capture was busy.",
                                              context: .forTask(task))
                }
            } else {
                captureInProgress = true
                let capturedState = state.tasks
                captureTask = Task { [weak self, capture] in
                    let result = await capture(tasks)
                    guard !Task.isCancelled else { return }
                    await MainActor.run {
                        guard let self, !self.stopped else { return }
                        if !result.skipped {
                            self.recordExecutions(due, successfulIDs: result.successfulTaskIDs, capturedState: capturedState)
                        }
                        self.captureInProgress = false
                        self.captureTask = nil
                        self.reschedule()
                    }
                }
            }
        }
        reschedule()
    }

    private func recordFixedOccurrences(_ occurrences: [TaskOccurrence]) {
        var changed = false
        for occurrence in occurrences {
            guard let key = occurrence.fixedOccurrenceKey else { continue }
            let id = stateKey(occurrence.task.id)
            guard var runtime = state.tasks[id], runtime.lastFixedOccurrence != key else { continue }
            runtime.lastFixedOccurrence = key
            state.tasks[id] = runtime
            changed = true
        }
        if changed { stateStore.save(state) }
    }

    private func recordExecutions(_ occurrences: [TaskOccurrence], successfulIDs: [String],
                                  capturedState: [String: TaskRuntimeState]) {
        let successfulKeys = Set(successfulIDs.map(stateKey))
        var completedTasks: [ScreenshotTask] = []
        var changed = false
        for occurrence in occurrences {
            let task = occurrence.task
            let key = stateKey(task.id)
            guard let currentTask = settings.tasks.first(where: { $0.enabled && stateKey($0.id) == key }),
                  currentTask.schedule == task.schedule, var runtime = state.tasks[key],
                  let previous = capturedState[key], runtime.countSignature == previous.countSignature,
                  runtime.scheduleSignature == previous.scheduleSignature,
                  runtime.intervalAnchorUtc == previous.intervalAnchorUtc else { continue }
            if task.stopCondition.mode == "count" && successfulKeys.contains(key) {
                runtime.completedCaptures = min(2_147_483_647, (runtime.completedCaptures ?? 0) + 1)
                changed = true
            }
            if task.schedule.type == "fixedOnce", let occurrenceKey = occurrence.fixedOccurrenceKey {
                let clock = String(occurrenceKey.suffix(8))
                if !(runtime.completedFixedTimes ?? []).contains(clock) {
                    runtime.completedFixedTimes = (runtime.completedFixedTimes ?? []) + [clock]
                    changed = true
                }
            }
            state.tasks[key] = runtime
            if runtime.reachedCaptureLimit(for: currentTask) { completedTasks.append(currentTask) }
        }
        if changed { stateStore.save(state) }
        if !completedTasks.isEmpty {
            for task in completedTasks {
                configuration.logger.info("TASK_ENDED", "A task completed its configured executions.",
                                          context: .forTask(task), flush: true)
            }
            _ = configuration.disable(tasksIfUnchanged: completedTasks)
        }
    }

    private func stateKey(_ id: String) -> String { id.lowercased() }

    private func signature<T: Encodable>(of value: T) -> String {
        let data = (try? JSONCodec.encode(value)) ?? Data()
        var hash: UInt64 = 14_695_981_039_346_656_037
        for byte in data { hash = (hash ^ UInt64(byte)) &* 1_099_511_628_211 }
        return String(format: "%016llx", hash)
    }
}
