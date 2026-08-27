import sys, json, subprocess, time, select
sid, cwd = sys.argv[1], sys.argv[2]; cmd = sys.argv[3:]
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd)
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}})
send({"jsonrpc":"2.0","id":2,"method":"session/load","params":{"sessionId":sid,"cwd":cwd,"mcpServers":[]}})
t=time.time(); after=False; text=""
while time.time()-t<200:
    r,_,_=select.select([p.stdout],[],[],1)
    if not r: continue
    line=p.stdout.readline()
    if not line: break
    o=json.loads(line)
    if o.get("id")==2:
        after=True; send({"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":sid,"prompt":[{"type":"text","text":"Without using any tools: what is the exact name of the unit test you wrote earlier in this session? One line."}]}})
    elif after and o.get("method")=="session/update":
        u=o["params"]["update"]; print("UPD", u.get("sessionUpdate"), (u.get("content") or {}).get("text","")[:200])
    elif o.get("id")==3: print("PROMPT RESPONSE", json.dumps(o)[:300]); break
p.kill()
