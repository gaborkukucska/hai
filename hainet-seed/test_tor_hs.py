import base64
import hashlib
import os

with open(os.path.expanduser("~/.hainet/identity/ed25519_priv.b64"), "r") as f:
    priv_b64 = f.read().strip()

pkcs8 = base64.b64decode(priv_b64)
seed = pkcs8[-32:]

hasher = hashlib.sha512()
hasher.update(seed)
expanded = bytearray(hasher.digest())

expanded[0] &= 248
expanded[31] &= 127
expanded[31] |= 64

hs_key = b"== ed25519v1-secret: type0 ==\x00\x00\x00" + expanded

os.makedirs("/tmp/tor_test/hs", exist_ok=True)
with open("/tmp/tor_test/hs/hs_ed25519_secret_key", "wb") as f:
    f.write(hs_key)

os.chmod("/tmp/tor_test/hs", 0o700)
os.chmod("/tmp/tor_test/hs/hs_ed25519_secret_key", 0o600)

torrc = """DataDirectory /tmp/tor_test/data
SocksPort 9052
ControlPort 9053
HiddenServiceDir /tmp/tor_test/hs
HiddenServicePort 8080 127.0.0.1:8080
"""
with open("/tmp/tor_test/torrc", "w") as f:
    f.write(torrc)
