import CoreGraphics
import Foundation

// keyinject <gap-ms> <digits>
// Posts REAL OS key events (keyDown+keyUp per digit) through CGEventPost(.cgSessionEventTap),
// with a fixed inter-key gap. macOS keycodes for digits.
let keycodes: [Character: UInt16] = [
    "0": 29, "1": 18, "2": 19, "3": 20, "4": 21,
    "5": 23, "6": 22, "7": 26, "8": 28, "9": 25,
]

guard CommandLine.arguments.count == 3,
      let gapMs = UInt32(CommandLine.arguments[1]) else {
    FileHandle.standardError.write("usage: keyinject <gap-ms> <digits>\n".data(using: .utf8)!)
    exit(2)
}
let digits = CommandLine.arguments[2]
for ch in digits {
    guard let code = keycodes[ch] else { continue }
    if let down = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true) {
        down.post(tap: .cgSessionEventTap)
    }
    usleep(8000) // key held 8 ms
    if let up = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false) {
        up.post(tap: .cgSessionEventTap)
    }
    usleep(gapMs * 1000)
}
