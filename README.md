# router-bocah

Sinkronisasi otomatis daftar model dari router API (default: `ai.bocahdigital.com`) ke config **opencode**, **Claude Code**, dan **Zed**.

Router sering update daftar model (model baru, model dihapus). Daripada copas manual, jalankan satu perintah — tool fetch model terbaru dari API dan menulisnya ke config editor kamu. Backup otomatis dibuat sebelum setiap perubahan.

> Binary distribusi: `router-bocah`. Source crate: `router-sync`.

## Install

Download binary untuk platform kamu dari [latest release](https://github.com/budi-byte/router-sync/releases), lalu taruh di `PATH`.

| Platform | Asset |
|----------|-------|
| Linux x86_64 | `router-bocah-linux-amd64.tar.gz` |
| Linux ARM64 | `router-bocah-linux-arm64.tar.gz` |
| macOS Intel | `router-bocah-macos-amd64.tar.gz` |
| macOS Apple Silicon | `router-bocah-macos-arm64.tar.gz` |
| Windows x86_64 | `router-bocah-windows-amd64.exe.zip` |

```bash
# contoh macOS / Linux
tar xzf router-bocah-macos-arm64.tar.gz
chmod +x router-bocah
sudo mv router-bocah /usr/local/bin/
```

Atau build sendiri (butuh Rust):

```bash
cargo build --release
# binary: target/release/router-sync  (rename ke router-bocah kalau mau)
```

## Usage

```bash
router-bocah [OPTIONS]
```

### Opsi

| Flag | Default | Keterangan |
|------|---------|-----------|
| `--api <KEY>` | prompt interaktif / env | API key router |
| `--baseurl <URL>` | `https://ai.bocahdigital.com` | Base URL router (tanpa `/v1`) |
| `--config <PATH>` | lihat bawah | Path file config tujuan |
| `--apply` | (dry-run) | Tulis perubahan ke config. Tanpa flag ini hanya preview |
| `--claude` | — | Sync ke config **Claude Code** |
| `--zed` | — | Sync ke config **Zed** |
| (tanpa flag editor) | — | Sync ke config **opencode** |

### Config path default

| Editor | Path |
|--------|------|
| opencode | `~/.config/opencode/opencode.json` |
| Claude Code | `~/.claude/settings.json` |
| Zed | `~/.config/zed/settings.json` |

### API key

Dibaca dari (urutan): flag `--api` → env `ROUTER_BOCAH_API_KEY` → prompt interaktif.

```bash
export ROUTER_BOCAH_API_KEY="rb-xxxxxxxxxxxx"
```

## Contoh

Sync ke opencode (dry-run dulu, lalu apply):

```bash
router-bocah                          # preview perubahan
router-bocah --apply                  # tulis ke opencode.json
```

Sync ke Claude Code dengan API key spesifik:

```bash
router-bocah --claude --api rb-xxxxxxxx --apply
```

Sync ke Zed:

```bash
router-bocah --zed --apply
```

Router lain (base URL beda):

```bash
router-bocah --baseurl https://router.example.com --apply
```

## Behavior

- **Backup otomatis**: sebelum menulis, file config disalin ke `<nama>.bak.<timestamp>`.
- **opencode — auto-create entry**: kalau provider `Router-Bocah` belum ada di config, entry akan dibuat otomatis dengan `npm: @ai-sdk/openai-compatible`, `options.apiKey`, dan `options.baseURL`. Tidak perlu edit JSON manual.
- **Setiap sync me-refresh** `apiKey` dan `baseURL` di provider `Router-Bocah` — ganti key cukup jalankan ulang.
- **Diff preview**: tanpa `--apply`, tool menampilkan model yang akan ditambah / dihapus / tidak berubah.

## Build

Cross-platform build via GitHub Actions (trigger tag `v*`). Lihat [`.github/workflows/release.yml`](.github/workflows/release.yml).

```bash
cargo build --release
```

## License

MIT
