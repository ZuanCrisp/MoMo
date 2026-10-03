# AI dan model di MoMo

Fitur ini tersedia pada MoMo **0.2.0 untuk Windows/Linux**. Integrasi Claude Code tetap di bagian pengaturan tersendiri. Versi macOS belum memakai panel ini.

## Memilih AI online

1. Buka ikon tray **MoMo → Settings → AI & models**.
2. Klik **Add profile**. Isi nama yang mudah dikenali, misalnya `Gemini kerja`.
3. Pilih provider: **Gemini**, **OpenAI / ChatGPT**, **Claude**, atau **Other online API**.
4. Tempel satu API key pada setiap baris. Gunakan **Add API key** untuk key cadangan dan beri label seperti `Utama` atau `Cadangan pribadi`.
5. Klik **Load models**, pilih model yang tersedia, atau isi **Model ID** secara manual. Daftar model berasal dari provider, sehingga tidak bergantung pada daftar model yang ditanam di aplikasi. Akses model, batas output, fitur pencarian, dan biaya mengikuti akun/provider Anda.
6. Klik **Use as active model**, lalu **Save changes**.

Untuk API lain, server harus mendukung API OpenAI Chat Completions (`POST /chat/completions`) dan biasanya model discovery (`GET /models`). Isi **API base URL** seperti `https://api.provider.example/v1`. Jika provider tidak mendukung daftar model, isi ID model secara manual. Protokol khusus selain yang tersedia di menu memerlukan adapter tambahan.

Pilihan **OpenAI / ChatGPT** memakai API OpenAI dengan API key. Login atau langganan aplikasi ChatGPT tidak menggantikan konfigurasi API di MoMo. Protokol OpenAI memakai [Responses API](https://developers.openai.com/api/docs/guides/text); Gemini memakai [GenerateContent](https://ai.google.dev/api/generate-content); Claude memakai [Messages API](https://platform.claude.com/docs/en/api/messages/create).

## Model lokal / offline

MoMo terhubung ke server model yang berjalan pada perangkat. MoMo tidak memasang runtime atau mengunduh model secara otomatis. Sesudah runtime dan model tersedia, chat lokal dapat dipakai tanpa internet.

**Ollama**

1. Pasang [Ollama](https://docs.ollama.com/) dan unduh model yang cocok dengan RAM/VRAM perangkat, misalnya model keluarga Llama.
2. Jalankan server Ollama, lalu tambah profil **Ollama · local** di MoMo.
3. Biarkan URL `http://127.0.0.1:11434`, klik **Load models**, dan pilih model yang sudah diunduh. Model cloud Ollama tidak ditawarkan pada profil lokal.
4. API key biasanya tidak diperlukan. Klik **Use as active model → Save changes**.

**LM Studio / llama.cpp / server lokal yang kompatibel dengan OpenAI**

1. Unduh dan muat model dalam runtime pilihan, lalu nyalakan server API lokalnya.
2. Tambah profil **LM Studio / local OpenAI API**.
3. Isi URL sesuai server. Default LM Studio adalah `http://127.0.0.1:1234/v1`. Pilih model melalui **Load models** atau isi ID sesuai server.
4. Jika server memakai autentikasi, tambahkan key. Jika tidak, biarkan daftar key kosong.

Lihat [Ollama Chat API](https://docs.ollama.com/api/chat) dan [API kompatibel OpenAI di LM Studio](https://lmstudio.ai/docs/developer/openai-compat). Profil lokal hanya menerima alamat localhost/loopback. Kecepatan dan kemampuan model mengikuti runtime serta perangkat Anda; naikkan **Advanced → Timeout** jika pemuatan model lambat.

## Key cadangan dan fallback model

Setiap profil mewakili satu provider, model, dan daftar key. Buat profil terpisah untuk beberapa model pada provider yang sama; key dapat dimasukkan pada masing-masing profil.

- **Key fallback:** key dicoba dari atas ke bawah ketika terjadi kegagalan koneksi, timeout, key ditolak, kuota/billing, atau gangguan server. Tombol panah mengatur urutan. Baris kosong baru harus diisi atau dihapus sebelum disimpan.
- **Model fallback:** buka **Fallback order**, aktifkan **Automatic model fallback**, centang profil tujuan, lalu atur urutannya. Fitur ini mati secara default. Setelah key pada profil utama habis dicoba, MoMo mencoba profil cadangan yang dipilih.
- Profil dengan model/endpoint tidak tersedia atau lampiran yang tidak didukung dapat dilewati menuju profil cadangan yang dipilih. Permintaan salah dan penolakan konten dari provider menghentikan permintaan.
- Seluruh profil fallback menerima percakapan dan lampiran yang sama. Jika profil lokal memiliki fallback online yang aktif, percakapan dapat dikirim ke provider online saat lokal gagal; panel menampilkan pemberitahuan ini. Biarkan fallback mati atau pilih hanya profil lokal untuk penggunaan lokal saja.

Pemilih di bagian atas chat mengganti profil aktif yang sudah disimpan tanpa menghapus percakapan. Profil yang baru menjadi aktif dikeluarkan dari daftar fallback. Jawaban menampilkan profil/model yang benar-benar menjawab, dengan tanda `fallback` bila key/model cadangan dipakai. Tombol **Stop** membatalkan permintaan. Pesan yang gagal dikembalikan ke kolom input agar mudah dicoba lagi.

## Penyimpanan dan lampiran

Key disimpan di **Windows Credential Manager** atau **Linux Secret Service**, terpisah dari file pengaturan. Linux memerlukan layanan keyring yang aktif dan terbuka pada sesi pengguna. Nilai key tersimpan tidak dibaca kembali ke UI; kolom password hanya menampilkan nilai baru yang Anda masukkan. Profil/key tidak disalin ke perangkat lain bersama installer.

Perubahan ditampung sampai **Save changes**. **Discard** membatalkan draft. Mengganti provider/server meminta key baru; menghapus key/profil menghapus credential milik profil tersebut ketika perubahan disimpan. Kegagalan penyimpanan mencoba memulihkan konfigurasi dan credential sebelumnya.

Lampiran gambar/PDF maksimal 16 MiB dan teks UTF-8 maksimal 200 KB. PDF didukung oleh adapter Claude, Gemini, dan OpenAI; adapter lainnya menerima teks/gambar. Model yang dipilih tetap harus mendukung jenis lampiran tersebut. Pencarian web opsional tersedia untuk provider bawaan yang mendukungnya dan bergantung pada kemampuan model/akun.

## Pengujian

Pengujian Rust memakai server HTTP lokal dan vault dalam memori untuk fallback, pembatalan, protokol, migrasi pengaturan, serta rollback tanpa menggunakan credential pribadi. Pengujian Playwright memakai IPC simulasi untuk formulir, tampilan sempit, dan chat. Pengujian ini tidak memakai API key berbayar atau menjalankan inference model sungguhan.

```powershell
cd windows
npm.cmd ci
npm.cmd run build
npx.cmd playwright install chromium
npm.cmd run test:ui
cargo fmt --all -- --check
cargo test --workspace --release --locked
```
