import http from 'node:http';
import https from 'node:https';

const TARGET_HOST = 'agentrouter.org';
const LOCAL_PORT = 4000;

// Точные заголовки, которые AgentRouter требует для OpenAI эндпоинта (Codex CLI)
function buildAgentRouterHeaders(incomingHeaders) {
  const headers = {};

  for (const [key, value] of Object.entries(incomingHeaders)) {
    const lower = key.toLowerCase();
    // Вычищаем любые упоминания aider, litellm, python, stainless
    if (
      lower.startsWith('x-stainless') ||
      lower.startsWith('x-aider') ||
      lower.startsWith('x-litellm') ||
      lower === 'user-agent' ||
      lower === 'host' ||
      lower === 'originator' ||
      lower === 'version'
    ) {
      continue;
    }
    headers[key] = value;
  }

  // Аутентичный белый список AgentRouter для OpenAI / Codex CLI
  headers['host'] = TARGET_HOST;
  headers['originator'] = 'codex_cli_rs';
  headers['user-agent'] = 'codex_cli_rs/0.101.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464';
  headers['version'] = '0.101.0';
  headers['accept-encoding'] = 'gzip, deflate, br';

  return headers;
}

const server = http.createServer((req, res) => {
  if (req.method === 'OPTIONS') {
    res.writeHead(200, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': '*',
    });
    res.end();
    return;
  }

  const outboundHeaders = buildAgentRouterHeaders(req.headers);

  const options = {
    hostname: TARGET_HOST,
    port: 443,
    path: req.url,
    method: req.method,
    headers: outboundHeaders,
  };

  const proxyReq = https.request(options, (proxyRes) => {
    res.writeHead(proxyRes.statusCode, proxyRes.headers);
    proxyRes.pipe(res);
  });

  proxyReq.on('error', (err) => {
    console.error(`[PROXY ERROR] ${err.message}`);
    res.writeHead(502, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Proxy Gateway Error', message: err.message }));
  });

  req.pipe(proxyReq);
});

server.listen(LOCAL_PORT, '127.0.0.1', () => {
  console.log(`=======================================================`);
  console.log(`[AGENTROUTER-PROXY] Listening on http://127.0.0.1:${LOCAL_PORT}`);
  console.log(`[AGENTROUTER-PROXY] Target: https://${TARGET_HOST}`);
  console.log(`[AGENTROUTER-PROXY] Camouflage: codex_cli_rs/0.101.0 (Codex CLI white-label)`);
  console.log(`=======================================================`);
});
