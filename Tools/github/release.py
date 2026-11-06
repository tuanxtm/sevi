"""Sevi's Windows x64 release lane. Run from the standalone repository root."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
import xml.etree.ElementTree as ET
import zipfile

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "tuanxtm/sevi"
SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"
SEVI = "https://github.com/tuanxtm/sevi/xmlns"
PUBLIC = ROOT.joinpath("updates/public-key.txt")
PRIVATE = ROOT.joinpath("target/release-secrets/update-private-key.txt")


def version():
    return tomllib.loads(ROOT.joinpath("Cargo.toml").read_text())["workspace"]["package"]["version"]


def check_tag(tag):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", tag) or tag != "v" + version():
        raise ValueError("Release tag must equal v plus the Cargo.toml version")
    if len(base64.b64decode(PUBLIC.read_text().strip(), validate=True)) != 32:
        raise ValueError("Invalid update public key")


def private_key(encoded):
    key = Ed25519PrivateKey.from_private_bytes(base64.b64decode(encoded.strip(), validate=True))
    expected = base64.b64decode(PUBLIC.read_text().strip(), validate=True)
    if key.public_key().public_bytes_raw() != expected:
        raise ValueError("Signing secret does not match the committed update public key")
    return key


def keygen():
    PRIVATE.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if PRIVATE.exists():
        key = Ed25519PrivateKey.from_private_bytes(base64.b64decode(PRIVATE.read_text().strip(), validate=True))
    else:
        if PUBLIC.exists():
            raise ValueError("Public key already exists. Restore its private backup; do not rotate it accidentally")
        key = Ed25519PrivateKey.generate()
        with os.fdopen(os.open(PRIVATE, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
            stream.write(base64.b64encode(key.private_bytes_raw()).decode() + "\n")
    public = base64.b64encode(key.public_key().public_bytes_raw()).decode()
    if PUBLIC.exists() and PUBLIC.read_text().strip() != public:
        raise ValueError("Existing keys do not match")
    PUBLIC.parent.mkdir(parents=True, exist_ok=True)
    PUBLIC.write_text(public + "\n")
    print("Public update key:", public)
    print("Private backup (never commit):", PRIVATE)


def notes():
    parts = [ROOT.joinpath("ACKNOWLEDGEMENTS.md").read_text()]
    for name in ["LICENSE-Craft-Launcher-MIT.txt", "OFL-Inter.txt", "OFL-GeistMono.txt"]:
        parts.append(name + "\n\n" + ROOT.joinpath("crates/sevi-ui/assets", name).read_text())
    return "\n\n".join(parts)


def package(tag):
    check_tag(tag)
    output = ROOT.joinpath("dist")
    staging = ROOT.joinpath("target/release-package")
    output.mkdir(exist_ok=True)
    staging.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT.joinpath("target/x86_64-pc-windows-msvc/release/sevi.exe"), staging.joinpath("Sevi.exe"))
    shutil.copy2(ROOT.joinpath("LICENSE"), staging.joinpath("LICENSE"))
    staging.joinpath("THIRD_PARTY_NOTICES.txt").write_text(notes(), encoding="utf-8")
    compilers = [shutil.which("ISCC.exe")]
    for directory in [os.environ.get("ProgramFiles(x86)"), os.environ.get("ProgramFiles")]:
        if directory:
            compilers.extend(str(Path(directory).joinpath("Inno Setup " + v, "ISCC.exe")) for v in ["7", "6"])
    compiler = next((p for p in compilers if p and Path(p).is_file()), None)
    if not compiler:
        raise ValueError("Install Inno Setup 6 or newer to build the update installer")
    subprocess.run([compiler, "/DVersion=" + version(), "/DArch=x64", "/DSrc=" + str(staging),
                    "/O" + str(output), str(ROOT.joinpath("Windows/installer/Sevi.iss"))], check=True, cwd=ROOT)
    shutil.copy2(staging.joinpath("Sevi.exe"), output.joinpath("Sevi-" + version() + "-x64.exe"))
    with zipfile.ZipFile(output.joinpath("Sevi-" + version() + "-x64-portable.zip"), "w", zipfile.ZIP_DEFLATED) as archive:
        for name in ["Sevi.exe", "LICENSE", "THIRD_PARTY_NOTICES.txt"]:
            archive.write(staging.joinpath(name), name)


def appcast(tag, installer, encoded):
    check_tag(tag)
    key = private_key(encoded)
    data = installer.read_bytes()
    signature = key.sign(data)
    key.public_key().verify(signature, data)
    ET.register_namespace("sparkle", SPARKLE)
    ET.register_namespace("sevi", SEVI)
    root = ET.Element("rss", {"version": "2.0"})
    channel = ET.SubElement(root, "channel")
    ET.SubElement(channel, "title").text = "Sevi for Windows"
    ET.SubElement(channel, "link").text = "https://github.com/" + REPOSITORY
    item = ET.SubElement(channel, "item")
    ET.SubElement(item, "title").text = "Sevi " + version()
    ET.SubElement(item, "{" + SPARKLE + "}shortVersionString").text = version()
    ET.SubElement(item, "description").text = "Sevi " + version() + ". See the GitHub release for changes."
    ET.SubElement(item, "enclosure", {
        "url": f"https://github.com/{REPOSITORY}/releases/download/{tag}/{installer.name}",
        "type": "application/octet-stream", "length": str(len(data)),
        "{" + SPARKLE + "}os": "windows", "{" + SEVI + "}arch": "x64",
        "{" + SPARKLE + "}edSignature": base64.b64encode(signature).decode(),
    })
    ET.indent(root)
    # The native lightweight parser consumes <item> blocks and compact enclosure attributes.
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def sign(tag):
    output = ROOT.joinpath("dist")
    installer = output.joinpath("Sevi-" + version() + "-x64-Setup.exe")
    encoded = os.environ.get("SEVI_UPDATE_PRIVATE_KEY", "")
    if not encoded:
        raise ValueError("SEVI_UPDATE_PRIVATE_KEY is required; never publish unsigned update feeds")
    output.joinpath("appcast.xml").write_bytes(appcast(tag, installer, encoded))
    names = [installer.name, "Sevi-" + version() + "-x64.exe", "Sevi-" + version() + "-x64-portable.zip", "appcast.xml"]
    output.joinpath("SHA256SUMS").write_text("".join(
        hashlib.sha256(output.joinpath(name).read_bytes()).hexdigest() + "  " + name + "\n" for name in names))


def publish(tag):
    check_tag(tag)
    args = ["gh", "release"]
    view = subprocess.run(args + ["view", tag, "--repo", REPOSITORY, "--json", "isDraft"],
                          cwd=ROOT, text=True, capture_output=True)
    if view.returncode == 0:
        if not json.loads(view.stdout)["isDraft"]:
            raise ValueError("This release is already published; create a new version instead of replacing signed assets")
    else:
        subprocess.run(args + ["create", tag, "--repo", REPOSITORY, "--draft", "--verify-tag",
                              "--title", "Sevi " + version(), "--generate-notes"], check=True, cwd=ROOT)
    names = ["Sevi-" + version() + "-x64-Setup.exe", "Sevi-" + version() + "-x64.exe",
             "Sevi-" + version() + "-x64-portable.zip", "appcast.xml", "SHA256SUMS"]
    subprocess.run(args + ["upload", tag, "--repo", REPOSITORY, "--clobber"] +
                   [str(ROOT.joinpath("dist", name)) for name in names], check=True, cwd=ROOT)
    subprocess.run(args + ["edit", tag, "--repo", REPOSITORY, "--draft=false", "--latest"], check=True, cwd=ROOT)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["keygen", "check", "package", "sign", "publish"])
    parser.add_argument("--tag")
    args = parser.parse_args()
    if args.command == "keygen":
        keygen()
    else:
        check_tag(args.tag or "")
        if args.command == "package": package(args.tag)
        elif args.command == "sign": sign(args.tag)
        elif args.command == "publish": publish(args.tag)


if __name__ == "__main__": main()
