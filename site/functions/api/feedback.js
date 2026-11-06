// POST /api/feedback {message, email?, kind, platform, version, lang, token} -> one row in D1 (sevi-notify.feedback).
const KINDS = ["bug", "idea", "other"];
const PLATFORMS = ["macos", "windows", "linux", "ios", "android", "web"];
const json = (body, status = 200) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
const clip = (v, n) => String(v || "").trim().slice(0, n);

export async function onRequestPost({ request, env }) {
  let d;
  try { d = await request.json(); } catch { return json({ ok: false, error: "bad_request" }, 400); }
  if (d.website) return json({ ok: true });   // honeypot
  const message = clip(d.message, 5000);
  if (message.length < 3) return json({ ok: false, error: "message" }, 400);
  const email = clip(d.email, 254).toLowerCase();
  if (email && !/^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(email)) return json({ ok: false, error: "email" }, 400);
  const kind = KINDS.includes(d.kind) ? d.kind : "other";
  const platform = PLATFORMS.includes(d.platform) ? d.platform : "web";
  const v = await fetch("https://challenges.cloudflare.com/turnstile/v0/siteverify", { method: "POST",
    body: new URLSearchParams({ secret: env.TURNSTILE_SECRET, response: String(d.token || ""), remoteip: request.headers.get("CF-Connecting-IP") || "" }) });
  if (!(await v.json()).success) return json({ ok: false, error: "captcha" }, 403);
  await env.DB.prepare("INSERT INTO feedback (message, email, kind, platform, version, lang, ua) VALUES (?, ?, ?, ?, ?, ?, ?)")
    .bind(message, email || null, kind, platform, clip(d.version, 32) || null, d.lang === "en" ? "en" : "vi", clip(request.headers.get("user-agent"), 300)).run();
  return json({ ok: true });
}
