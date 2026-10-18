"""Serve the web build over HTTPS and a public CA certificate over HTTP."""

import argparse
from functools import partial
from http.server import BaseHTTPRequestHandler, SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import ssl
import threading


def main():
    root = Path(__file__).resolve().parent.parent
    certificates = root / ".tmp" / "local-https"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--https-port", type=int, default=8443)
    parser.add_argument("--certificate-port", type=int, default=8081)
    args = parser.parse_args()

    certificate = (certificates / "patience-dev-ca.cer").read_bytes()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(certificates / "server.pem", certificates / "server-key.pem")

    class GameHandler(SimpleHTTPRequestHandler):
        def end_headers(self):
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

    class CertificateHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path.split("?", 1)[0] != "/patience-dev-ca.cer":
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", "application/x-x509-ca-cert")
            self.send_header("Content-Length", str(len(certificate)))
            self.send_header("Content-Disposition", 'inline; filename="patience-dev-ca.cer"')
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(certificate)

    game_server = ThreadingHTTPServer(
        (args.bind, args.https_port), partial(GameHandler, directory=str(root / "dist"))
    )
    game_server.socket = context.wrap_socket(game_server.socket, server_side=True)
    certificate_server = ThreadingHTTPServer(
        (args.bind, args.certificate_port), CertificateHandler
    )
    threading.Thread(target=certificate_server.serve_forever, daemon=True).start()
    print(f"Certificate: http://{args.bind}:{args.certificate_port}/patience-dev-ca.cer", flush=True)
    print(f"Game: https://{args.bind}:{args.https_port}/", flush=True)
    try:
        game_server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        certificate_server.shutdown()
        certificate_server.server_close()
        game_server.server_close()


if __name__ == "__main__":
    main()
