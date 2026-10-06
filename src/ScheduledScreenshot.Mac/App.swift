import AppKit
import Darwin

private struct LaunchOptions {
    var headless = false
    var runFor: Double?
    var directory = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Application Support/ScheduledScreenshot", isDirectory: true)

    init() throws {
        var arguments = Array(CommandLine.arguments.dropFirst())
        while !arguments.isEmpty {
            switch arguments.removeFirst() {
            case "--headless": headless = true
            case "--run-for":
                guard !arguments.isEmpty, let seconds = Double(arguments.removeFirst()),
                      seconds.isFinite, seconds > 0, seconds <= 86400 else {
                    throw NSError(domain: "Launch", code: 2, userInfo: [NSLocalizedDescriptionKey: "--run-for requires 1..86400 seconds."])
                }
                runFor = seconds
            case "--data-directory":
                guard !arguments.isEmpty else {
                    throw NSError(domain: "Launch", code: 2, userInfo: [NSLocalizedDescriptionKey: "--data-directory requires a path."])
                }
                directory = URL(fileURLWithPath: (arguments.removeFirst() as NSString).expandingTildeInPath, isDirectory: true)
            default:
                throw NSError(domain: "Launch", code: 2, userInfo: [NSLocalizedDescriptionKey:
                    "Usage: ScheduledScreenshot [--headless] [--run-for SECONDS] [--data-directory PATH]"])
            }
        }
    }
}

@MainActor
private final class SessionMonitor {
    var onChange: (() -> Void)?
    var onClockChange: (() -> Void)?
    private var sleeping = false
    private var locked = false
    private var inactive = false
    private var observations: [(NotificationCenter, NSObjectProtocol)] = []

    var available: Bool {
        guard !sleeping, !locked, !inactive,
              let session = CGSessionCopyCurrentDictionary() as? [String: Any],
              session[kCGSessionOnConsoleKey as String] as? Bool == true else { return false }
        // Session notifications protect captures in progress; this also checks startup while locked.
        if session["CGSSessionScreenIsLocked"] as? Bool == true { return false }
        return CGDisplayIsAsleep(CGMainDisplayID()) == 0
    }

    init() {
        let workspace = NSWorkspace.shared.notificationCenter
        observe(workspace, NSWorkspace.willSleepNotification) { self.sleeping = true }
        observe(workspace, NSWorkspace.screensDidSleepNotification) { self.sleeping = true }
        observe(workspace, NSWorkspace.didWakeNotification) { self.sleeping = false }
        observe(workspace, NSWorkspace.screensDidWakeNotification) { self.sleeping = false }
        observe(workspace, NSWorkspace.sessionDidResignActiveNotification) { self.inactive = true }
        observe(workspace, NSWorkspace.sessionDidBecomeActiveNotification) { self.inactive = false }
        let distributed = DistributedNotificationCenter.default()
        observe(distributed, Notification.Name("com.apple.screenIsLocked")) { self.locked = true }
        observe(distributed, Notification.Name("com.apple.screenIsUnlocked")) { self.locked = false }
        for name in [Notification.Name.NSSystemClockDidChange, NSNotification.Name.NSSystemTimeZoneDidChange] {
            let token = NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.onClockChange?() }
            }
            observations.append((.default, token))
        }
    }

    private func observe(_ center: NotificationCenter, _ name: Notification.Name, update: @escaping @MainActor () -> Void) {
        let token = center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                update()
                self?.onChange?()
            }
        }
        observations.append((center, token))
    }

    func shutdown() {
        for (center, token) in observations { center.removeObserver(token) }
        observations.removeAll()
    }
}

