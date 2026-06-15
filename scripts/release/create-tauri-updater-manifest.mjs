#!/usr/bin/env node

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { parseReleaseVersion } from "./release-version.mjs";

const [assetDir, outputPath, notesPath] = process.argv.slice(2);

if (!assetDir || !outputPath) {
  console.error(
    "Usage: create-tauri-updater-manifest.mjs <asset-dir> <output-path> [notes-file]",
  );
  process.exit(1);
}

let releaseVersion;
try {
  releaseVersion = parseReleaseVersion(
    process.env.CRATEBAY_RELEASE_TAG || process.env.RELEASE_TAG,
  );
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}

const releaseTag = releaseVersion.releaseTag;
const repository = (
  process.env.CRATEBAY_UPDATE_REPOSITORY ||
  process.env.GITHUB_REPOSITORY ||
  ""
).trim();

if (!repository) {
  console.error("CRATEBAY_UPDATE_REPOSITORY or GITHUB_REPOSITORY is required.");
  process.exit(1);
}

const files = new Set(readdirSync(assetDir));
const platforms = {};

function targetForArtifact(filename) {
  if (/macOS-x64\.app\.tar\.gz$/i.test(filename)) return "darwin-x86_64";
  if (/macOS-x86_64\.app\.tar\.gz$/i.test(filename)) return "darwin-x86_64";
  if (/macOS-aarch64\.app\.tar\.gz$/i.test(filename)) return "darwin-aarch64";
  if (/Windows-x64-Setup\.exe$/i.test(filename)) return "windows-x86_64";
  if (/Windows-x86_64-Setup\.exe$/i.test(filename)) return "windows-x86_64";
  if (/Windows-x64\.msi$/i.test(filename)) return "windows-x86_64-msi";
  if (/Windows-x86_64\.msi$/i.test(filename)) return "windows-x86_64-msi";
  if (/Linux-x86_64\.AppImage$/i.test(filename)) return "linux-x86_64";
  if (/Linux-x86_64\.deb$/i.test(filename)) return "linux-x86_64-deb";
  if (/Linux-x86_64\.rpm$/i.test(filename)) return "linux-x86_64-rpm";
  return null;
}

function releaseAssetUrl(filename) {
  return `https://github.com/${repository}/releases/download/${encodeURIComponent(releaseTag)}/${encodeURIComponent(filename)}`;
}

function releaseNotes() {
  if (!notesPath) return `CrateBay ${releaseTag}`;
  const notes = readFileSync(notesPath, "utf8").trim();
  return notes || `CrateBay ${releaseTag}`;
}

for (const file of files) {
  if (!file.endsWith(".sig")) continue;

  const artifact = basename(file.slice(0, -".sig".length));
  if (!files.has(artifact)) continue;

  const target = targetForArtifact(artifact);
  if (!target) continue;

  const signature = readFileSync(join(assetDir, file), "utf8").trim();
  if (!signature) {
    console.error(`Signature file is empty: ${file}`);
    process.exit(1);
  }

  platforms[target] = {
    signature,
    url: releaseAssetUrl(artifact),
  };

  if (target === "darwin-x86_64") {
    platforms["darwin-x86_64-app"] = platforms[target];
  } else if (target === "darwin-aarch64") {
    platforms["darwin-aarch64-app"] = platforms[target];
  } else if (target === "windows-x86_64") {
    platforms["windows-x86_64-nsis"] = platforms[target];
  } else if (target === "linux-x86_64") {
    platforms["linux-x86_64-appimage"] = platforms[target];
  }
}

if (Object.keys(platforms).length === 0) {
  console.error("No updater artifacts with matching .sig files were found.");
  process.exit(1);
}

const manifest = {
  version: releaseVersion.appVersion,
  notes: releaseNotes(),
  pub_date: new Date().toISOString(),
  platforms,
};

writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
  `Wrote updater manifest with ${Object.keys(platforms).length} platform entries: ${outputPath}`,
);
