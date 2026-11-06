import type { DeviceKind, ReadingThread } from "./schema";

/**
 * Where a thread was last moved, from this install's point of view (devices.md D5).
 * Two phones are two devices: the comparison is by device id. Only a row that predates
 * device ids (no `deviceId`) falls back to comparing the coarse kind.
 */

export interface HereDevice {
  readonly deviceId: string | null;
  readonly kind: DeviceKind;
}

export type HandoffPlace =
  | { readonly kind: "here" }
  | { readonly kind: "named"; readonly name: string }
  | { readonly kind: "device"; readonly device: DeviceKind };

export function isThreadHere(thread: ReadingThread, here: HereDevice): boolean {
  if (thread.deviceId !== null && here.deviceId !== null) return thread.deviceId === here.deviceId;
  return thread.device === here.kind;
}

/**
 * The label for a thread last moved elsewhere: the device's own name when the server
 * resolved one, else the device kind ("on your phone"). A removed device has no name.
 */
export function handoffPlace(thread: ReadingThread, here: HereDevice): HandoffPlace {
  if (isThreadHere(thread, here)) return { kind: "here" };
  if (thread.deviceName !== null) return { kind: "named", name: thread.deviceName };
  return { kind: "device", device: thread.device };
}