@MainActor
private final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let options: LaunchOptions
    private let lockDescriptor: Int32
    private var configuration: Configuration!
    private var scheduler: Scheduler!
    private var capture: CaptureCoordinator!
    private var session: SessionMonitor!
    private var statusItem: NSStatusItem?
    private var signals: [DispatchSourceSignal] = []
    private var lastWarning = Date.distantPast
    private var terminating = false

    init(options: LaunchOptions, lockDescriptor: Int32) {
        self.options = options
        self.lockDescriptor = lockDescriptor
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        configuration = Configuration(directory: options.directory)
        session = SessionMonitor()
        capture = CaptureCoordinator(directory: options.directory, logger: configuration.logger)
        capture.isSessionAvailable = { [weak self] in self?.session.available ?? false }
        capture.onError = { [weak self] message in self?.showWarning(message) }
        scheduler = Scheduler(configuration: configuration) { [weak self] tasks in
            guard let self = self, !self.terminating else { return CaptureBatchResult(skipped: true) }
            return await self.capture.capture(tasks, reason: "scheduled")
        }
        configuration.onChange = { [weak self] in self?.refreshAvailability() }
        configuration.onError = { [weak self] message in self?.showWarning(message) }
        if let error = configuration.lastError { showWarning(error) }
        scheduler.onChange = { [weak self] in
            guard let self = self else { return }
            self.statusItem?.button?.toolTip = self.statusText
        }
        session.onClockChange = { [weak self] in self?.scheduler.notifyClockChanged() }
        session.onChange = { [weak self] in
            guard let self = self else { return }
            self.configuration.logger.info("SESSION_CHANGED", self.session.available ? "Session is available." : "Session is unavailable.")
            self.refreshAvailability()
        }
        refreshAvailability()
        if !options.headless {
            let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            item.button?.image = NSImage(systemSymbolName: "camera", accessibilityDescription: "Scheduled Screenshot")
            let menu = NSMenu()
            menu.delegate = self
            item.menu = menu
            statusItem = item
        }
        for signalNumber in [SIGTERM, SIGINT] {
            signal(signalNumber, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: signalNumber, queue: .main)
            source.setEventHandler { NSApplication.shared.terminate(nil) }
            source.resume()
            signals.append(source)
        }
        if let duration = options.runFor {
            DispatchQueue.main.asyncAfter(deadline: .now() + duration) { NSApplication.shared.terminate(nil) }
        }
        configuration.logger.info("APP_START", "Scheduled Screenshot started on macOS.")
    }

    private func refreshAvailability() {
        let available = session.available && CGPreflightScreenCaptureAccess()
        if scheduler.available == available { scheduler.reschedule() }
        else { scheduler.available = available }
        statusItem?.button?.toolTip = statusText
    }

    private var statusText: String {
        if configuration.settings.paused { return "Paused" }
        if !CGPreflightScreenCaptureAccess() { return "Screen Recording permission required" }
        if !session.available { return "Waiting for an active session" }
        if let date = scheduler.nextDate {
            return "Next: \(scheduler.nextName ?? "Capture") at \(DateFormatter.localizedString(from: date, dateStyle: .short, timeStyle: .medium))"
        }
        return "No scheduled captures"
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        refreshAvailability()
        menu.removeAllItems()
        let status = NSMenuItem(title: statusText, action: nil, keyEquivalent: "")
        status.isEnabled = false
        menu.addItem(status)
        menu.addItem(.separator())
        let now = NSMenuItem(title: "Capture Now", action: nil, keyEquivalent: "")
        let captures = NSMenu()
        for task in configuration.settings.tasks where task.enabled {
            let item = menuItem(task.name, #selector(captureNow(_:)))
            item.representedObject = task.id
            captures.addItem(item)
        }
        now.submenu = captures
        now.isEnabled = !captures.items.isEmpty
        menu.addItem(now)
        menu.addItem(menuItem(configuration.settings.paused ? "Resume All" : "Pause All", #selector(togglePause)))
        menu.addItem(.separator())
        menu.addItem(menuItem("Grant Screen Recording Access", #selector(requestPermission)))
        menu.addItem(menuItem("Configure in Browser", #selector(openEditor)))
        menu.addItem(menuItem("Edit JSON", #selector(editJSON)))
        menu.addItem(menuItem("Open Settings Folder", #selector(openSettings)))
        let outputs = NSMenuItem(title: "Open Output Folder", action: nil, keyEquivalent: "")
        let directories = NSMenu()
        for task in configuration.settings.tasks {
            let item = menuItem(task.name, #selector(openOutput(_:)))
            item.representedObject = task.id
            directories.addItem(item)
        }
        outputs.submenu = directories
        outputs.isEnabled = !directories.items.isEmpty
        menu.addItem(outputs)
        menu.addItem(menuItem("Open Logs", #selector(openLogs)))
        menu.addItem(.separator())
        menu.addItem(menuItem("Quit Scheduled Screenshot", #selector(quit), key: "q"))
    }

    private func menuItem(_ title: String, _ action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    @objc private func captureNow(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let task = configuration.settings.tasks.first(where: { $0.id == id && $0.enabled }) else { return }
        Task { _ = await capture.capture([task], reason: "manual") }
    }

    @objc private func togglePause() { configuration.setPaused(!configuration.settings.paused) }
    @objc private func quit() { NSApplication.shared.terminate(nil) }
    @objc private func openSettings() { open(options.directory) }
    @objc private func editJSON() { open(options.directory.appendingPathComponent("settings.json")) }
    @objc private func openLogs() {
        open(configuration.resolveDirectory(configuration.settings.logging.directory), createDirectory: true)
    }
    @objc private func openOutput(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let task = configuration.settings.tasks.first(where: { $0.id == id }) else { return }
        open(configuration.resolveDirectory(task.capture.outputFolder), createDirectory: true)
    }
    @objc private func openEditor() {
        guard let url = Bundle.main.url(forResource: "config-editor", withExtension: "html") else {
            showWarning("The offline configuration editor is missing from the app bundle.")
            return
        }
        open(url)
    }
    @objc private func requestPermission() {
        NSApplication.shared.activate(ignoringOtherApps: true)
        if !CGRequestScreenCaptureAccess(), let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            open(url)
        }
        refreshAvailability()
    }

    private func open(_ url: URL, createDirectory: Bool = false) {
        do {
            if createDirectory { try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true) }
            if !NSWorkspace.shared.open(url) { showWarning("Unable to open \(url.path).") }
        } catch { showWarning(error.localizedDescription) }
    }

    private func showWarning(_ message: String) {
        guard !options.headless, Date().timeIntervalSince(lastWarning) > 30, !terminating else { return }
        lastWarning = Date()
        // Present outside a menu-tracking callback so alerts cannot interrupt the menu.
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = "Scheduled Screenshot"
            alert.informativeText = message
            alert.alertStyle = .warning
            NSApplication.shared.activate(ignoringOtherApps: true)
            alert.runModal()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        terminating = true
        session?.shutdown()
        scheduler?.shutdown()
        configuration?.logger.info("APP_EXIT", "Scheduled Screenshot is exiting.")
        configuration?.shutdown()
        for source in signals { source.cancel() }
        flock(lockDescriptor, LOCK_UN)
        close(lockDescriptor)
    }
}

@main
private struct ScheduledScreenshotApp {
    @MainActor static func main() {
        do {
            let options = try LaunchOptions()
            try FileManager.default.createDirectory(at: options.directory, withIntermediateDirectories: true)
            let lock = Darwin.open(options.directory.appendingPathComponent(".instance.lock").path, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
            guard lock >= 0 else { throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno)) }
            guard flock(lock, LOCK_EX | LOCK_NB) == 0 else {
                close(lock)
                fputs("Scheduled Screenshot is already running for this settings folder.\n", stderr)
                exit(1)
            }
            let application = NSApplication.shared
            application.setActivationPolicy(options.headless ? .prohibited : .accessory)
            let delegate = AppDelegate(options: options, lockDescriptor: lock)
            application.delegate = delegate
            withExtendedLifetime(delegate) { application.run() }
        } catch {
            fputs("\(error.localizedDescription)\n", stderr)
            exit(2)
        }
    }
}
