use crate::{
    key_for,
    prep::decompose,
    rng::Mt19937,
    sim::{Simulator, Unit},
};
use sevi_engine::{keys::*, *};
use std::{error::Error, fs};
const NAMES: [&str; 7] = [
    "typo",
    "back-into-word",
    "cursor-trip",
    "click",
    "edit-elsewhere",
    "switch-trip",
    "stray-punct",
];
#[derive(Clone, Copy, Default)]
struct Stat {
    ops: usize,
    retyped: usize,
}
struct Run {
    s: Simulator,
    tail: Vec<u16>,
    guard: usize,
    acts: String,
    show: i32,
    why: Option<usize>,
    trace: bool,
    stats: [Stat; 7],
    guard_fails: usize,
    elsewhere: usize,
    stuck: usize,
    msg: usize,
}
fn telex_for(c: u16) -> String {
    if c < 128 {
        return char::from_u32(c as u32).unwrap().to_string();
    }
    if c == 0x111 {
        return "dd".into();
    }
    if c == 0x110 {
        return "DD".into();
    }
    let Some(ch) = char::from_u32(c as u32) else {
        return String::new();
    };
    let Some((keys, _, tone)) = decompose(&ch.to_string()) else {
        return String::new();
    };
    let mut out = keys.concat();
    if let Some((t, _)) = tone {
        out.push(t)
    }
    out
}
fn neighbour(c: u8, rng: &mut Mt19937) -> u8 {
    let rows = [b"qwertyuiop".as_slice(), b"asdfghjkl", b"zxcvbnm"];
    for (r, row) in rows.iter().enumerate() {
        if let Some(i) = row.iter().position(|b| *b == c) {
            let mut n = vec![];
            if i > 0 {
                n.push(row[i - 1])
            }
            if i + 1 < row.len() {
                n.push(row[i + 1])
            }
            if r > 0 && i < rows[r - 1].len() {
                n.push(rows[r - 1][i])
            }
            if r < 2 && i < rows[r + 1].len() {
                n.push(rows[r + 1][i])
            }
            return n[rng.next_u32() as usize % n.len()];
        }
    }
    b'x'
}
impl Run {
    fn text(&self) -> Vec<u16> {
        let mut t = self.s.screen.clone();
        t.extend_from_slice(&self.tail);
        t
    }
    fn report(&mut self, what: &str, detail: String) {
        self.show -= 1;
        if self.show >= 0 {
            eprintln!(
                "{what} (message {}): {detail}\n   actions: {}",
                self.msg, self.acts
            )
        }
    }
    fn key(&mut self, key: u32, shift: bool) {
        let old = self.s.screen.clone();
        let before = old.len();
        self.s.press(key, shift);
        let common = old
            .iter()
            .zip(&self.s.screen)
            .take_while(|(a, b)| a == b)
            .count();
        let floor = if key == KEY_DELETE && before > 0 {
            self.guard.min(before - 1)
        } else {
            self.guard
        };
        if common < floor {
            self.guard_fails += 1;
            self.report("GUARD",format!("a key changed text from before the last cursor move/click/switch: '{}' -> '{}'",String::from_utf16_lossy(&old),String::from_utf16_lossy(&self.s.screen)))
        }
        if key == KEY_DELETE && self.s.screen.len() < self.guard {
            self.guard = self.s.screen.len()
        }
        if self.trace {
            eprintln!(
                "  key {key}{} -> '{}|{}'",
                if shift { "^" } else { "" },
                String::from_utf16_lossy(&self.s.screen),
                String::from_utf16_lossy(&self.tail)
            )
        }
    }
    fn type_keys(&mut self, text: &str) {
        for c in text.chars() {
            if let Some((k, s)) = key_for(c) {
                self.key(k, s)
            }
        }
    }
    fn back(&mut self, n: usize) {
        for _ in 0..n {
            self.key(KEY_DELETE, false);
            self.acts.push('⌫')
        }
    }
    fn click(&mut self) {
        self.s.click();
        self.guard = self.s.screen.len()
    }
    fn arrow(&mut self, left: bool) {
        self.s.press(if left { KEY_LEFT } else { KEY_RIGHT }, false);
        if left {
            if let Some(c) = self.s.screen.pop() {
                self.tail.insert(0, c)
            }
        } else if !self.tail.is_empty() {
            self.s.screen.push(self.tail.remove(0))
        }
        self.guard = self.s.screen.len()
    }
    fn switch(&mut self) {
        self.s.hotkey();
        self.guard = self.s.screen.len();
        self.acts.push_str("⌥Z")
    }
    fn reset(&mut self) {
        let config = self.s.driver.engine.config.clone();
        let mut engine = std::mem::replace(&mut self.s.driver.engine, Engine::new(config.clone()));
        engine.new_session();
        engine.forget();
        engine.config.language = Language::Vietnamese;
        self.s = Simulator::new(config);
        self.s.driver.engine = engine;
        self.tail.clear();
        self.guard = 0;
    }
    fn restore(&mut self, want: &[u16]) {
        while !self.tail.is_empty() {
            self.arrow(false)
        }
        let keep = self
            .s
            .screen
            .iter()
            .zip(want)
            .take_while(|(a, b)| a == b)
            .count();
        self.back(self.s.screen.len() - keep);
        self.s.screen = want.to_vec();
        self.click()
    }
    fn neutral(&mut self, kind: usize, before: &[u16]) {
        self.stats[kind].ops += 1;
        if self.tail.is_empty() && self.text() == before {
            return;
        }
        self.stats[kind].retyped += 1;
        if self.why == Some(kind) {
            self.report(
                "RETYPE",
                format!(
                    "{}: got '{}' want '{}'",
                    NAMES[kind],
                    String::from_utf16_lossy(&self.text()),
                    String::from_utf16_lossy(before)
                ),
            )
        }
        self.restore(before)
    }
    fn correct(&mut self, u: &Unit, base: &[u16], meant: &[u16], keys: &str, slip: usize) {
        let ok = |s: &Self| {
            s.tail.is_empty()
                && s.s.screen.len() == base.len() + meant.len()
                && s.s.screen.ends_with(meant)
        };
        if ok(self) {
            return;
        }
        self.stats[slip].retyped += 1;
        if self.why == Some(slip) {
            self.report(
                "RETYPE",
                format!(
                    "{}: got '{}' want '...{}'",
                    NAMES[slip],
                    String::from_utf16_lossy(&self.text()),
                    String::from_utf16_lossy(meant)
                ),
            )
        }
        for attempt in 0..3 {
            self.restore(base);
            self.acts.push_str(match attempt {
                0 => "[retype]",
                1 => "[⌥Z retype]",
                _ => "[retype, ⌫ undo]",
            });
            let need = if u.vietnamese {
                Language::Vietnamese
            } else {
                Language::English
            };
            if attempt >= 1 && self.s.driver.language() != need {
                self.switch()
            }
            self.type_keys(keys);
            if ok(self) {
                return;
            }
            if attempt >= 2 {
                self.back(1);
                self.type_keys(&u.separators);
                if ok(self) {
                    return;
                }
            }
        }
        self.stuck += 1;
        self.report(
            "STUCK",
            format!(
                "{}: got '{}' want '...{}'",
                NAMES[slip],
                String::from_utf16_lossy(&self.text()),
                String::from_utf16_lossy(meant)
            ),
        );
        let mut want = base.to_vec();
        want.extend_from_slice(meant);
        self.restore(&want)
    }
}
pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(
        args.first()
            .ok_or("usage: rtsim units [--seed N] [--show N] [--only N] [--trace] [--why kind]")?,
    )?;
    let val = |name: &str| {
        args.windows(2)
            .find(|p| p[0] == name)
            .map(|p| p[1].as_str())
    };
    let seed: u32 = val("--seed").unwrap_or("1").parse()?;
    let only: Option<usize> = val("--only").map(str::parse).transpose()?;
    let mut r = Run {
        s: Simulator::new(EngineConfig::default()),
        tail: vec![],
        guard: 0,
        acts: String::new(),
        show: val("--show").unwrap_or("5").parse()?,
        why: val("--why").and_then(|n| NAMES.iter().position(|v| *v == n)),
        trace: args.iter().any(|a| a == "--trace"),
        stats: [Stat::default(); 7],
        guard_fails: 0,
        elsewhere: 0,
        stuck: 0,
        msg: 0,
    };
    let mut messages: Vec<Vec<Unit>> = vec![vec![]];
    for line in source.lines() {
        if line == "N" {
            messages.push(vec![])
        } else if let Some(rest) = line.strip_prefix("W\t") {
            if let Some((k, v)) = rest.split_once('\t') {
                messages.last_mut().unwrap().push(Unit {
                    keys: k.into(),
                    vietnamese: v.starts_with('1'),
                    word: true,
                    ..Default::default()
                })
            }
        } else {
            let msg = messages.last_mut().unwrap();
            if msg.is_empty() {
                msg.push(Unit::default())
            }
            msg.last_mut()
                .unwrap()
                .separators
                .push_str(line.strip_prefix("S\t").unwrap_or(""))
        }
    }
    let mut words = 0;
    for msg in messages {
        r.msg += 1;
        if msg.is_empty() || only.is_some_and(|n| n != r.msg) {
            continue;
        }
        let mut rng = Mt19937::new(seed.wrapping_mul(7919).wrapping_add(r.msg as u32));
        r.reset();
        let meant: Vec<Vec<u16>> = msg.iter().map(|u| r.s.golden(u)).collect();
        r.reset();
        r.acts.clear();
        for (ui, u) in msg.iter().enumerate() {
            let dice = rng.next_u32() % 1000;
            let after = ui > 0
                && msg[ui - 1].separators.ends_with(' ')
                && msg[ui - 1].word
                && !r.s.screen.is_empty();
            if after
                && u.word
                && dice < 50
                && r.s.screen.len() >= 2
                && !telex_for(r.s.screen[r.s.screen.len() - 2]).is_empty()
            {
                let before = r.text();
                let again = telex_for(r.s.screen[r.s.screen.len() - 2]);
                r.acts.push_str("{back:");
                r.back(2);
                r.acts.push_str(&again);
                r.type_keys(&again);
                r.key(KEY_SPACE, false);
                r.acts.push_str("␣}");
                r.neutral(1, &before)
            } else if after && u.word && dice < 60 && r.s.screen.len() > 6 {
                if let Some(sp) = r.s.screen[..r.s.screen.len() - 1]
                    .iter()
                    .rposition(|c| *c == 32)
                    && sp > 0
                {
                    let before = r.text();
                    let n = r.s.screen.len() - sp - 1;
                    r.acts.push_str(&format!("{{elsewhere←{n} ok␣⌫⌫⌫→}}"));
                    for _ in 0..n {
                        r.arrow(true)
                    }
                    r.type_keys("ok ");
                    let mut want = before[..sp + 1].to_vec();
                    want.extend("ok ".encode_utf16());
                    want.extend_from_slice(&before[sp + 1..]);
                    if r.text() != want {
                        r.elsewhere += 1;
                        r.report(
                            "ELSEWHERE",
                            format!(
                                "got '{}' want '{}'",
                                String::from_utf16_lossy(&r.text()),
                                String::from_utf16_lossy(&want)
                            ),
                        )
                    }
                    r.back(3);
                    for _ in 0..n {
                        r.arrow(false)
                    }
                    r.neutral(4, &before)
                }
            } else if after && u.word && (80..100).contains(&dice) {
                let before = r.text();
                let m = b"-.,/;"[rng.next_u32() as usize % 5] as char;
                r.acts.push_str(&format!("{{stray:{m}⌫}}"));
                r.type_keys(&m.to_string());
                r.back(if r.tail.is_empty() && r.text().len() > before.len() {
                    r.text().len() - before.len()
                } else {
                    1
                });
                r.neutral(6, &before)
            } else if after && u.word && dice < 80 {
                let before = r.text();
                let w = [
                    "hello", "ok", "meeting", "deadline", "test", "sorry", "file", "update",
                ][rng.next_u32() as usize % 8];
                r.switch();
                r.acts.push_str(w);
                r.type_keys(w);
                r.switch();
                r.back(if r.tail.is_empty() && r.text().len() > before.len() {
                    r.text().len() - before.len()
                } else {
                    w.len()
                });
                r.neutral(5, &before)
            }
            let base = r.text();
            let mut slip = None;
            for c in u.keys.chars() {
                let Some((key, shift)) = key_for(c) else {
                    continue;
                };
                let d = rng.next_u32() % 1000;
                if c.is_ascii_lowercase() && d < 40 {
                    let was = r.text();
                    let n = 1 + usize::from(rng.next_u32().is_multiple_of(4));
                    r.acts.push_str("{typo:");
                    for _ in 0..n {
                        let t = neighbour(c as u8, &mut rng) as char;
                        r.acts.push(t);
                        let (k, s) = key_for(t).unwrap();
                        r.key(k, s)
                    }
                    r.acts.push('}');
                    r.back(n);
                    slip = Some(0);
                    r.stats[0].ops += 1;
                    if r.text() != was && r.tail.is_empty() {
                        let t = r.text();
                        let same = t.iter().zip(&was).take_while(|(a, b)| a == b).count();
                        if same >= base.len() {
                            r.back(t.len() - same);
                            let again: String = was[same..].iter().map(|c| telex_for(*c)).collect();
                            r.acts.push_str(&format!("[{again}]"));
                            r.type_keys(&again)
                        }
                    }
                } else if d >= 990 {
                    let n = 1 + rng.next_u32() % 4;
                    r.acts.push_str(&format!("{{←→{n}}}"));
                    let guard = r.guard;
                    for _ in 0..n {
                        r.arrow(true)
                    }
                    for _ in 0..n {
                        r.arrow(false)
                    }
                    r.guard = guard.min(
                        r.s.screen
                            .iter()
                            .rposition(|c| *c == 32 || *c == 10)
                            .map(|i| i + 1)
                            .unwrap_or(0),
                    );
                    slip = Some(2);
                    r.stats[2].ops += 1
                } else if d >= 985 {
                    r.acts.push_str("{click}");
                    r.click();
                    slip = Some(3);
                    r.stats[3].ops += 1
                }
                r.key(key, shift);
                r.acts.push(c)
            }
            r.type_keys(&u.separators);
            r.acts.extend(
                u.separators
                    .chars()
                    .map(|c| if c == '\n' { '|' } else { c }),
            );
            if u.word {
                words += 1;
                if let Some(slip) = slip {
                    r.correct(
                        u,
                        &base,
                        &meant[ui],
                        &format!("{}{}", u.keys, u.separators),
                        slip,
                    )
                }
            }
        }
    }
    println!(
        "realistic typing: {} messages, {words} words | guard {}  elsewhere {}  stuck {}",
        r.msg, r.guard_fails, r.elsewhere, r.stuck
    );
    print!("  slips that needed fixing by hand:");
    for (i, s) in r.stats.iter().enumerate() {
        print!("  {} {}/{}", NAMES[i], s.retyped, s.ops)
    }
    println!();
    if r.guard_fails > 0 || r.elsewhere > 0 || r.stuck > 0 {
        return Err("realistic typing violations".into());
    }
    Ok(())
}
