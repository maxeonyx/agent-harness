// A stand-in for Cloudflare Access and its tunnel in front of a brain:
//
//   node demos/deploy/access.mjs BRAIN-PORT EXPIRED-FILE
//
// It terminates TLS on a free port (printed first) with the machine's local
// certificate and forwards every request and WebSocket to the brain on
// localhost, as cloudflared does. While EXPIRED-FILE exists the session has
// expired: it drops every link it carries and answers every request with a
// 302 to a login page, as Access does. Each request goes to stdout.
import fs from "node:fs";
import https from "node:https";
import tls from "node:tls";

const [brain, expiredFile] = process.argv.slice(2);
const certs = `${process.env.HOME}/.config/dev-certs/shared-local`;
const cert = { cert: fs.readFileSync(`${certs}/wildcard-local.crt`), key: fs.readFileSync(`${certs}/wildcard-local.key`) };
// It checks the brain's certificate as cloudflared does: against the CA.
const ca = fs.readFileSync(`${certs}/ca/dev-ca.crt`);
const expired = () => fs.existsSync(expiredFile);
const login = "https://login.example.invalid/cdn-cgi/access/login";
const links = new Set();
setInterval(() => { if (expired()) for (const s of links) s.destroy(); }, 200);

const server = https.createServer(cert, (req, res) => {
  console.log(`${expired() ? "302" : "fwd"} ${req.method} ${req.url}`);
  if (expired()) return res.writeHead(302, { location: login }).end();
  const up = https.request({ host: "localhost", port: brain, path: req.url, method: req.method, headers: req.headers, ca }, (r) => {
    res.writeHead(r.statusCode, r.headers);
    r.pipe(res);
  });
  up.on("error", () => res.destroy());
  req.pipe(up);
});
server.on("upgrade", (req, socket, head) => {
  console.log(`${expired() ? "302" : "fwd"} link ${req.url}`);
  if (expired()) return socket.end(`HTTP/1.1 302 Found\r\nLocation: ${login}\r\nContent-Length: 0\r\n\r\n`);
  const up = tls.connect({ host: "localhost", port: brain, ca }, () => {
    const headers = Object.entries(req.headers).map(([k, v]) => `${k}: ${v}\r\n`).join("");
    up.write(`${req.method} ${req.url} HTTP/1.1\r\n${headers}\r\n`);
    up.write(head);
    socket.pipe(up).pipe(socket);
  });
  links.add(socket);
  socket.on("close", () => links.delete(socket));
  up.on("error", () => socket.destroy());
  socket.on("error", () => up.destroy());
});
server.listen(0, () => console.log(server.address().port));
