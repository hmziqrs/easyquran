import { describe, expect, it } from "vite-plus/test";

import { AUDIO_CATALOGUE, AUDIO_ENABLED, recordingById } from "../catalogue";

// The shipped catalogue is empty until a source is licensed (O22). The dock and
// its audio code stay dark while this holds. Update this test together with the
// catalogue when a source is added.
describe("shipped audio catalogue", () => {
  it("is empty and keeps every audio surface disabled", () => {
    expect(AUDIO_CATALOGUE.recordings).toHaveLength(0);
    expect(AUDIO_CATALOGUE.reciters).toHaveLength(0);
    expect(AUDIO_ENABLED).toBe(false);
  });

  it("resolves no recording id", () => {
    expect(recordingById("anything")).toBeUndefined();
  });
});
