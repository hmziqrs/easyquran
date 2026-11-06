// Launches argv[1], waits for an on-screen window of its pid. Prints: pid ms x y w h. Leaves the app running.
import Foundation
import CoreGraphics
let t=Process(); t.executableURL=URL(fileURLWithPath:CommandLine.arguments[1])
t.standardOutput=FileHandle.nullDevice; t.standardError=FileHandle.nullDevice
let s=DispatchTime.now(); try! t.run(); let pid=Int(t.processIdentifier)
func ms()->Double{Double(DispatchTime.now().uptimeNanoseconds-s.uptimeNanoseconds)/1e6}
while ms()<30000 {
  let l=CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements],kCGNullWindowID) as! [[String:Any]]
  if let w=l.first(where:{($0[kCGWindowOwnerPID as String] as? Int)==pid && ($0[kCGWindowLayer as String] as? Int)==0}), let b=w[kCGWindowBounds as String] as? [String:Double] {
    print("\(pid) \(ms()) \(b["X"]!) \(b["Y"]!) \(b["Width"]!) \(b["Height"]!)"); exit(0)
  }
  usleep(2000)
}
print("\(pid) timeout"); exit(1)
