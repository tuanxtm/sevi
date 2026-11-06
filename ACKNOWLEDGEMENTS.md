# Acknowledgements

Sevi is a Rust port of [Vikey](https://github.com/tonyminibot/Vikey) by
Tony Dinh (Đinh Quang Trung, [@trungdq88](https://github.com/trungdq88)),
which builds on [OpenKey](https://github.com/tuyenvm/OpenKey) by Tuyen Mai.
Sevi is distributed under GPL-3.0-only; see [LICENSE](LICENSE).
Original author credits and the component notices below are retained.

The app contains only word statistics counted from the data below, never the text itself.

## Code

- [Vikey](https://github.com/tonyminibot/Vikey), Tony Dinh (GPL-3.0), the original project ported to Rust as Sevi
- [OpenKey](https://github.com/tuyenvm/OpenKey), Tuyen Mai (GPL-3.0)
- [Sparkle](https://sparkle-project.org), Sparkle Project (MIT)
- [TweetNaCl](https://tweetnacl.cr.yp.to), Daniel J. Bernstein et al. (public domain), Windows update signature check
- [Inno Setup](https://jrsoftware.org/isinfo.php), Jordan Russell, Martijn Laan (Inno Setup License), Windows installer
- [egui / eframe](https://github.com/emilk/egui), egui contributors (MIT OR Apache-2.0), shared desktop UI
- [Inter](https://rsms.me/inter/), Rasmus Andersson (SIL Open Font License 1.1)
- [Geist Mono](https://vercel.com/font), Vercel (SIL Open Font License 1.1)

The shared UI's embedded font licenses are in `crates/sevi-ui/assets/OFL-Inter.txt`
and `crates/sevi-ui/assets/OFL-GeistMono.txt`. The unused Archivo reference asset
retains its license in that folder; it is not embedded in the app.
The shared theme, title-bar glyphs and footer are adapted from
[Craft Launcher](https://github.com/storytold/craft-launcher) (MIT); its license
and the exact source revision are in `crates/sevi-ui/assets/`. Sevi uses upstream
egui/eframe without the launcher's forks or application services.

## Word data (counted into the app)

Only frequency counts and word-pair counts are built in.

- [FrequencyWords (OpenSubtitles 2018)](https://github.com/hermitdave/FrequencyWords), Hermit Dave (CC BY-SA 4.0)
- [FineWeb-2 and FineWeb](https://huggingface.co/datasets/HuggingFaceFW/fineweb-2), Hugging Face (ODC-By 1.0)
- [HPLT v2](https://hplt-project.org), High Performance Language Technologies (CC0)
- [MADLAD-400](https://huggingface.co/datasets/allenai/MADLAD-400), Google (ODC-By 1.0)
- [Reddit comments](https://huggingface.co/datasets/tensorshield/reddit_dataset_157), tensorshield/reddit_dataset_157 (MIT)
- [Vietnamese forums and reader comments](https://vikey.app/acknowledgements), VnExpress, Tuổi Trẻ, Tinhte, Otofun, Dạy Nhau Học (word statistics)
- [Emoji names and keywords (CLDR)](https://cldr.unicode.org), Unicode, Inc. (Unicode License v3)
- [Emojibase](https://emojibase.dev), Miles Johnson (MIT)

## Test data (not in the app)

Used only to measure and tune accuracy.

- [ViLexNorm](https://github.com/ngxtnhi/ViLexNorm), Nguyen et al., EACL 2024 (CC BY-NC-SA 4.0)
- [UIT-ViCTSD](https://huggingface.co/datasets/tarudesu/ViCTSD), UIT NLP Group (research use)
- [VietMix](https://huggingface.co/datasets/razent/vietmix), EACL 2026 (research use)
- [Tap typing with touch sensing images](https://github.com/google-research-datasets/tap-typing-with-touch-sensing-images), Google Research (CC BY 4.0)
- [How We Type: Mobile](https://userinterfaces.aalto.fi/how-we-type-mobile), Aalto University (research use)
- [GitHub Typo Corpus](https://github.com/mhagiwara/github-typo-corpus), Hagiwara & Mita, LREC 2020 (research use)
- [Birkbeck spelling error corpus](https://www.dcs.bbk.ac.uk/~roger/corpora.html), Roger Mitton (free to use)
- [Lists of common misspellings](https://en.wikipedia.org/wiki/Wikipedia:Lists_of_common_misspellings), Wikipedia (CC BY-SA 4.0)

