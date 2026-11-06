# sevi

**Smart English/Vietnamese input.** Written in Rust, with a small desktop UI and a soft spot for dấu.

Type in English and Vietnamese; Sevi uses the word and its context to help switch
languages for you. A little side project for a keyboard with two languages on its mind.

[Download](https://github.com/tuanxtm/sevi/releases/latest) · [Build](BUILDING.md) · [Report a bug](https://github.com/tuanxtm/sevi/issues) · [License & credits](#license-and-credits)

## Less switching, more typing

- **Telex, as usual.** Tone placement, spell checking and restoration of the original keys when a word isn't Vietnamese.
- **English fits in.** Automatic language detection, with context for words that could belong to either language.
- **Your preferences stick.** Remember the language per app and supported website, or switch manually with a configurable shortcut.
- **One small settings window.** English labels, simple checkboxes, light and dark themes, and proper display scaling.

A few examples:

```text
treen       → trên
truowfng    → trường
more        → more
git push    → git push
```

## Get started

Grab the **Windows x64 installer** or **portable ZIP** from the
[latest release](https://github.com/tuanxtm/sevi/releases/latest).
Quit any other Vietnamese input method before starting Sevi, then choose Telex
and your preferred switching options in settings.

With **Automatic updates** enabled, Windows downloads signed updates in the background.
Upgrading from an older test build? Install the GitHub version manually once;
the [update guide](RELEASING.md#what-the-installed-app-does) covers the details.

The source and shared UI cover Windows, Linux and macOS. Packaged releases and
automatic installation currently target **Windows x64**; see the
[build guide](BUILDING.md) for the other platforms.

## Under the hood

Rust handles the typing engine and native desktop integration. [egui](https://github.com/emilk/egui)
powers the shared UI, which runs on demand while the input method stays in the tray.
Typing rules are checked against regression cases and Vietnamese/English text corpora.

Want to poke around? With the pinned Rust toolchain and your platform's
[build dependencies](BUILDING.md) installed:

```sh
git clone https://github.com/tuanxtm/sevi.git
cd sevi
cargo run --locked --release -p sevi-ui --bin sevi-ui-preview
cargo xtask check
```

Found a word that trips it up? [Open an issue](https://github.com/tuanxtm/sevi/issues)
with what you typed, what appeared, and which app you were using.
Small fixes are welcome too: [contributing](CONTRIBUTING.md).

## License and credits

Sevi is a Rust port of **[Vikey](https://github.com/tonyminibot/Vikey)** by
**Tony Dinh**, built on **[OpenKey](https://github.com/tuyenvm/OpenKey)** by **Tuyen Mai**.
Their work made this little project possible. Thank you. ❤️

Licensed under **GPL-3.0-only**; see [LICENSE](LICENSE). Original author credits
are retained. UI, fonts, libraries and word-data sources are listed in
[Acknowledgements](ACKNOWLEDGEMENTS.md).
