import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = NSRect(x: 60, y: 60, width: 940, height: 860)
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    super.awakeFromNib()

    // The xib-provided frame wins if set before this point.
    self.setFrame(NSRect(x: 60, y: 60, width: 940, height: 860), display: true)
  }
}
