import json, os, urllib.request, concurrent.futures as cf
key = open(os.path.expanduser('~/.secrets/anthropic_api_key')).read().strip().split('=')[-1]
topics = ["dev team standup, bugs, deploy, PR review", "startup product meeting, KPI, roadmap", "friends planning a trip, booking hotel",
 "office HR, leave request, meeting room", "ecommerce shop owner talking to customers", "marketing team, ads campaign, content",
 "gaming friends, rank, match", "family group chat", "university students, deadline, assignment", "designers, Figma, feedback",
 "crypto and stock investors chatting", "customer support for a SaaS app", "data team, dashboard, SQL query", "freelancer talking to a client",
 "couple chatting casually", "devops, server down, incident", "mobile app team, TestFlight, App Store review", "AI tools, ChatGPT, prompt",
 "food delivery and restaurants", "football fans", "remote work, Zoom, Slack", "real estate agent and buyer"]
def ask(i):
    t = topics[i % len(topics)]
    prompt = (f"Write 45 realistic chat messages (Slack/Messenger/Zalo style) about: {t}. "
              "Written by Vietnamese people the way they actually type: Vietnamese with full diacritics, naturally mixing in English words, "
              "tech terms and sometimes whole English phrases or sentences. About 1 in 8 messages fully in English. Casual, varied length (3 to 40 words). "
              "No emoji, no names prefix, no numbering, no quotes. One message per line, nothing else.")
    body = {"model": "claude-haiku-4-5", "max_tokens": 4000, "messages": [{"role": "user", "content": prompt}]}
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(),
        headers={"x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json"})
    d = json.load(urllib.request.urlopen(req, timeout=120))
    return d["content"][0]["text"]
with cf.ThreadPoolExecutor(6) as ex:
    res = list(ex.map(ask, range(44)))
lines = [l.strip() for r in res for l in r.split('\n') if len(l.split()) >= 3]
open('chat_messages.txt', 'w', encoding='utf-8').write('\n'.join(lines))
print(len(lines), 'messages', sum(len(l.split()) for l in lines), 'words')
