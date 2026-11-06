use crate::key_for;
use sevi_engine::{keys::*, *};
use std::{error::Error, fs, io::Write};
#[derive(Clone, Default)]
pub struct Unit {
    pub keys: String,
    pub vietnamese: bool,
    pub separators: String,
    pub word: bool,
}
pub fn units(source: &str) -> Vec<Unit> {
    let mut out: Vec<Unit> = vec![];
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("W\t") {
            if let Some((keys, lang)) = rest.split_once('\t') {
                out.push(Unit {
                    keys: keys.into(),
                    vietnamese: lang.starts_with('1'),
                    word: true,
                    ..Default::default()
                });
            }
        } else {
            let sep = if line == "N" {
                "\n"
            } else {
                line.strip_prefix("S\t").unwrap_or("")
            };
            if out.is_empty() {
                out.push(Unit::default())
            }
            out.last_mut().unwrap().separators.push_str(sep)
        }
    }
    out
}
#[derive(Clone)]
pub struct Simulator {
    pub driver: Driver,
    pub screen: Vec<u16>,
    nav: Option<Driver>,
    offset: i32,
}
impl Simulator {
    pub fn new(config: EngineConfig) -> Self {
        let mut driver = Driver::new(config);
        let integer = |name: &str, default| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let weight = |name: &str, default| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        driver.simulation.no_lookback = std::env::var_os("SIM_NO_LOOKBACK").is_some();
        driver.simulation.no_restored_previous = std::env::var_os("SIM_NO_RESTORED_PREV").is_some();
        driver.simulation.early_words = integer("SIM_EARLY", 0);
        driver.simulation.early_direction = match integer("SIM_EARLY_DIR", -1) {
            0 => Some(Language::English),
            1 => Some(Language::Vietnamese),
            _ => None,
        };
        if let Some(path) = std::env::var_os("SIM_BIGRAM") {
            driver.engine.load_bigrams(
                Some(std::path::Path::new(&path)),
                weight("SIM_BIGRAM_W", 1.0),
                integer("SIM_BIGRAM_MIN", 3),
            );
        }
        if let Some(path) = std::env::var_os("SIM_TRIGRAM") {
            driver.engine.load_trigrams(
                Some(std::path::Path::new(&path)),
                weight("SIM_TRIGRAM_W", 2.0),
                integer("SIM_TRIGRAM_MIN", 3),
            );
        }
        if let Some(path) = std::env::var_os("SIM_CAP") {
            driver
                .engine
                .load_capitals(Some(std::path::Path::new(&path)), integer("SIM_CAP_MIN", 5));
        }
        Self {
            driver,
            screen: vec![],
            nav: None,
            offset: 0,
        }
    }
    pub fn press(&mut self, key: u32, shift: bool) {
        if self.driver.language() == Language::English
            && (key == KEY_LEFT || key == KEY_RIGHT)
            && !shift
        {
            if key == KEY_LEFT && self.offset == 0 && self.driver.has_english_context() {
                self.nav = Some(self.driver.clone());
                self.offset = -1;
                self.driver.suspend_english_context();
                return;
            }
            if self.offset < 0 {
                if key == KEY_LEFT {
                    self.offset -= 1
                } else {
                    self.offset += 1;
                    if self.offset == 0 {
                        self.driver = self.nav.take().unwrap()
                    }
                }
                return;
            }
        }
        self.offset = 0;
        self.nav = None;
        let edit = self.driver.press_simulated(key, shift);
        self.screen
            .truncate(self.screen.len().saturating_sub(edit.delete_count));
        self.screen.extend(edit.text);
    }
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            if let Some((k, s)) = key_for(c) {
                self.press(k, s)
            }
        }
    }
    pub fn hotkey(&mut self) {
        self.nav = None;
        self.offset = 0;
        self.driver.pause_after_manual = 0.0;
        self.driver.manual_switch(0.0)
    }
    pub fn click(&mut self) {
        self.nav = None;
        self.offset = 0;
        if self.driver.language() == Language::Vietnamese {
            self.driver
                .engine
                .handle_event(Event::Mouse, EventState::MouseDown, 0, 0, false);
        } else {
            self.driver.reset_context()
        }
    }
    pub fn golden(&self, u: &Unit) -> Vec<u16> {
        let mut out = if !u.word {
            vec![]
        } else if !u.vietnamese {
            u.keys.encode_utf16().collect()
        } else {
            let mut s = self.clone();
            s.driver.auto_switch = false;
            s.driver.engine.config.language = Language::Vietnamese;
            s.driver.engine.new_session();
            s.screen.clear();
            s.type_text(&u.keys);
            s.press(KEY_SPACE, false);
            s.screen.pop();
            s.screen
        };
        out.extend(u.separators.encode_utf16());
        out
    }
}
pub fn wrong_words(a: &[u16], b: &[u16]) -> usize {
    let split = |s: &[u16]| {
        s.split(|c| *c == 32 || *c == 10)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_vec())
            .collect::<Vec<_>>()
    };
    let (x, y) = (split(a), split(b));
    let mut previous = vec![0; y.len() + 1];
    for a in x {
        let mut current = vec![0; y.len() + 1];
        for j in 1..=y.len() {
            current[j] = if a == y[j - 1] {
                previous[j - 1] + 1
            } else {
                previous[j].max(current[j - 1])
            }
        }
        previous = current
    }
    y.len() - previous[y.len()]
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let path = args
        .first()
        .ok_or("usage: sim units [--lazy] [--start en|vi] [--off] [--dump file]")?;
    let units = units(&fs::read_to_string(path)?);
    let mut config = crate::config_from_env();
    config.auto_switch = !args.iter().any(|a| a == "--off");
    let value = |name: &str| {
        args.windows(2)
            .find(|a| a[0] == name)
            .map(|a| a[1].as_str())
    };
    if value("--start") == Some("en") {
        config.language = Language::English
    }
    let auto = config.auto_switch;
    let mut s = Simulator::new(config);
    let lazy = args.iter().any(|a| a == "--lazy");
    let trace = args.iter().any(|a| a == "--trace");
    let mut show: usize = value(if lazy { "--show-lazy" } else { "--show" })
        .unwrap_or("0")
        .parse()?;
    let mut dump = value("--dump").map(fs::File::create).transpose()?;
    let (mut words, mut vi, mut en, mut manual, mut errors, mut lines, mut perfect) =
        (0, 0, 0, 0, 0, 0, 0);
    let mut expected: Vec<u16> = vec![];
    for u in &units {
        let mut want = expected.clone();
        want.extend(s.golden(u));
        if lazy {
            s.type_text(&u.keys);
            s.type_text(&u.separators);
            expected = want;
            if u.separators.contains('\n') {
                let wrong = wrong_words(&s.screen, &expected);
                errors += wrong;
                lines += 1;
                if wrong == 0 {
                    perfect += 1
                }
                if let Some(file) = &mut dump {
                    let trim = |text: &[u16]| {
                        String::from_utf16_lossy(if text.last() == Some(&10) {
                            &text[..text.len() - 1]
                        } else {
                            text
                        })
                    };
                    writeln!(file, "{}\t{}", trim(&s.screen), trim(&expected))?;
                }
                if wrong > 0 && show > 0 {
                    show -= 1;
                    eprintln!(
                        "GOT  {}\nWANT {}",
                        String::from_utf16_lossy(&s.screen),
                        String::from_utf16_lossy(&expected)
                    )
                }
                s.screen.clear();
                expected.clear();
            }
        } else {
            let saved = s.clone();
            s.type_text(&u.keys);
            s.type_text(&u.separators);
            if s.screen != want {
                s = saved;
                if std::env::var_os("SIM_MANUAL_LOG").is_some() {
                    eprintln!(
                        "MANUAL\t{}\t{}\t{}",
                        u.keys,
                        u.vietnamese as i32,
                        s.driver.language() as i32
                    )
                }
                s.hotkey();
                manual += 1;
                s.type_text(&u.keys);
                s.type_text(&u.separators);
                if s.screen != want {
                    errors += 1;
                    if show > 0 {
                        show -= 1;
                        eprintln!(
                            "ERROR at '{}': got ...{}| want ...{}|",
                            u.keys,
                            String::from_utf16_lossy(
                                &s.screen[s.screen.len().saturating_sub(40)..]
                            ),
                            String::from_utf16_lossy(&want[want.len().saturating_sub(40)..])
                        )
                    }
                    s.screen = want.clone();
                }
            }
            if trace {
                eprintln!(
                    "unit '{}' lang={} screen='{}' manual={manual}",
                    u.keys,
                    s.driver.language() as i32,
                    String::from_utf16_lossy(&s.screen)
                )
            }
            expected = want;
            if expected.len() > 4000
                && let Some(cut) = expected[..expected.len() - 999]
                    .iter()
                    .rposition(|c| *c == 10)
                && s.screen.get(..cut + 1) == Some(&expected[..cut + 1])
            {
                expected.drain(..cut + 1);
                s.screen.drain(..cut + 1);
            }
        }
        if u.word {
            words += 1;
            if u.vietnamese { vi += 1 } else { en += 1 }
        }
    }
    let state = if auto { "auto ON " } else { "auto OFF" };
    if lazy {
        println!(
            "{state} lazy  words {words}  wrong words {errors} ({:.2}%)  messages {lines}, perfect {perfect} ({:.1}%)",
            100.0 * errors as f64 / words as f64,
            100.0 * perfect as f64 / lines as f64
        )
    } else {
        println!(
            "{state}  words {words} (vi {vi}, en {en})  manual switches {manual} ({:.2} per 100 words)  wrong words {errors} ({:.2}%)  auto {}  undo-by-hotkey {}",
            100.0 * manual as f64 / words as f64,
            100.0 * errors as f64 / words as f64,
            s.driver.auto_switches,
            s.driver.undos
        )
    }
    Ok(())
}
