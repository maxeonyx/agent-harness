// Press "Dictate the sample" without a browser: the page's WebSocket, over TLS to this machine.
import tls from "node:tls";
const port = process.argv[2] || 4320;
const s = tls.connect({ host: "127.0.0.1", port, servername: "mx-hv-mj.local" }, () => {
  s.write(`GET /dictation?rate=16000&sample HTTP/1.1\r\nHost: mx-hv-mj.local:${port}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n`);
});
let buf = Buffer.alloc(0), head = false; const t0 = Date.now();
s.on("data", (d) => {
  buf = Buffer.concat([buf, d]);
  if (!head) { const i = buf.indexOf("\r\n\r\n"); if (i < 0) return; console.log(buf.subarray(0, buf.indexOf("\r\n")).toString()); buf = buf.subarray(i + 4); head = true; }
  for (;;) {
    if (buf.length < 2) return;
    let len = buf[1] & 127, off = 2;
    if (len === 126) { if (buf.length < 4) return; len = buf.readUInt16BE(2); off = 4; }
    else if (len === 127) { if (buf.length < 10) return; len = Number(buf.readBigUInt64BE(2)); off = 10; }
    if (buf.length < off + len) return;
    const op = buf[0] & 15, body = buf.subarray(off, off + len); buf = buf.subarray(off + len);
    if (op === 1) console.log(`[${((Date.now() - t0) / 1000).toFixed(1)}s]`, body.toString().slice(0, 300));
    if (op === 8) { console.log("closed"); process.exit(0); }
  }
});
s.on("close", () => { console.log("socket closed"); process.exit(0); });
setTimeout(() => process.exit(0), 40000);
