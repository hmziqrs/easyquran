// posts wheel events at (x,y): 240 steps at ~120Hz of dy pixels, alternating direction every 80.
import Foundation
import CoreGraphics
let x=Double(CommandLine.arguments[1])!, y=Double(CommandLine.arguments[2])!
let m=CGEvent(mouseEventSource:nil,mouseType:.mouseMoved,mouseCursorPosition:CGPoint(x:x,y:y),mouseButton:.left)!; m.post(tap:.cghidEventTap); usleep(200000)
for i in 0..<720 {
  let dy:Int32 = -30; _ = i
  let e=CGEvent(scrollWheelEvent2Source:nil,units:.pixel,wheelCount:1,wheel1:dy,wheel2:0,wheel3:0)!
  e.location=CGPoint(x:x,y:y); e.post(tap:.cghidEventTap); usleep(8333)
}
