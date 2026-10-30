import { strict as assert } from "node:assert";
import { test } from "node:test";
import { buildPlan } from "./build-desktop.mjs";

test("refuses a different OS instead of implying unsupported cross-compilation", () => {
  assert.throws(() => buildPlan(["windows"], "linux"), /GitHub Actions/);
  assert.throws(() => buildPlan(["linux"], "darwin"), /GitHub Actions/);
});

test("forwards a macOS architecture target without shell interpolation", () => {
  const plan = buildPlan(
    [
      "macos",
      "--target",
      "x86_64-apple-darwin",
      "--config",
      "a path with spaces.json",
    ],
    "darwin",
  );
  assert.deepEqual(plan.args.slice(-4), [
    "--target",
    "x86_64-apple-darwin",
    "--config",
    "a path with spaces.json",
  ]);
  assert.ok(plan.args.includes("app,dmg"));
});

test("rejects cross-OS and missing target arguments before building", () => {
  assert.throws(
    () => buildPlan(["linux", "--target=x86_64-pc-windows-msvc"], "linux"),
    /must match/,
  );
  assert.throws(() => buildPlan(["macos", "--target"], "darwin"), /requires/);
});

test("native command chooses the current host's bundles", () => {
  assert.equal(buildPlan([], "win32").platform, "windows");
  assert.ok(buildPlan([], "linux").args.includes("deb,appimage"));
  assert.throws(() => buildPlan([], "freebsd"), /Supported platforms/);
});
