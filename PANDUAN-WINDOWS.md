# MoMo untuk Windows

## Memasang aplikasi

Gunakan Windows 10/11 64-bit (x64). Pilih installer standar jika perangkat memiliki internet, atau varian offline jika WebView2 perlu dipasang tanpa internet.

1. Salin installer ke perangkat tujuan.
2. Klik dua kali file installer dan ikuti wizard.
3. Buka **MoMo** dari Start Menu.
4. Arahkan mouse ke tengah bagian paling atas layar. Gunakan ikon tray untuk Settings atau Quit.

Perangkat tujuan tidak perlu memasang Node.js, npm, Rust, Cargo atau C++ Build Tools. Instalasi berlaku untuk akun Windows saat ini. Chat/API dan integrasi online tetap memerlukan internet serta konfigurasi per perangkat.

Installer MoMo belum ditandatangani secara digital. Installer standar sudah diuji pada perangkat pengembangan; varian offline belum diuji pada perangkat bersih tanpa WebView2.

## Membuat installer dari source

Pasang Node.js 22, Rust stable MSVC, C++ Build Tools dan WebView2 sesuai [prasyarat Tauri](https://v2.tauri.app/start/prerequisites/#windows). Buka terminal baru setelah memasangnya agar PATH diperbarui.

Dari root repository:

```powershell
cd windows
npm.cmd ci
npm.cmd run pack
npm.cmd run pack:offline
```

Hasil berada di `windows/release/`:

- `MoMo-Windows-0.1.1-setup.exe`: installer standar.
- `MoMo-Windows-0.1.1-x64-offline-setup.exe`: menyertakan installer resmi Microsoft WebView2.

Nomor versi mengikuti konfigurasi source. Build offline membutuhkan internet ketika mengunduh runtime untuk dimasukkan ke installer. File installer hasil build tidak dimasukkan ke Git.

Jika tersedia toolchain portable dalam `.tools/` pada root repository, ganti `npm.cmd` dengan `.\npm-momo.cmd`. Helper ini juga dapat memakai toolchain yang sudah ada di PATH.

Alternatif: buka **Actions → Windows → Run workflow** di GitHub. Aktifkan opsi offline jika diperlukan, lalu unduh artifact setelah build berhasil.

## Instalasi otomatis

Dari folder hasil build:

```powershell
.\MoMo-Windows-0.1.1-x64-offline-setup.exe /S
```

## Menghapus aplikasi

Jika pernah memasang hook integrasi, gunakan **Uninstall hooks** di pengaturan MoMo terlebih dahulu. Kemudian buka **Windows Settings → Apps → MoMo → Uninstall**.

## Lisensi

Kode mengikuti [LICENSE](LICENSE). Karakter, ikon, animasi, suara dan media asli mengikuti [LICENSE-ASSETS.md](LICENSE-ASSETS.md); ketentuan aset tersebut tetap berlaku setelah rebranding. Kedua pemberitahuan lisensi disertakan dalam installer.
