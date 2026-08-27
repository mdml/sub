import sys, json, subprocess, time, os, select
sid, cwd = sys.argv[1], sys.argv[2]; cmd = sys.argv[3:]
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd)
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"sub-spike-probe","version":"0"}}})
send({"jsonrpc":"2.0","id":2,"method":"session/resume","params":{"sessionId":sid,"cwd":cwd,"mcpServers":[]}})
t=time.time(); text=""; n=0; sent=False
while time.time()-t<240:
    r,_,_=select.select([p.stdout],[],[],1)
    if not r: continue
    line=p.stdout.readline()
    if not line: break
    try: o=json.loads(line)
    except: print("RAW", line[:300]); continue
    if o.get("id")==2:
        print("RESUME RESPONSE:", json.dumps(o)[:800]); 
        if "error" in o: break
        send({"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":sid,"prompt":[{"type":"text","text":"Without using any tools: what is the exact name of the unit test you wrote earlier in this session, and what command did you run? One line."}]}}); sent=True
    elif o.get("method")=="session/update":
        n+=1; u=o["params"]["update"]
        if u.get("sessionUpdate")=="agent_message_chunk" and u["content"].get("type")=="text": text+=u["content"]["text"]
    elif o.get("id")==3:
        print("PROMPT RESPONSE:", json.dumps(o)[:600]); break
    elif "method" in o and "id" in o:
        print("SERVER REQUEST:", o["method"]); 
print("updates:", n); print("TEXT:", text[:600])
p.kill(); print("STDERR:", p.stderr.read()[-800:])
