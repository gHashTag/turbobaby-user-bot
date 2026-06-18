#!/usr/bin/env python3
"""Headless-Chrome smoke check over the DevTools Protocol.

Loads a URL in real Chrome, captures console errors / thrown exceptions / error
log entries, and confirms the Dioxus app actually mounted (DOM rendered + a known
marker present). Exits 0 only when the console is clean AND the app rendered.

Why this exists: `cargo check`/`cargo test`/`trunk build` only COMPILE the wasm
and emit the wasm-bindgen JS glue — none of them execute it. Module-load errors
(e.g. a wasm-bindgen CLI/crate version skew producing "Importing binding name X
is not found") only surface when a browser instantiates the module. This is the
gate that catches them before dist/ is committed. See the 2026-06-18 incident.

Usage:  python3 scripts/cdp_smoke.py <url> [marker]
Env:    CHROME_BIN (path to Chrome), CDP_PORT, SMOKE_WAIT_S (post-load settle)
"""
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request

try:
    import websocket  # websocket-client
except ImportError:
    print("SMOKE FAIL: python module 'websocket-client' is required "
          "(pip install websocket-client)")
    sys.exit(2)

URL = sys.argv[1] if len(sys.argv) > 1 else None
MARKER = sys.argv[2] if len(sys.argv) > 2 else "WOODY"
if not URL:
    print("usage: cdp_smoke.py <url> [marker]")
    sys.exit(2)

CHROME = os.environ.get(
    "CHROME_BIN",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
)
PORT = int(os.environ.get("CDP_PORT", str(random.randint(9210, 9790))))
SETTLE_S = float(os.environ.get("SMOKE_WAIT_S", "6"))
MIN_DOM = 120  # floor only; the real mount signal is the marker (below)

if not os.path.exists(CHROME):
    print(f"SMOKE FAIL: Chrome not found at '{CHROME}' (set CHROME_BIN)")
    sys.exit(2)

profile = tempfile.mkdtemp(prefix="wwb-smoke-")
chrome = subprocess.Popen(
    [CHROME, "--headless=new", "--disable-gpu", "--no-first-run",
     "--no-default-browser-check", "--disable-extensions",
     # Chrome >= 111 rejects CDP websocket connections unless the origin is
     # explicitly allowed.
     "--remote-allow-origins=*",
     f"--remote-debugging-port={PORT}", f"--user-data-dir={profile}",
     "about:blank"],
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
)


def cleanup():
    try:
        chrome.terminate()
        chrome.wait(timeout=5)
    except Exception:
        try:
            chrome.kill()
        except Exception:
            pass
    shutil.rmtree(profile, ignore_errors=True)


def fail(msg):
    print(f"SMOKE FAIL: {msg}")
    cleanup()
    sys.exit(1)


# Find a page target's websocket URL.
ws_url = None
for _ in range(60):
    try:
        targets = json.load(
            urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json", timeout=1)
        )
        for t in targets:
            if t.get("type") == "page" and t.get("webSocketDebuggerUrl"):
                ws_url = t["webSocketDebuggerUrl"]
                break
        if ws_url:
            break
    except Exception:
        pass
    time.sleep(0.25)

if not ws_url:
    fail("could not reach Chrome DevTools endpoint")

try:
    ws = websocket.create_connection(
        ws_url, timeout=30, max_size=None,
        suppress_origin=True,  # Chrome >= 111 rejects unexpected Origin headers
    )
except Exception as e:
    fail(f"DevTools websocket connect failed: {e}")
_msg_id = 0
fatal = []  # frontend is broken (uncaught JS exceptions, module-load failures)
notes = []  # expected noise: API 404s (no backend on a static serve), extensions


def send(method, params=None):
    global _msg_id
    _msg_id += 1
    ws.send(json.dumps({"id": _msg_id, "method": method, "params": params or {}}))
    return _msg_id


def classify(msg):
    """Sort a CDP event into fatal frontend errors vs expected noise.

    FATAL = uncaught JS exceptions (this is how a wasm-bindgen module-load
    failure surfaces, e.g. "Importing binding name 'get_select_data' is not
    found") and non-network error log entries.
    NOISE = network resource 404s (the static smoke serve has no /api backend,
    so the app's own fetches 404 — that is the app WORKING) and console.error
    (noisy without a backend). Noise is reported but does not fail the gate.
    """
    m = msg.get("method")
    p = msg.get("params", {})
    if m == "Runtime.exceptionThrown":
        d = p.get("exceptionDetails", {})
        txt = (d.get("exception", {}) or {}).get("description") or d.get("text") \
            or "exception"
        fatal.append("uncaught exception: " + str(txt).splitlines()[0][:300])
    elif m == "Runtime.consoleAPICalled" and p.get("type") == "error":
        args = " ".join(str(a.get("value", a.get("description", "")))
                        for a in p.get("args", []))
        notes.append("console.error: " + args[:200])
    elif m == "Log.entryAdded":
        e = p.get("entry", {})
        if e.get("level") == "error":
            src = e.get("source", "")
            text = str(e.get("text", ""))[:200]
            if src == "network":
                notes.append("network 404 (no backend on static serve): " + text)
            else:
                fatal.append(f"log.error[{src}]: " + text)


# Enable domains BEFORE navigating so load-time errors are captured.
send("Runtime.enable")
send("Log.enable")
send("Page.enable")
send("Page.navigate", {"url": URL})

# Pump events while the page loads + wasm initialises.
deadline = time.time() + 12 + SETTLE_S
while time.time() < deadline:
    ws.settimeout(1.0)
    try:
        classify(json.loads(ws.recv()))
    except websocket.WebSocketTimeoutException:
        continue
    except Exception:
        break

# Check the app actually mounted.
expr = ("JSON.stringify({len: document.body.innerText.length, "
        "marker: document.body.innerText.indexOf(%s) >= 0})"
        % json.dumps(MARKER))
eval_id = send("Runtime.evaluate",
               {"expression": expr, "returnByValue": True})
dom = None
edl = time.time() + 8
while time.time() < edl:
    ws.settimeout(1.0)
    try:
        msg = json.loads(ws.recv())
    except websocket.WebSocketTimeoutException:
        continue
    except Exception:
        break
    classify(msg)
    if msg.get("id") == eval_id:
        try:
            dom = json.loads(msg["result"]["result"]["value"])
        except Exception:
            dom = None
        break

try:
    ws.close()
except Exception:
    pass

# Verdict. The app mounting at all is proven by the marker (rendered ONLY by the
# Rust app, never present in the index.html skeleton). Fatal JS errors fail hard.
problems = []
if fatal:
    problems.append(f"{len(fatal)} fatal frontend error(s):")
    for e in fatal[:10]:
        problems.append("   • " + e)
if dom is None:
    problems.append("could not read the DOM (page may not have loaded)")
elif not dom.get("marker"):
    problems.append(f"app did NOT mount — marker '{MARKER}' absent "
                    f"(DOM text len {dom.get('len')})")
elif dom.get("len", 0) < MIN_DOM:
    problems.append(f"DOM suspiciously small (len {dom.get('len')} < {MIN_DOM})")

cleanup()

if notes:
    print(f"  (ignored {len(notes)} expected note(s), e.g. {notes[0]})")

if problems:
    print("SMOKE FAIL:")
    for p in problems:
        print("  " + p)
    sys.exit(1)

print(f"SMOKE OK: app mounted (marker '{MARKER}' present, DOM len {dom['len']}), "
      f"no uncaught JS errors")
sys.exit(0)
