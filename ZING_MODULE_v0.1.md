# Zing MP3 module — v0.1 (fork of ytm-rs 0.3.4 → crate 0.3.5)

Adds a third-party **Zing MP3** source to ytm-rs next to YouTube Music:
a "Zing MP3" page in the sidebar with search and playback through the app's
existing audio engine and queue, plus autoplay of similar songs. YouTube
Music behaviour is unchanged. GPL-3.0-only, like the upstream project.

## What was added

| File | Change |
|---|---|
| `src/zing.rs` | New. The Zing web-client API client (signing, search, streaming, similar-songs chain) and the Zing→`model::Track` mapping. |
| `src/main.rs` | `mod zing;` |
| `src/backend.rs` | `Req::ZingSearch` / `Req::ZingAutoplay` (+ `Resp::ZingSearch` / `Resp::ZingAutoplay`); a `zing:` branch at the top of `start_playback`; Zing prefetch cache; guards so `zing:` ids never reach YouTube-only actions. |
| `src/ui/mod.rs` | `Page::Zing`; the Zing page's state (`ZingState`) on `App`. |
| `src/ui/shell.rs` | "Zing MP3" sidebar entry (music-note icon), between Library and Listen together. |
| `src/ui/pages.rs` | The Zing page: search field + button, status line, one simple row per result; a click plays the song. |
| `src/ui/playback.rs` | `App::play_zing`: queue = the clicked track, then `Req::ZingAutoplay` is fired. |
| `src/ui/responses.rs` | `Resp::ZingSearch` fills the page; `Resp::ZingAutoplay` appends via `Queue::add_radio` (same dedupe path as YouTube radio). |
| `Cargo.toml` | Version 0.3.4 → 0.3.5. No new dependencies (`reqwest`, `sha2`, `hmac`, `serde_json` were already there). |

## How the Zing client works

The module talks to the Zing web client's internal API — the same one the
official Zing desktop app uses:

- `GET https://zingmp3.vn/` once; the `zmp3_app_version.1` / `zmp3_rqid`
  cookies are captured from `Set-Cookie` and sent on API calls (reqwest's
  cookie store is not enabled, so this is done manually).
- Every call is signed: `sig = HMAC-SHA512(SECRET, path + SHA256_HEX(hash_input))`,
  with query params `ctime` (unix **seconds**), `version` (`1.6.34`), `apiKey`
  and `sig`. The `ctime` inside `hash_input` must be the same value as the
  `ctime` parameter, otherwise the service replies err -104
  "Incorect signature". Hash inputs: search uses `ctime={ctime}version={VERSION}`;
  the `id` endpoints use `ctime={ctime}id={id}version={VERSION}`.
  The apiKey/secret are the web client's own, as embedded in the public
  open-source libraries implementing this client (Whoant/ZingMp3API,
  zatamod/zingmp3).
- Endpoints: `/api/v2/search/multi`, `/api/v2/song/get/streaming`,
  `/api/v2/song/get/section-bottom`, `/api/v2/page/get/playlist`.
  Responses are `{err, msg, data}` envelopes; `err != 0` is an error whose
  `msg` is shown to the user.
- A Zing song becomes a `Track` with id **`zing:<encodeId>`** — the namespace
  prefix is what routes playback and keeps Zing ids out of every YouTube
  code path.

## Playback and autoplay

- `Req::Play` for a `zing:` id branches at the top of
  `Backend::start_playback`: the stream URL is resolved via
  `song/get/streaming` (preferring the `128` quality), then the file is fed
  to the existing machinery as `stream::Media::Feed` (proper `Range` header;
  the buffer is sized from a `HEAD` request, with a whole-file in-memory
  fallback if the CDN gives no length) and decoded by the same
  `stream::decoder` / rodio path as everything else. Generation handling
  mirrors the YouTube tail exactly; YouTube-specific extras (loudness, play
  reporting, song-version lookup) do not apply.
- A Zing play error (e.g. Zing's geo/VIP refusal) reaches the UI as
  `Resp::PlayError` carrying Zing's own message.
- `Req::Prefetch` for a `zing:` id only resolves and caches the stream URL
  in a small backend map that the play branch consults first.
- Autoplay chain (verified live): seed song → `song/get/section-bottom` →
  first playlist of the first section that has items →
  `page/get/playlist` → its songs (deduped, seed dropped) are appended to
  the queue, so when a song ends the next similar one plays. It is
  best-effort: any failure yields an empty extension, never an error.
  The queue's own `top_up` also benefits: a `Req::Radio` seeded with a
  `zing:` id is answered with the same chain instead of YouTube radio.
- Guards: like/dislike/remove-rating/song-rating/download/music-video/
  canvas/segments requests for a `zing:` id are answered gracefully
  (failed/no-op responses or a "not supported yet" message) and never reach
  the YouTube API.

## Build (Windows)

```bat
:: Rust (rustup, MSVC toolchain) + Visual Studio Build Tools (C++ workload)
cargo build --release
:: → target\release\ytm-rs.exe
```

The WebView2 runtime is only needed for the Google sign-in window
(YouTube Music); the Zing page works without it. No extra system libraries
are required for the Zing module itself.

## Known limitations (v0.1)

- **Streaming may be geo- or account-restricted.** From a non-Vietnam IP the
  service answers err **-1110** for most songs; that message is shown as the
  play error. Search/metadata are not region-locked in the same way.
- No Zing sign-in: personal library, liked songs and VIP qualities
  (320 kbps / lossless) are not available; playback uses the anonymous
  web-client stream (128 kbps where offered).
- No downloads, likes, lyrics or comments for Zing tracks (guarded, see
  above). Lyrics fetched by title/artist from the app's usual external
  sources may still appear.
- The Zing result list is deliberately simple (one text row per song, no
  covers/context menus yet), and the autoplay list comes from Zing's own
  section playlists — its size and relevance are whatever Zing returns.
- The unofficial API can change without notice; if Zing rotates the web
  client's version string or keys, `src/zing.rs` constants need updating.
