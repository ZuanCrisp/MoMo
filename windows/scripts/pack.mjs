// Copies the packages Tauri buries in target/release/bundle/ into
// windows/release/, with the names they ship under. Windows gets one version
// folder containing an installer and README. Used by `npm run pack` and
// by the release workflows, so both produce exactly the same file names.

import { readFileSync, writeFileSync, mkdirSync, copyFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const targetRoot = process.env.CARGO_TARGET_DIR
  ? resolve(root, process.env.CARGO_TARGET_DIR)
  : join(root, "target");
const bundleRoot = join(targetRoot, "release", "bundle");

const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const outDir = process.platform === "win32" ? join(root, "release", version) : join(root, "release");

// What each platform ships: where Tauri puts it, how to recognise it, and the
// names it is published under (the rolling name, when there is one, always
// points at the latest release).
const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
const debArch = process.arch === "arm64" ? "arm64" : "amd64";
const windowsArch = process.arch === "arm64" ? "arm64" : "x64";
const PACKAGES = {
  win32: [
    {
      dir: "nsis",
      suffix: "-setup.exe",
      names: [`MoMo-${version}-Windows-${windowsArch}-Setup.exe`],
    },
  ],
  linux: [
    {
      dir: "appimage",
      suffix: ".AppImage",
      names: [`MoMo-Linux-${version}-${arch}.AppImage`, `MoMo-Linux-${arch}.AppImage`],
    },
    { dir: "deb", suffix: ".deb", names: [`MoMo-Linux-${version}-${debArch}.deb`] },
    { dir: "rpm", suffix: ".rpm", names: [`MoMo-Linux-${version}-${arch}.rpm`] },
  ],
};

const packages = PACKAGES[process.platform];
if (!packages) {
  console.error(`Nothing to pack on ${process.platform}.`);
  process.exit(1);
}

/** The newest file in `dir` ending with `suffix`, in case an older build is still lying around. */
function newest(dir, suffix) {
  let files = [];
  try {
    files = readdirSync(dir).filter((f) => f.endsWith(suffix));
  } catch {
    return null;
  }
  if (files.length === 0) return null;
  return files
    .map((f) => join(dir, f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
}

mkdirSync(outDir, { recursive: true });
const written = [];
for (const { dir, suffix, names } of packages) {
  const built = newest(join(bundleRoot, dir), suffix);
  if (!built) {
    console.error(`No *${suffix} in ${join(bundleRoot, dir)} — run \`npm run tauri build\` first.`);
    process.exit(1);
  }
  for (const name of names) {
    const dest = join(outDir, name);
    copyFileSync(built, dest);
    written.push(dest);
  }
}

if (process.platform === "win32") {
  const name = PACKAGES.win32[0].names[0];
  const readme = `# MoMo ${version} untuk Windows\n\n` +
    `Installer: **${name}**\n\n` +
    `Satu installer untuk Windows 10/11 ${windowsArch}. Sudah menyertakan Microsoft WebView2 Runtime, sehingga pemasangan bisa dilakukan tanpa internet jika runtime belum tersedia.\n\n` +
    `1. Salin installer ke perangkat tujuan.\n2. Klik dua kali installer dan ikuti wizard.\n3. Buka MoMo dari Start Menu.\n4. Gunakan ikon tray untuk Settings atau Quit. Arahkan mouse ke tengah bagian paling atas layar untuk membuka island.\n\n` +
    `Perangkat tujuan tidak membutuhkan Node.js, Rust, npm atau C++ Build Tools. Aplikasi dipasang untuk akun Windows saat ini.\n\n` +
    `## Mengatur AI\n\nBuka **Settings → AI & models** untuk memilih provider/model, mengisi key dan mengatur fallback. Untuk model Ollama yang sudah diunduh, buka **Local AI · offline**, pilih folder berisi blobs dan manifests, klik **Start local AI**, lalu **Add model profile → Save changes**. Runtime dan model AI disediakan terpisah; installer tidak memuat model besar atau credential pribadi. Chat/API online tetap membutuhkan internet.\n\n` +
    `## Tindakan komputer\n\nAktifkan **Computer actions → Allow AI to open apps and create notes**. Pilih model yang mendukung tool calling, lalu minta MoMo membuka Notepad, Calculator, Paint atau File Explorer, atau membuat catatan. Catatan disimpan di %LOCALAPPDATA%\\MoMo\\notes dan dibuka di Notepad. Hasil tindakan ditampilkan di chat.\n\n` +
    `## Instalasi otomatis\n\n\`\`\`powershell\n.\\${name} /S\n\`\`\`\n\n` +
    `Installer MoMo belum ditandatangani secara digital. Source dan panduan: https://github.com/ZuanCrisp/MoMo\n`;
  writeFileSync(join(outDir, "README.md"), readme);
}

console.log("\n  Packages ready\n");
for (const f of written) {
  const mb = (statSync(f).size / 1024 / 1024).toFixed(2);
  console.log(`  ${f}  (${mb} MB)`);
}
console.log();
