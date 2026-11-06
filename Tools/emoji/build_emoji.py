#!/usr/bin/env python3
"""Builds App/Resources/Emoji.json for the emoji picker (Mac + iOS) from:
  emojibase-data (MIT): the emoji list, order, groups, skin tones, Unicode version, English tags + GitHub shortcodes
  Unicode CLDR annotations (Unicode License): English and Vietnamese names + keywords
Each entry: [emoji, group, version, en name, vi name, en keywords, vi keywords, [5 skin-tone variants] or 0, popularity,
emoticons]. Emoticons: emojibase's (:D, <3) plus Vietnamese and Japanese styles in EMOTICONS (Tony: =)), :v, ^_^).
Popularity (0-10) breaks ties in search: POPULAR is a hand-picked list of the most used emoji, most used first.
Run: python3 Tools/emoji/build_emoji.py   (downloads the npm packages into a temp folder)"""
import json, os, subprocess, tarfile, tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
OUT = os.path.join(ROOT, "App/Resources/Emoji.json")
PKGS = {"emojibase-data": "18.0.0", "cldr-annotations-full": "49.0.0", "cldr-annotations-derived-full": "49.0.0"}

def fetch(tmp):
    d = {}
    for name, ver in PKGS.items():
        tgz = subprocess.check_output(["npm", "pack", f"{name}@{ver}", "--silent"], cwd=tmp, text=True).strip().split("\n")[-1]
        dest = os.path.join(tmp, name); tarfile.open(os.path.join(tmp, tgz)).extractall(dest)
        d[name] = os.path.join(dest, "package")
    return d

def cldr(p, lang):
    a = json.load(open(f"{p['cldr-annotations-full']}/annotations/{lang}/annotations.json"))["annotations"]["annotations"]
    b = json.load(open(f"{p['cldr-annotations-derived-full']}/annotationsDerived/{lang}/annotations.json"))["annotationsDerived"]["annotations"]
    return {**b, **a}

POPULAR = ("😂❤️🤣👍😭🙏😘🥰😍😊🎉😁💕🥺😅🔥☺️🤦♥️🤷🙄😆🤗😉🎂🤔👏🙂😳🥳😎👌💜😔💪✨💖👀😋😏😢👉💗😩💯🌹💞🎈💙😃😡💐😜"
           "🙈🤞😄🤤🙌🤪❣️😀💋💀👇💔😌💓🤩🙃😬😱😴🤭😐🌞😒😇🌸😈🎶✌️🎊🥵😞💚☀️🖤💰😚👑🎁💥🙋☹️😑🥴👈💩✅👋🤮😤🤢🌟❗😥🌈"
           "💛😝😫😲‼️🔴🌻🤯💃👊🤬🏃😕⚡☕🍀💦⭐🦋🤨🌺😹🤘🌷💝💤🤝🐰😓💘🍻😟😣🧐😠🤠😻🌙😛🤙🙊🇻🇳🐶🐱🍺🍕🎵📷🍜🧧🌏🐴🐎")

# emoticon -> emoji it means, best first. Vietnamese chat styles (=)), :v, hihi) and Japanese kaomoji (^_^, T_T)
EMOTICONS = {
    ":))": "😂🤣😆", ":)))": "😂🤣", "=))": "😂🤣😆", "=)))": "😂🤣", ":v": "😂😜😆", ":3": "😽🥰😊",
    "^_^": "😊😄☺️", "^^": "😊😄☺️", "^.^": "😊😄", "^-^": "😊😄", "-_-": "😑😒", "-.-": "😑😒",
    "t_t": "😭😢", "t.t": "😭😢", ";_;": "😭😢", "o_o": "😳🤨", "o.o": "😳🤨", ">.<": "😣😆", ">_<": "😣😆",
    "*_*": "😍🤩", "u_u": "😔", ":((": "😢😭", ":(((": "😭", "=)": "🙂", "(:": "🙂", "=d": "😄", ":-)": "🙂",
    ":-(": "☹️", ":-d": "😄", ";-)": "😉", ":-p": "😛", "<33": "❤️", "@@": "😵‍💫😵", "zzz": "😴💤",
    ":d": "😄😁😃😀", ":(": "☹️🙁😞", ":-o": "😮😲", "haha": "😂😆😄", "hihi": "😊😄", "hehe": "😆😄", "huhu": "😭😢", "hichic": "😢😭", "kkk": "😂😆", "keke": "😆😂",
}

def norm(e): return e.replace("️", "")

def main():
    with tempfile.TemporaryDirectory() as tmp:
        p = fetch(tmp)
        data = json.load(open(f"{p['emojibase-data']}/en/data.json"))
        github = json.load(open(f"{p['emojibase-data']}/en/shortcodes/github.json"))
        en, vi = cldr(p, "en"), cldr(p, "vi")
    en = {norm(k): v for k, v in en.items()}; vi = {norm(k): v for k, v in vi.items()}
    import regex   # grapheme clusters, to split POPULAR into emoji
    pop = {norm(g): i for i, g in enumerate(regex.findall(r"\X", POPULAR))}
    emo = {}
    for x in data:
        e = x.get("emoticon")
        for t in ([e] if isinstance(e, str) else (e or [])): emo.setdefault(norm(x["emoji"]), []).append(t.lower())
    for t, chars in EMOTICONS.items():
        for g in regex.findall(r"\X", chars): emo.setdefault(norm(g), []).append(t)
    out = []
    for x in sorted((x for x in data if "group" in x and x["group"] != 2), key=lambda x: x["order"]):   # 2 = components
        e = x["emoji"]; k = norm(e)
        ea, va = en.get(k, {}), vi.get(k, {})
        codes = github.get(x["hexcode"], [])
        codes = [codes] if isinstance(codes, str) else codes
        enk = sorted(set(ea.get("default", []) + x.get("tags", []) + [c.replace("_", " ") for c in codes]))
        enname = (ea.get("tts") or [x["label"]])[0]
        viname = (va.get("tts") or [""])[0]
        vik = sorted(set(va.get("default", [])))
        skins = [s["emoji"] for s in x.get("skins", []) if isinstance(s.get("tone"), int)]
        out.append([e, x["group"], x["version"], enname, viname, " | ".join(enk), " | ".join(vik), skins if len(skins) == 5 else 0,
                    max(0, 10 - pop[k] // 16) if k in pop else 0, " ".join(sorted(set(emo.get(k, []))))])
    json.dump(out, open(OUT, "w"), ensure_ascii=False, separators=(",", ":"))
    novi = sum(1 for o in out if not o[4])
    print(f"{len(out)} emoji -> {os.path.relpath(OUT, ROOT)} ({os.path.getsize(OUT)//1024} KB), {novi} without a Vietnamese name")

if __name__ == "__main__":
    main()
