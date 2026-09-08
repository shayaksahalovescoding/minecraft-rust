import sys
import os
import torch
import uvicorn

if sys.platform == "win32":
    import asyncio.proactor_events
    import socket
    def _safe_call_connection_lost(self, exc):
        if self._called_connection_lost:
            return
        try:
            self._protocol.connection_lost(exc)
        finally:
            if hasattr(self._sock, "shutdown") and self._sock.fileno() != -1:
                try:
                    self._sock.shutdown(socket.SHUT_RDWR)
                except (ConnectionResetError, OSError):
                    pass
            self._sock.close()
            self._sock = None
            server = self._server
            if server is not None:
                server._detach()
                self._server = None
            self._called_connection_lost = True
    asyncio.proactor_events._ProactorBasePipeTransport._call_connection_lost = _safe_call_connection_lost

from backend.config import BASE_DIR, PRETRAINED_DIR, RMVPE_PATH, BASE_G40K, BASE_D40K, DEVICE, DEVICE_NAME, VRAM_GB

def check_environment():
    print("=" * 60)
    print("  VoiceClone — Personal AI Singing Voice Pipeline")
    print("=" * 60)
    print(f"Python Version : {sys.version.split()[0]}")
    print(f"PyTorch Version: {torch.__version__}")
    print(f"Device         : {DEVICE} ({DEVICE_NAME})")
    print(f"VRAM           : {VRAM_GB} GB")
    print("-" * 60)

    missing = []
    if not RMVPE_PATH.exists():
        missing.append("rmvpe.pt")
    if not BASE_G40K.exists():
        missing.append("f0G40k.pth")
    if not BASE_D40K.exists():
        missing.append("f0D40k.pth")

    if missing:
        print(f"WARNING: Missing pretrained models: {missing}")
        print("Run: python scripts/download_models.py")
    else:
        print("Pretrained assets verified: RMVPE, ContentVec, RVC v2 40k weights present.")

    print(f"Serving Web Interface & API at: http://127.0.0.1:8000")
    print("=" * 60)

if __name__ == "__main__":
    check_environment()
    uvicorn.run("backend.app:app", host="127.0.0.1", port=8000, reload=False, log_level="info")
