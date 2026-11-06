// POST /api/notify {email, platform, lang} -> one row in D1 (sevi-notify.notify), for "tell me when <platform> is out".
const PLATFORMS = ["macos", "windows", "linux", "ios", "android", "play"];   // play = the Google Play listing
const json = (body, status = 200) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

export async function onRequestPost({ request, env }) {
  let d;
  try { d = await request.json(); } catch { return json({ ok: false, error: "bad_request" }, 400); }
  if (d.website) return json({ ok: true });   // honeypot: bots fill every field
  const email = String(d.email || "").trim().toLowerCase().slice(0, 254);
  const platform = String(d.platform || "");
  const lang = d.lang === "en" ? "en" : "vi";
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(email)) return json({ ok: false, error: "email" }, 400);
  if (!PLATFORMS.includes(platform)) return json({ ok: false, error: "platform" }, 400);
  // Cloudflare Turnstile (widget "sevi-notify"); the secret is a Pages secret, TURNSTILE_SECRET
  const v = await fetch("https://challenges.cloudflare.com/turnstile/v0/siteverify", { method: "POST",
    body: new URLSearchParams({ secret: env.TURNSTILE_SECRET, response: String(d.token || ""), remoteip: request.headers.get("CF-Connecting-IP") || "" }) });
  if (!(await v.json()).success) return json({ ok: false, error: "captcha" }, 403);
  await env.DB.prepare("INSERT OR IGNORE INTO notify (email, platform, lang) VALUES (?, ?, ?)").bind(email, platform, lang).run();
  return json({ ok: true });
}
