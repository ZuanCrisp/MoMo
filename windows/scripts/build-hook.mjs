import { spawnSync } from "node:child_process";

// Windows relays are copied into a separate per-user bin directory. Link the
// CRT into the executable so that directory does not need a runtime DLL.
const windows = process.platform === "win32";
const args = windows
  ? ["rustc", "--release", "--locked", "-p", "momo-hook", "--", "-C", "target-feature=+crt-static"]
  : ["build", "--release", "--locked", "-p", "momo-hook"];
const result = spawnSync(windows ? "cargo.exe" : "cargo", args, { stdio: "inherit" });
if (result.error) {
  console.error(`Could not build the MoMo relay: ${result.error.message}`);
}
process.exit(result.status ?? 1);
