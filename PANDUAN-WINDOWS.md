# MoMo untuk Windows

## Memasang aplikasi

Gunakan Windows 10/11 64-bit (x64). Setiap versi disediakan dalam satu folder yang berisi **satu installer dan README.md**. Installer menyertakan Microsoft WebView2 Runtime agar pemasangan bisa dilakukan tanpa internet jika runtime belum terpasang.

1. Salin installer ke perangkat tujuan.
2. Klik dua kali file installer dan ikuti wizard.
3. Buka **MoMo** dari Start Menu.
4. Arahkan mouse ke tengah bagian paling atas layar. Gunakan ikon tray untuk Settings atau Quit.

Perangkat tujuan tidak perlu Node.js, npm, Rust, Cargo atau C++ Build Tools. Instalasi berlaku untuk akun Windows saat ini. AI online membutuhkan internet dan key per perangkat; AI lokal memerlukan runtime dan model yang sudah tersedia di perangkat.

Installer MoMo belum ditandatangani secara digital. Pemasangan diuji pada perangkat pengembangan yang memiliki WebView2; pemasangan runtime pada perangkat bersih tanpa WebView2 belum diuji.

## Mengatur AI

Buka **Settings → AI & models** untuk memilih provider/model, key utama/cadangan, atau server model lokal. Ikuti [panduan AI](docs/AI-MODELS.md). Installer tidak memuat credential pribadi.

## Membuat installer dari source

Pasang Node.js 22, Rust stable MSVC, C++ Build Tools dan WebView2 sesuai [prasyarat Tauri](https://v2.tauri.app/start/prerequisites/#windows). Buka terminal baru setelah pemasangan agar PATH diperbarui.

Dari root repository:

```powershell
cd windows
npm.cmd ci
npm.cmd run pack
```

Hasil build:

```text
windows/release/0.2.0/
  MoMo-0.2.0-Windows-x64-Setup.exe
  README.md
```

Nomor versi mengikuti konfigurasi source. Build membutuhkan internet untuk mengunduh runtime WebView2 yang dimasukkan ke installer. Installer hasil build tidak dimasukkan ke Git.

Jika tersedia toolchain portable dalam `.tools/` pada root repository, ganti `npm.cmd` dengan `.\npm-momo.cmd`. Helper ini juga dapat memakai toolchain yang sudah ada di PATH.

Alternatif: buka **Actions → Windows → Run workflow** di GitHub, lalu unduh artifact setelah build berhasil.

## Instalasi otomatis

Dari folder installer:

```powershell
.\MoMo-0.2.0-Windows-x64-Setup.exe /S
```

## Menghapus aplikasi

Jika pernah memasang hook integrasi, gunakan **Uninstall hooks** di pengaturan MoMo terlebih dahulu. Kemudian buka **Windows Settings → Apps → MoMo → Uninstall**.

## Lisensi source

Kode mengikuti [LICENSE](LICENSE). Karakter, ikon, animasi, suara dan media asli mengikuti [LICENSE-ASSETS.md](LICENSE-ASSETS.md). Pemberitahuan lisensi disertakan di dalam aplikasi.
