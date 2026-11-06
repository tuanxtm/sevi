// The feedback form: the app opens /feedback?platform=macos&v=1.0.24; Turnstile against spam; POST /api/feedback.
(() => {
  const f = document.getElementById("fb"); if (!f) return;
  const en = document.documentElement.lang === "en";
  const q = new URLSearchParams(location.search);
  const ua = navigator.userAgent;
  const guess = /iPhone|iPad/.test(ua) ? "ios" : /Android/.test(ua) ? "android" : /Windows/.test(ua) ? "windows" : /Mac OS X/.test(ua) ? "macos" : /Linux|X11/.test(ua) ? "linux" : "web";
  const platform = q.get("platform") || guess, version = q.get("v") || "";
  const NAME = { macos: "macOS", windows: "Windows", linux: "Linux", ios: "iOS", android: "Android", web: "Web" };
  f.querySelector(".meta").textContent = (en ? "Sent with: " : "Gửi kèm: ") + "Sevi " + (version ? version + " " : "") + (en ? "for " : "cho ") + (NAME[platform] || platform);
  const err = f.querySelector(".err"), btn = f.querySelector("[type=submit]");
  let token = "", wid;
  window.vkFb = () => { wid = window.turnstile.render(f.querySelector(".ts"), { sitekey: "0x4AAAAAAFqrh-eE-Wh5vzx8", language: en ? "en" : "vi", size: "flexible",
    callback: t => { token = t; }, "expired-callback": () => { token = ""; } }); };
  const s = document.createElement("script"); s.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?onload=vkFb&render=explicit"; s.async = true; document.head.append(s);
  f.onsubmit = async e => {
    e.preventDefault(); err.textContent = "";
    const message = f.message.value.trim(), email = f.email.value.trim();
    if (message.length < 3) { err.textContent = en ? "Please write a few words." : "Bạn viết vài chữ nhé."; f.message.focus(); return; }
    if (email && !/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(email)) { err.textContent = en ? "Please check your email address." : "Email chưa đúng, bạn kiểm tra lại nhé."; f.email.focus(); return; }
    if (!token) { err.textContent = en ? "Please complete the check above." : "Bạn hoàn tất bước xác minh ở trên nhé."; return; }
    btn.disabled = true;
    try {
      const r = await fetch("/api/feedback", { method: "POST", headers: { "content-type": "application/json" },
        body: JSON.stringify({ message, email, kind: f.kind.value, platform, version, lang: en ? "en" : "vi", website: f.website.value, token }) });
      if (!r.ok) throw new Error(r.status);
      f.hidden = true; document.querySelector(".fbdone").hidden = false; window.scrollTo(0, 0);
    } catch { err.textContent = en ? "Something went wrong. Please try again." : "Có lỗi, bạn thử lại nhé."; btn.disabled = false; token = ""; if (wid !== undefined) window.turnstile.reset(wid); }
  };
})();
