# AI dan model di MoMo

Fitur ini tersedia pada MoMo **0.4.0 untuk Windows/Linux**. Versi macOS belum memakai panel ini.

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

1. Pasang [Ollama](https://docs.ollama.com/) dan sediakan model yang cocok dengan RAM/VRAM perangkat.
2. Buka **Settings → Local AI · offline**. Isi folder Ollama yang berisi `blobs` dan `manifests`; folder `Local AI Models` di dekat project/aplikasi terdeteksi otomatis jika tersedia.
3. Klik **Check folder → Start local AI**. MoMo menjalankan server sendiri di `http://127.0.0.1:11435` dengan [OLLAMA_MODELS](https://docs.ollama.com/faq#how-do-i-set-them-to-a-different-location) menunjuk folder tersebut dan cloud dimatikan. Server Ollama lain pada port 11434 tetap dapat digunakan melalui profil manual.
4. Pilih model yang terdeteksi, klik **Add model profile → Save changes**. Profil lokal menjadi aktif tanpa API key. Sesudah restart MoMo, server dari folder tersimpan dijalankan kembali.

Pilih model kecil dahulu jika RAM/VRAM terbatas, misalnya Llama 3.2 3B. Daftar mengecualikan embedding/cloud dan manifest yang blob-nya belum lengkap. File GGUF terpisah perlu diimpor ke Ollama atau dimuat melalui LM Studio. Folder `Local AI Models` serta format model besar dikecualikan dari Git; model dan runtime tidak dibundel ke installer. Perangkat lain dapat memilih lokasi foldernya sendiri.

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

## Membuka aplikasi dan membuat catatan

Aktifkan **Settings → Computer actions → Allow AI to open apps and create notes**. Contoh: `Buka Notepad` atau `Buat catatan belanja di Notepad: beras, telur, kopi`. Di Windows, aplikasi yang tersedia adalah Notepad, Calculator, Paint dan File Explorer. Linux memakai editor/aplikasi setara yang terpasang. Catatan baru disimpan otomatis di folder data lokal MoMo (`%LOCALAPPDATA%\MoMo\notes` pada Windows), memakai UTF-8 dan tidak menimpa catatan yang sudah ada.

Model harus mendukung function/tool calling. Adapter menyediakan format tool untuk [OpenAI](https://developers.openai.com/api/docs/guides/function-calling), [Gemini](https://ai.google.dev/gemini-api/docs/function-calling), [Claude](https://platform.claude.com/docs/en/agents-and-tools/tool-use/define-tools), [Ollama](https://docs.ollama.com/capabilities/tool-calling), serta server kompatibel OpenAI. Hasil tindakan nyata muncul di chat. Jika koneksi gagal atau Stop ditekan setelah tindakan berjalan, MoMo tetap melaporkan tindakan tersebut dan tidak mencoba provider lain untuk mengulangnya. Untuk Gemini, mode tindakan memakai custom tools; pencarian Google bawaan tidak digabungkan dalam permintaan tersebut.

Fitur saat ini mencakup pembukaan aplikasi dan catatan teks. Klik mouse, pengetikan pada aplikasi lain dan navigasi desktop umum belum tersedia.

Panel chat tetap terbuka saat model dimuat atau aplikasi lain dibuka agar hasilnya dapat dibaca. Gunakan **Esc** untuk menutup atau pindah ke Overview untuk kembali ke perilaku auto-close.

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

## Optimasi GPU 8 GB dan adaptive reasoning

Untuk RTX 5060 8 GB dengan RAM sekitar 16 GB, gunakan model 3B/4B sebagai titik awal. Library lokal ini memiliki Qwen3.5 4B Q4_K_M sekitar 3,16 GiB: ada ruang lebih untuk konteks dan aplikasi lain dibanding model 9B/20B/30B. Ukuran weight bukan total kebutuhan VRAM; konteks, image encoder dan aplikasi lain juga memakai memori. Model 20B/30B dapat berjalan dengan offload CPU/RAM, tetapi respons lebih lambat dan RAM 16 GB dapat menjadi batas.

1. Pilih model pada **Local AI · offline → Add model profile**.
2. Pada profil Ollama, klik **Inspect model** untuk membaca ukuran, kuantisasi, batas konteks dan kemampuan dari server.
3. Klik **Balanced · 8 GB VRAM → Save changes**: konteks 4096, output 4096, timeout 300 detik dan keep-alive 5 menit. Sampling mengikuti default model.
4. **Adaptive · automatic** memilih thinking off/on untuk model yang mendukungnya, atau low/medium untuk model seperti GPT-OSS. Pesan pendek memakai usaha lebih rendah; analisis, debugging, matematika dan pesan panjang memakai usaha lebih tinggi. Ini kebijakan pemilihan MoMo, sehingga model yang tidak mendukung pengaturan reasoning tetap memakai defaultnya. Nilai dari metadata Ollama diprioritaskan; runtime lama memakai kontrol family yang dikenal untuk Qwen3/Qwen3.5 dan GPT-OSS.

Server yang dimiliki MoMo membatasi satu model/satu permintaan paralel, mengaktifkan Flash Attention serta cache KV q8_0 untuk mengurangi pemakaian memori. Ini hanya mengatur server MoMo pada port 11435. [Dokumentasi memori Ollama](https://docs.ollama.com/faq) dan [kontrol thinking](https://docs.ollama.com/capabilities/thinking).

History lengkap tetap tersimpan. Permintaan lokal memakai estimasi ukuran konteks untuk mempertahankan giliran terbaru; estimasi ini bukan tokenizer dan runtime tetap menerapkan batas konteksnya. Jika reasoning menghabiskan output tanpa jawaban, naikkan **Output token limit**, kurangi reasoning, atau gunakan model yang lebih ringan. First load memerlukan waktu lebih lama daripada respons sesudah model berada di memori.

## History chat

Adaptive menjadi default untuk semua profil lokal baru, termasuk Ollama dan server kompatibel OpenAI lokal. Profil versi lama tanpa pengaturan reasoning dimigrasikan ke Adaptive. Anda tetap dapat memilih Model default atau nilai manual yang didukung. Untuk LM Studio yang melaporkan level low/medium/high pada API model, MoMo memakai Responses API dan mempertahankan tool calling serta konteks percakapan. Server lama, kontrol yang belum didukung adapter, atau model tanpa thinking memakai default server. Konteks dan auto-unload LM Studio diatur saat memuat model di LM Studio. Lihat [metadata model LM Studio](https://lmstudio.ai/docs/developer/rest/list) dan [reasoning pada Responses](https://lmstudio.ai/docs/developer/openai-compat/responses).

Jika model lokal menghabiskan budget hanya untuk thinking tanpa jawaban, Adaptive boleh mencoba sekali lagi pada model yang sama dengan thinking off atau level low yang didukung. MoMo memberi label bahwa reasoning diturunkan. Pemulihan ini tidak beralih ke provider online dan tidak dilakukan setelah hasil desktop tool masuk ke permintaan, sehingga tindakan tidak diulang.

Klik **History** di chat untuk mencari dan membuka percakapan, atau **+ / New chat** untuk memulai percakapan baru. Model di kiri bisa diganti sambil melanjutkan konteks teks. Klik tombol hapus dua kali untuk menghapus satu percakapan.

Giliran yang selesai disimpan lokal di **%LOCALAPPDATA%\MoMo\chat-history.json** pada Windows, atau direktori data MoMo pada Linux. Penyimpanan dibatasi 100 percakapan, 200 pesan per percakapan dan 8 MiB; yang paling lama dibersihkan saat penuh. Teks sangat panjang dipersingkat dalam arsip. File/gambar/PDF tidak disimpan ulang di history: lampirkan kembali pada chat baru bila diperlukan. Riwayat tindakan hanya dibaca sebagai bukti hasil; membukanya tidak menjalankan tindakan kembali. History berisi teks percakapan dan disimpan terpisah dari vault API key, Git serta installer.

## Memahami integrasi

Alurnya: **token layanan → API layanan → status di pill MoMo**. MoMo membaca data secara berkala. Tombol pada kartu dapat membuka dashboard layanan. API key Gemini/OpenAI/Claude hanya untuk chat, sedangkan integrasi memakai token layanan masing-masing. Computer actions diatur terpisah.

| Layanan | Data yang ditampilkan | Interval |
| --- | --- | --- |
| GitHub | Jumlah repository dan star akun yang dapat diakses token | 5 menit |
| Vercel | Deployment terbaru dan statusnya | 30 detik |
| n8n | Status eksekusi workflow pada instance Anda | 15 detik |
| Resend | Aktivitas email yang tersedia untuk API key | 1 menit |
| Stripe | Saldo dan pembayaran terbaru | 30 detik |
| Notion | Halaman yang dibagikan kepada integration | 5 menit |
| Cal.com | Booking mendatang | 5 menit |

Contoh GitHub: buat personal access token di akun GitHub Anda dengan akses baca yang sesuai → buka **Settings → Integrations → GitHub** → isi **Token**, klik **Save** → aktifkan pill GitHub. MoMo mulai membaca API dan menampilkan jumlah repo/star. Token tidak membuat MoMo melakukan commit atau push. Untuk Notion, bagikan halaman ke integration; untuk n8n, isi URL instance dan API key. Aktifkan maksimal empat pill. Matikan toggle untuk menghentikan polling layanan tersebut; **Pause** di tray menghentikan semua poller integrasi. **How [service] works** di Settings menjelaskan setiap layanan.
