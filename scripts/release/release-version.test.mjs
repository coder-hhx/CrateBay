import assert from "node:assert/strict";
import test from "node:test";
import { parseReleaseVersion, tauriVersionConfig } from "./release-version.mjs";

test("parses stable v-prefixed semver tags", () => {
  assert.deepEqual(parseReleaseVersion("refs/tags/v1.2.3"), {
    appVersion: "1.2.3",
    isPrerelease: false,
    releaseTag: "v1.2.3",
  });
});

test("parses prerelease tags", () => {
  assert.deepEqual(parseReleaseVersion("v1.2.3-beta.1"), {
    appVersion: "1.2.3-beta.1",
    isPrerelease: true,
    releaseTag: "v1.2.3-beta.1",
  });
});

test("rejects non-v tags", () => {
  assert.throws(() => parseReleaseVersion("1.2.3"), /must start with "v"/);
});

test("builds Tauri version overlay", () => {
  assert.deepEqual(tauriVersionConfig("1.2.3"), { version: "1.2.3" });
});
