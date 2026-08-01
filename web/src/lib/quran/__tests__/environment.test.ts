import { describe, expect, it } from "vite-plus/test";
import {
  DEFAULT_QURAN_R2_BASE,
  LOCAL_QURAN_ARTIFACT_BASE,
  QuranDataEnvironment,
  resolveQuranArtifactBase,
  resolveQuranDataEnvironment,
} from "../environment";

describe("Quran data environment", () => {
  it("uses the requested environment or its caller-supplied fallback", () => {
    expect(resolveQuranDataEnvironment("local", QuranDataEnvironment.Production)).toBe(
      QuranDataEnvironment.Local,
    );
    expect(resolveQuranDataEnvironment(undefined, QuranDataEnvironment.Production)).toBe(
      QuranDataEnvironment.Production,
    );
  });

  it("rejects ambiguous source selections", () => {
    expect(() => resolveQuranDataEnvironment("staging", QuranDataEnvironment.Local)).toThrow(
      "PUBLIC_ENV",
    );
  });

  it("uses local files or R2", () => {
    expect(resolveQuranArtifactBase(QuranDataEnvironment.Local)).toBe(LOCAL_QURAN_ARTIFACT_BASE);
    expect(resolveQuranArtifactBase(QuranDataEnvironment.Production)).toBe(DEFAULT_QURAN_R2_BASE);
  });
});
