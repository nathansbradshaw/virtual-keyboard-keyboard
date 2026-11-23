<p align="center">
  <img src="assets/k2-logo-v2.png" alt="Virtual Keyboard Keyboard logo" width="160" />
</p>

# Virtual Keyboard Keyboard

[Open Virtual Keyboard Keyboard](https://nathansbradshaw.github.io/virtual-keyboard-keyboard/)

<a href="https://0rwx.com/">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/NamtaoProductions/0rwx.com/main/static/AI-W-dark.png" />
    <img src="https://raw.githubusercontent.com/NamtaoProductions/0rwx.com/main/static/AI-W-light.png" alt="AI:W — AI collaboration (0rwx)" width="80" />
  </picture>
</a>

Virtual Keyboard Keyboard (formerly K2 MIDI Viewer) renders MIDI files across a full
computer-keyboard layout with transport, track muting, pitch controls, staff
notation, and a built-in soft synth.

## Run locally

Native app:

```sh
cargo run --release
```

Browser app:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
trunk serve
```

`trunk serve` uses the LTO-optimized release profile by default so the browser
does not have to compile the much larger debug WASM. During rapid code
iteration, opt back into faster rebuilds explicitly:

```sh
trunk serve --release=false
```

Then open <http://localhost:8080>. Browser audio starts after the first Play or
keyboard interaction because Web Audio requires a user gesture.

The browser release uses a size-optimized Cargo profile, full-resolution WebP textures, and reuses keyboard
geometry between note changes. Stopped playback sleeps unless Web MIDI needs
polling. Startup failures show a reload link instead of leaving a blank page.
Oversized playback backlogs resynchronize to the current position, restoring
up to 128 held notes instead of replaying stale hits. Seeking and resuming use
the same held-note limit. Ordinary playback retains
its original event ordering. Selection summaries and badges stay compact even
for long passages; the complete selected sequence remains available internally.
Extremely dense passages limit the staff to 1,024 visible notes and show a
notice when additional notes are hidden. This drawing limit does not remove
notes from the file or change ordinary playback.
MIDI loading streams events and yields to the browser while parsing, sorting,
pairing notes, and preparing key mappings. **Cancel load** keeps the current
song, and a cancelled request cannot replace a newer one. Browser audio starts
on a musical interaction, so choosing a file does not start its audio scheduler.
Interval indexing also avoids scanning old notes when a long sustain
spans the current view.

To compare staff note lookup against a full scan on a 100,000-note fixture:

```sh
cargo test measure_staff_note_lookup -- --ignored --nocapture
```

## Brand assets

The full-size logo is [`assets/k2-logo-v2.png`](assets/k2-logo-v2.png). Small
browser and desktop icons use the simplified
[`assets/k2-micro-mark.png`](assets/k2-micro-mark.png), which keeps `K²`
legible down to 16 pixels. Platform-ready `.ico` and `.icns` files are in
[`assets/desktop`](assets/desktop).

## Web MIDI

Desktop Chrome and Firefox support connected MIDI hardware. Click **Connect
MIDI**, grant the browser permission, then use the independent **MIDI IN** and
**MIDI OUT** buttons to cycle through available ports or turn either side off.
Web MIDI requires HTTPS in production; localhost is accepted for development.

Set **IN OCT** to the same value as **Settings → Octave** on the physical
Keyboard Keyboard. The viewer defaults to `2`, matching the hardware firmware,
and uses this only to locate the originating virtual key; MIDI audio keeps the
pitch sent by the hardware. The selection is saved in the page URL, so another
octave setting can be kept for future sessions or shared setups.

For a live hardware demonstration, select both a **MIDI IN** and **MIDI OUT**,
then enable **THRU**. Every MIDI message received from the input is forwarded
unchanged to the output while the virtual board continues to show incoming
notes. Turning THRU off or changing the input sends All Notes Off to prevent
held notes from becoming stuck. THRU is off by default and its setting is also
saved in the page URL.

Firefox uses a stricter permission flow: connect the MIDI device before starting
Firefox, then approve the site-specific MIDI permission add-on when prompted.
Firefox for Android does not support Web MIDI.

## Play with Synthphone E

Click **Play with Synthphone** to open both instruments together, or open the
[paired view](https://nathansbradshaw.github.io/virtual-keyboard-keyboard/?duet=1).

1. **Power on** the phone. The keyboard shows **SYNTHPHONE: ON** once it connects.
2. Play the keyboard's keys or load a MIDI file. Its transport, track mutes,
   channel controls and octave settings now drive the phone.
3. For vocals, enable **MIC** on the phone and lift its receiver. Its screen,
   keypad, dials and vocal effects remain independently playable.

Channels keep the real phone's meanings: **2 = synth**, **1 = vocal control**,
**10 = drums**. The keyboard's live keys default to channel 2. For a MIDI file whose
melody is on channel 1, set that track's **CH** to **2** to hear its synth.

The keyboard's local synth stays silent while Synthphone is selected. Click the
**SYNTHPHONE** button to switch back; playback pauses during the switch.
Hardware MIDI output remains independent. No Web MIDI permission or virtual
MIDI driver is needed for the app-to-app connection. Microphone permission is
requested only by the phone's MIC button. Both apps process audio locally.

The paired view uses the existing virtual phone, including its firmware and
audio limitations. This connection does not establish microphone latency or
physical-device parity. Narrow screens stack the two instruments vertically.

## Key-image annotations

Annotated copies let the renderer extract precise key and label geometry. All
five alpha-row copies now supply text guides and calibrated boundaries; the
fourth row's final divider is temporarily inferred from its photographic valley
until that missing red mark is added. The numpad copy now supplies measured
boundaries and yellow safe-text boxes for all 20 keys. The navigation and arrow
source images remain future annotation work. Keep the originals unchanged and
preserve the exact image dimensions. On each annotated copy:

- Draw every key boundary in solid red, including unambiguous borders where keys
  touch.
- Prefer a yellow text-boundary box on every key. Its center places the label,
  and rendered text is fitted so it never extends outside the box.
- A small purple dot or cross is also supported when only a text center is
  needed.
- Use opaque, non-antialiased marks when possible so the colors are easy to detect
  programmatically.
- Name the copy after the original with an `_annotated` suffix or ` copy`.

The red guides drive sprite extraction and hit geometry; the purple or yellow
guides drive label placement. When **Drum symbols** is enabled, the numpad swaps
to prebuilt blank photographic caps from `numpad-clean.png` before drawing the
symbols; normal mode retains the photographed number legends.

## GitHub Pages

Pushing `main` runs [the Pages workflow](.github/workflows/pages.yml), builds the
LTO-optimized WebAssembly release with the repository subpath as its public URL,
and deploys the `dist` artifact through GitHub Pages. In the repository's GitHub
settings, set **Pages → Source** to **GitHub Actions** once before the first
deployment.

The former [K2 address](https://nathansbradshaw.github.io/k2-midi-viewer/)
redirects to the new site, preserving URL settings and fragments. The small
redirect site lives in the `nathansbradshaw/k2-midi-viewer` repository; its
`index.html` and `404.html` both use [`legacy-site/index.html`](legacy-site/index.html).
The application and its history live in `nathansbradshaw/virtual-keyboard-keyboard`.
