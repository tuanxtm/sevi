#!/usr/bin/env python3
"""Keeps site/appcast.xml: one feed for the Mac app (Sparkle) and the Windows app (its own updater, same key).
  appcast.py mac <ver> <build> <url> '<sig attrs>' '<notes html>'
  appcast.py windows <ver> <build> <arch> <url> '<sig attrs>' '<notes html>'
Each call replaces the item of that platform/arch, so the feed holds the newest of each."""
import os, re, sys, time
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
PATH = os.path.join(ROOT, "site", "appcast.xml")
HEAD = ('<?xml version="1.0" encoding="utf-8"?>\n<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" '
        'xmlns:sevi="https://vikey.app/xmlns">\n<channel>\n<title>Sevi</title>\n<link>https://vikey.app</link>\n')
TAIL = "</channel>\n</rss>\n"
def item(kind, ver, build, url, sig, notes, arch=None):
    date = time.strftime("%a, %d %b %Y %H:%M:%S +0000", time.gmtime())
    extra = "<sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>\n" if kind == "mac" else ""
    enc = (f'<enclosure url="{url}" type="application/octet-stream" {sig}/>' if kind == "mac"
           else f'<enclosure url="{url}" sparkle:os="windows" sevi:arch="{arch}" type="application/octet-stream" {sig}/>')
    title = f"Sevi {ver}" if kind == "mac" else f"Sevi {ver} for Windows ({arch})"
    return (f"<item>\n<title>{title}</title>\n<pubDate>{date}</pubDate>\n<sparkle:version>{build}</sparkle:version>\n"
            f"<sparkle:shortVersionString>{ver}</sparkle:shortVersionString>\n{extra}<description><![CDATA[{notes}]]></description>\n{enc}\n</item>\n")
def main():
    kind = sys.argv[1]
    old = open(PATH, encoding="utf-8").read() if os.path.exists(PATH) else ""
    items = re.findall(r"<item>.*?</item>\n?", old, flags=re.S)
    if kind == "mac":
        ver, build, url, sig, notes = sys.argv[2:7]; new = item("mac", ver, build, url, sig, notes)
        keep = [i for i in items if 'sparkle:os="windows"' in i]
    else:
        ver, build, arch, url, sig, notes = sys.argv[2:8]; new = item("windows", ver, build, url, sig, notes, arch)
        keep = [i for i in items if not ('sparkle:os="windows"' in i and f'sevi:arch="{arch}"' in i)]
    mac = [i for i in keep if 'sparkle:os="windows"' not in i]; win = [i for i in keep if 'sparkle:os="windows"' in i]
    body = "".join(mac + ([new] if kind == "mac" else []) + win + ([new] if kind != "mac" else []))
    open(PATH, "w", encoding="utf-8").write(HEAD + body + TAIL)
if __name__ == "__main__": main()
