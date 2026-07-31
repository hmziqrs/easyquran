import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";
import { trailingDebounce } from "../debounce";

describe("trailingDebounce", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("does not run immediately on schedule", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.schedule();
    expect(fn).not.toHaveBeenCalled();
    expect(d.pending).toBe(true);
  });

  it("runs once after the trailing window", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.schedule();
    vi.advanceTimersByTime(399);
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(d.pending).toBe(false);
  });

  it("coalesces repeated schedules into one trailing run", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.schedule();
    vi.advanceTimersByTime(200);
    d.schedule();
    vi.advanceTimersByTime(200);
    d.schedule();
    vi.advanceTimersByTime(200);
    d.schedule();
    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(400);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("flush() runs immediately and clears the timer", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.schedule();
    vi.advanceTimersByTime(50);
    d.flush();
    expect(fn).toHaveBeenCalledTimes(1);
    expect(d.pending).toBe(false);
    // No second run from the now-cleared timer.
    vi.advanceTimersByTime(1000);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("flush() is a no-op when nothing is pending", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.flush();
    expect(fn).not.toHaveBeenCalled();
  });

  it("cancel() drops the pending run", () => {
    const fn = vi.fn();
    const d = trailingDebounce(fn, 400);
    d.schedule();
    d.cancel();
    expect(d.pending).toBe(false);
    vi.advanceTimersByTime(1000);
    expect(fn).not.toHaveBeenCalled();
  });
});
