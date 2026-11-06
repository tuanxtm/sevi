// Download buttons that follow the visitor's OS, and the "notify me" form for platforms without a build yet.
// window.VK_DL (written by build.py from release.json): {macos:{url}, windows:{url, arm}, linux, ios, android}; null = pending.
(() => {
  const DL = window.VK_DL || {}, en = document.documentElement.lang === "en", pre = en ? "/en" : "";
  const NAME = { macos: "macOS", windows: "Windows", linux: "Linux", ios: "iOS", android: "Android", play: "Google Play" };
  const S = en
    ? { dl: os => `Download for ${os}`, store: "Download on the App Store", short: "Download", soon: "Soon",
        h: os => `Sevi for ${os} is coming`, p: os => `Leave your email and we'll let you know when Sevi for ${os} is ready.`,
        ph: "you@email.com", go: "Notify me", bad: "Please check your email address.", fail: "Something went wrong. Please try again.",
        captcha: "Please complete the check above.", ok: "You're on the list", okp: os => `We'll email you once, when Sevi for ${os} is out.`, done: "Done", close: "Close" }
    : { dl: os => `Tải cho ${os}`, store: "Tải trên App Store", short: "Tải về", soon: "Sắp có",
        h: os => `Sevi cho ${os} sắp có`, p: os => `Để lại email, Sevi sẽ báo bạn khi có bản cho ${os}.`,
        ph: "ban@email.com", go: "Báo cho tôi", bad: "Email chưa đúng, bạn kiểm tra lại nhé.", fail: "Có lỗi, bạn thử lại nhé.",
        captcha: "Bạn hoàn tất bước xác minh ở trên nhé.", ok: "Đã ghi nhận", okp: os => `Sevi sẽ email bạn một lần, khi bản cho ${os} ra mắt.`, done: "Xong", close: "Đóng" };

  const ua = navigator.userAgent;
  const os = /iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1) ? "ios"
    : /Android/.test(ua) ? "android" : /Windows/.test(ua) ? "windows" : /Mac OS X|Macintosh/.test(ua) ? "macos"
    : /Linux|X11|CrOS/.test(ua) ? "linux" : null;
  const icon = p => `<svg class="ic" aria-hidden="true"><use href="/assets/os.svg#${p}"/></svg>`;

  // Buttons marked data-cta become "Download for <your OS>" (or "notify me" when that OS has no build yet).
  if (os) document.querySelectorAll("[data-cta]").forEach(a => {
    const d = DL[os], long = a.dataset.cta === "long";
    a.innerHTML = icon(os) + (long ? (os === "ios" && d ? S.store : S.dl(NAME[os])) : S.short) + (d ? "" : `<span class="soon">${S.soon}</span>`);
    if (!long) { a.href = `${pre}/download#${os}`; if (!d) a.dataset.notify = os; return; }   // the header button opens the download page (Tony 2026-10-29)
    if (d && os === "linux") a.href = `${pre}/download#linux`;   // .deb or .rpm: let them pick on the card
    else if (d) { a.href = d.url; if (d.url.startsWith("/downloads/")) a.setAttribute("download", ""); }
    else { a.href = `${pre}/download#${os}`; a.dataset.notify = os; }
  });
  // Windows on ARM gets the ARM64 installer
  if (os === "windows" && DL.windows && DL.windows.arm && navigator.userAgentData && navigator.userAgentData.getHighEntropyValues)
    navigator.userAgentData.getHighEntropyValues(["architecture"]).then(v => {
      if (v.architecture === "arm") document.querySelectorAll('[data-cta],[data-os="windows"] .btn.dl').forEach(a => { if (a.getAttribute("href") === DL.windows.url) a.href = DL.windows.arm; });
    }).catch(() => {});
  if (os) document.querySelectorAll(`.pcard[data-os="${os}"]`).forEach(c => c.classList.add("mine"));

  // Cloudflare Turnstile against spam, loaded only when the form opens
  const SITEKEY = "0x4AAAAAAFqrh-eE-Wh5vzx8";
  let tsLoad;
  const turnstile = () => tsLoad || (tsLoad = new Promise((ok, no) => {
    window.vkTs = () => ok(window.turnstile);
    const s = document.createElement("script"); s.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?onload=vkTs&render=explicit";
    s.async = true; s.onerror = no; document.head.append(s);
  }));

  // The notify form, one <dialog> per page
  let dlg;
  function open(p) {
    if (!dlg) {
      dlg = document.createElement("dialog"); dlg.className = "sendbox"; document.body.append(dlg);
      dlg.addEventListener("click", e => { if (e.target === dlg) dlg.close(); });
    }
    dlg.innerHTML = `<form class="sendbox-in" method="dialog" novalidate>
<button class="x" type="button" aria-label="${S.close}">×</button>
<div class="nicon">${icon(p === "play" ? "android" : p)}</div><h3>${S.h(NAME[p])}</h3><p class="muted">${S.p(NAME[p])}</p>
<input type="email" name="email" required autocomplete="email" placeholder="${S.ph}" aria-label="Email">
<input class="hp" name="website" tabindex="-1" autocomplete="off" aria-hidden="true">
<div class="ts"></div><p class="err" role="alert"></p><button class="btn big" type="submit">${S.go}</button></form>`;
    const f = dlg.querySelector("form"), err = f.querySelector(".err"), btn = f.querySelector("[type=submit]");
    let token = "", wid;
    turnstile().then(ts => { wid = ts.render(f.querySelector(".ts"), { sitekey: SITEKEY, language: en ? "en" : "vi", size: "flexible",
      callback: t => { token = t; }, "expired-callback": () => { token = ""; } }); }).catch(() => {});
    f.querySelector(".x").onclick = () => dlg.close();
    f.onsubmit = async e => {
      e.preventDefault(); err.textContent = "";
      const email = f.email.value.trim();
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(email)) { err.textContent = S.bad; f.email.focus(); return; }
      if (!token) { err.textContent = S.captcha; return; }
      btn.disabled = true;
      try {
        const r = await fetch("/api/notify", { method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ email, platform: p, lang: en ? "en" : "vi", website: f.website.value, token }) });
        if (!r.ok) throw new Error(r.status);
        f.innerHTML = `<div class="sent"><div class="tick">✓</div><h3>${S.ok}</h3><p class="muted">${S.okp(NAME[p])}</p><button class="btn big" type="button">${S.done}</button></div>`;
        f.querySelector("button").onclick = () => dlg.close();
      } catch { err.textContent = S.fail; btn.disabled = false; token = ""; if (wid !== undefined) window.turnstile.reset(wid); }
    };
    dlg.showModal(); f.email.focus();
  }
  document.addEventListener("click", e => {
    const a = e.target.closest("[data-notify]");
    if (a) { e.preventDefault(); open(a.dataset.notify); }
  });
  const h = location.hash.slice(1);   // /download#ios from a pending CTA opens the form directly
  if (NAME[h] && !DL[h] && document.querySelector(".pgrid")) open(h);
})();
