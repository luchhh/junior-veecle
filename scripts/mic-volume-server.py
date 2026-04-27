#!/usr/bin/env python3
import re
import subprocess
from flask import Flask, request, jsonify

app = Flask(__name__)

MAX_VAL = 16

def find_mic_card():
    out = subprocess.check_output(["arecord", "-l"], text=True)
    for line in out.splitlines():
        m = re.match(r"card (\d+):.*USB PnP", line)
        if m:
            return int(m.group(1))
    raise RuntimeError("USB PnP mic not found")

def get_volume():
    card = find_mic_card()
    out = subprocess.check_output(["amixer", "-c", str(card), "cget", "numid=3"], text=True)
    m = re.search(r": values=(\d+)", out)
    if m:
        return int(m.group(1))
    raise RuntimeError("Could not parse volume")

def set_volume(val):
    card = find_mic_card()
    val = max(0, min(MAX_VAL, val))
    subprocess.run(["amixer", "-c", str(card), "cset", "numid=3", str(val)], check=True)
    return val

@app.route("/")
def index():
    vol = get_volume()
    pct = round(vol / MAX_VAL * 100)
    return f"""<!DOCTYPE html>
<html>
<head>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Junior — Mic Volume</title>
  <style>
    body {{ font-family: sans-serif; max-width: 400px; margin: 60px auto; padding: 0 20px; text-align: center; }}
    h1 {{ font-size: 1.4em; margin-bottom: 8px; }}
    #pct {{ font-size: 3em; font-weight: bold; margin: 16px 0; }}
    input[type=range] {{ width: 100%; height: 40px; }}
    button {{ margin-top: 20px; padding: 12px 32px; font-size: 1em; cursor: pointer; }}
  </style>
</head>
<body>
  <h1>Junior — Mic Volume</h1>
  <div id="pct">{pct}%</div>
  <input type="range" id="slider" min="0" max="100" value="{pct}">
  <button onclick="apply()">Apply</button>
  <script>
    document.getElementById('slider').oninput = function() {{
      document.getElementById('pct').textContent = this.value + '%';
    }};
    function apply() {{
      const pct = document.getElementById('slider').value;
      fetch('/set?pct=' + pct, {{method: 'POST'}})
        .then(r => r.json())
        .then(d => document.getElementById('pct').textContent = d.pct + '%');
    }}
  </script>
</body>
</html>"""

@app.route("/set", methods=["POST"])
def set_vol():
    pct = int(request.args.get("pct", 0))
    val = round(pct / 100 * MAX_VAL)
    actual_val = set_volume(val)
    actual_pct = round(actual_val / MAX_VAL * 100)
    return jsonify(pct=actual_pct)

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
