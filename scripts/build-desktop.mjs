#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const platforms = {
  windows: {
    host: "win32",
    bundles: "nsis,msi",
    formats: ".exe installer, .msi",
  },
  macos: { host: "darwin", bundles: "app,dmg", formats: ".app, .dmg" },
  linux: { host: "linux", bundles: "deb,appimage", formats: ".deb, .AppImage" },
};

export function buildPlan(argv, host = process.platform) {
  const args = [...argv];
  const native = Object.keys(platforms).find(
    (name) => platforms[name].host === host,
  );
  const requested = args[0] && !args[0].startsWith("-") ? args.shift() : native;
  const platform = platforms[requested];
  if (!platform) throw new Error("Supported platforms: windows, macos, linux.");
  if (platform.host !== host) {
    throw new Error(
      `Build ${requested} on ${requested}, or use the GitHub Actions “Build installers” workflow to build all platforms.`,
    );
  }
  const targetIndex = args.indexOf("--target");
  const target =
    targetIndex >= 0
      ? args[targetIndex + 1]
      : args.find((a) => a.startsWith("--target="))?.slice(9);
  if (targetIndex >= 0 && (!target || target.startsWith("-")))
    throw new Error("--target requires a Rust target triple.");
  if (target) {
    const targetPlatform = target.includes("windows")
      ? "windows"
      : target.endsWith("apple-darwin")
        ? "macos"
        : target.includes("linux")
          ? "linux"
          : undefined;
    if (targetPlatform !== requested)
      throw new Error(
        "The Rust target must match the selected operating system. Use GitHub Actions for other operating systems.",
      );
  }
  return {
    platform: requested,
    formats: platform.formats,
    args: ["build", "--bundles", platform.bundles, ...args],
    cwd: join(root, "apps", "desktop"),
  };
}

export function main(argv = process.argv.slice(2)) {
  if (argv.includes("--help") || argv.includes("-h")) {
    console.log(`Build native executables and installers.

  npm run build:desktop                    Build for the current OS
  npm run build:windows                    Windows: .exe installer + .msi
  npm run build:macos                      macOS: .app + .dmg
  npm run build:linux                      Linux: .deb + .AppImage
  npm run build:macos -- --target aarch64-apple-darwin

Run npm ci and install the platform's Tauri/Rust prerequisites first.
Each local command requires the matching operating system.
Use GitHub Actions > Build installers to build all OSes from any computer.
Additional arguments are forwarded to Tauri. --dry-run prints the build plan.
Default output: target/release/bundle/ (target/<triple>/release/bundle/ with --target).`);
    return 0;
  }
  try {
    const dryRun = argv.includes("--dry-run");
    const plan = buildPlan(argv.filter((arg) => arg !== "--dry-run"));
    console.log(`Building ${plan.platform}: ${plan.formats}`);
    if (dryRun) {
      console.log(JSON.stringify(plan, null, 2));
      return 0;
    }
    const require = createRequire(import.meta.url);
    let cli;
    try {
      const manifest = require.resolve("@tauri-apps/cli/package.json");
      const pkg = JSON.parse(readFileSync(manifest, "utf8"));
      cli = resolve(
        dirname(manifest),
        typeof pkg.bin === "string" ? pkg.bin : pkg.bin.tauri,
      );
    } catch {
      throw new Error(
        "The local Tauri CLI is missing. Run npm ci at the repository root first.",
      );
    }
    const env = { ...process.env };
    // Ad-hoc signatures permit development builds on Apple Silicon. A configured
    // Developer ID identity takes precedence; no certificate is bundled here.
    if (plan.platform === "macos" && !env.APPLE_SIGNING_IDENTITY)
      env.APPLE_SIGNING_IDENTITY = "-";
    const result = spawnSync(process.execPath, [cli, ...plan.args], {
      cwd: plan.cwd,
      env,
      stdio: "inherit",
      shell: false,
    });
    if (result.error)
      throw new Error(`Could not start Tauri: ${result.error.message}`);
    if (result.signal)
      throw new Error(`Build interrupted by ${result.signal}.`);
    if (result.status !== 0) return result.status ?? 1;
    console.log(
      "Build complete. Tauri lists the executable and bundle paths above.",
    );
    return 0;
  } catch (error) {
    console.error(
      error instanceof Error ? error.message : "Desktop build failed.",
    );
    return 1;
  }
}

if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  process.exitCode = main();
}
