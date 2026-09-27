"""极简本地压测目标：支持 GET/POST/PUT/DELETE，/ping 返回 200，/fail 返回 404，/slow 延迟 200ms，/stats 返回请求计数。

请求日志实时写入同目录 server.log（证明压测请求真实到达），同时输出到控制台。
实时查看：PowerShell 执行  Get-Content -Wait server.log
"""
import json
import logging
import os
import time
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'server.log')
logging.basicConfig(
    level=logging.INFO,
    format='[%(asctime)s] %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S',
    handlers=[
        logging.FileHandler(LOG_FILE, encoding='utf-8'),
        logging.StreamHandler(),
    ],
)

counter = Counter()

class ThreadingHTTPServerHighBacklog(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 256  # 默认 5，高并发下会连接被拒

class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'  # 支持 keepalive，配合 Content-Length
    def _handle(self):
        length = int(self.headers.get('Content-Length', 0) or 0)
        body = self.rfile.read(length) if length else b''
        path = self.path.split('?')[0]
        counter[path] += 1
        if body.strip():
            try:
                json.loads(body)
                counter['body_json_ok'] += 1
            except Exception:
                counter['body_json_bad'] += 1

        if path == '/stats':
            resp = json.dumps(dict(counter)).encode()
            status = 200
        elif path == '/fail':
            resp = json.dumps({"error": "not found"}).encode()
            status = 404
        elif path == '/slow':
            time.sleep(0.2)
            resp = b'{"ok":true}'
            status = 200
        else:
            resp = json.dumps({"method": self.command, "path": path,
                               "body": body.decode('utf-8', 'replace')}).encode()
            status = 200

        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)

        # 每请求一条日志：方法、路径、状态码、请求体字节数、body 预览、来源
        preview = body[:100].decode('utf-8', 'replace')
        logging.info('%s %s -> %d req_bytes=%d body=%r from=%s',
                     self.command, path, status, length, preview, self.client_address[0])

    do_GET = do_POST = do_PUT = do_DELETE = _handle
    def log_message(self, *args): pass  # 静默访问日志

ThreadingHTTPServerHighBacklog(('127.0.0.1', 8088), Handler).serve_forever()
