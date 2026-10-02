import { stickyNav } from "$lib/stores/sticky-nav.svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

// The header-flash fix: reader anchor restores move window.scrollY without user
// intent, so Nav.svelte's collapse/expand direction logic must be able to see
// "this scroll is programmatic" and only resync its baseline. These tests pin
// the suppression contract (depth + time tail).
describe("sticky nav programmatic-scroll suppression", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("is unsuppressed by default (marketing pages keep raw behavior)", () => {
    expect(stickyNav.scrollSuppressed).toBe(false);
  });

  it("suppresses while a programmatic window is open and briefly after", () => {
    const release = stickyNav.suppressProgrammaticScroll();
    expect(stickyNav.scrollSuppressed).toBe(true);
    release();
    // Tail: queued scroll events / rAF reads that land just after the restore
    // must still be ignored.
    expect(stickyNav.scrollSuppressed).toBe(true);
    vi.advanceTimersByTime(300);
    expect(stickyNav.scrollSuppressed).toBe(false);
  });

  it("stays suppressed until every nested window is released", () => {
    const releaseOuter = stickyNav.suppressProgrammaticScroll();
    const releaseInner = stickyNav.suppressProgrammaticScroll();
    releaseInner();
    expect(stickyNav.scrollSuppressed).toBe(true);
    releaseOuter();
    vi.advanceTimersByTime(300);
    expect(stickyNav.scrollSuppressed).toBe(false);
  });

  it("treats a release function as idempotent (no negative depth escape)", () => {
    const release = stickyNav.suppressProgrammaticScroll();
    release();
    release();
    vi.advanceTimersByTime(300);
    expect(stickyNav.scrollSuppressed).toBe(false);
  });

  it("does not touch the collapsed state", () => {
    stickyNav.collapse();
    const release = stickyNav.suppressProgrammaticScroll();
    release();
    vi.advanceTimersByTime(300);
    expect(stickyNav.collapsed).toBe(true);
    stickyNav.expand();
    expect(stickyNav.collapsed).toBe(false);
  });
});
