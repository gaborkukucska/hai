// Generates cross-language test vectors for hainet-noslop by running the SAME libraries the NoSlop
// Android app uses (Gson + BouncyCastle) through code that mirrors the Kotlin functions line by line:
// CryptoService.encodeForSigning / sign / deriveOnionAddress / getRawEd25519Seed / deriveDirectionalKeyV2 /
// canonicalDmAadV2 / encryptDMV2, EnvelopeAuth.payloadDigest / signingString,
// MediaManager.mediaRequestSigningString, canonicalHandshakePayloadV2, canonicalAnnouncePeerV2.
import com.google.gson.*;
import org.bouncycastle.crypto.params.*;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.bouncycastle.crypto.util.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;

public class GenVectors {
    static final byte[] PKCS8 = {0x30,0x2e,0x02,0x01,0x00,0x30,0x05,0x06,0x03,0x2b,0x65,0x70,0x04,0x22,0x04,0x20};
    static final byte[] X509 = {0x30,0x2a,0x30,0x05,0x06,0x03,0x2b,0x65,0x70,0x03,0x21,0x00};
    static Base64.Encoder B64 = Base64.getEncoder();

    static String encodeForSigning(String... parts) {
        StringBuilder sb = new StringBuilder();
        for (String p : parts) { if (p == null) sb.append("0:"); else sb.append(p.length()).append(':').append(p); }
        return sb.toString();
    }
    static byte[] cat(byte[] a, byte[] b) { byte[] r = Arrays.copyOf(a, a.length + b.length); System.arraycopy(b, 0, r, a.length, b.length); return r; }
    static String hex(byte[] d) { StringBuilder sb = new StringBuilder(); for (byte b : d) sb.append(String.format("%02x", b & 0xff)); return sb.toString(); }
    static String sha256Hex(String s) throws Exception { return hex(MessageDigest.getInstance("SHA-256").digest(s.getBytes(StandardCharsets.UTF_8))); }

    static JsonElement withoutNullMembers(JsonElement e) {
        if (e.isJsonObject()) { JsonObject o = new JsonObject(); for (Map.Entry<String, JsonElement> en : e.getAsJsonObject().entrySet()) if (!en.getValue().isJsonNull()) o.add(en.getKey(), withoutNullMembers(en.getValue())); return o; }
        if (e.isJsonArray()) { JsonArray a = new JsonArray(); for (JsonElement v : e.getAsJsonArray()) a.add(withoutNullMembers(v)); return a; }
        return e;
    }
    static String rendered(JsonElement payload) { return (payload == null || payload.isJsonNull()) ? "" : withoutNullMembers(payload).toString(); }

    static String sign(String payload, byte[] seed) {
        Ed25519Signer s = new Ed25519Signer(); s.init(true, new Ed25519PrivateKeyParameters(seed, 0));
        byte[] m = payload.getBytes(StandardCharsets.UTF_8); s.update(m, 0, m.length); return B64.encodeToString(s.generateSignature());
    }
    static String base32(byte[] bytes) { // CryptoService.base32 (RFC 4648 lower-case, no padding)
        String alpha = "abcdefghijklmnopqrstuvwxyz234567"; StringBuilder sb = new StringBuilder(); int buffer = 0, bits = 0;
        for (byte b : bytes) { buffer = (buffer << 8) | (b & 0xff); bits += 8; while (bits >= 5) { sb.append(alpha.charAt((buffer >> (bits - 5)) & 31)); bits -= 5; } }
        if (bits > 0) sb.append(alpha.charAt((buffer << (5 - bits)) & 31)); return sb.toString();
    }
    static String onion(byte[] rawPub) {
        org.bouncycastle.crypto.digests.SHA3Digest d = new org.bouncycastle.crypto.digests.SHA3Digest(256);
        byte[] in = cat(cat(".onion checksum".getBytes(StandardCharsets.UTF_8), rawPub), new byte[]{3});
        byte[] h = new byte[32]; d.update(in, 0, in.length); d.doFinal(h, 0);
        String enc = base32(cat(cat(rawPub, new byte[]{h[0], h[1]}), new byte[]{3}));
        while (enc.length() < 56) enc += "a"; return enc.substring(0, 56) + ".onion";
    }

    public static void main(String[] a) throws Exception {
        JsonObject out = new JsonObject();

        // --- encodeForSigning (UTF-16 lengths) ---
        JsonArray efs = new JsonArray();
        String[][] cases = { {"noslop-env-v1", "", "abc"}, {"héllo", "😀x", null, "日本"}, {"", null, ""} };
        for (String[] c : cases) {
            JsonObject v = new JsonObject(); JsonArray parts = new JsonArray();
            for (String p : c) parts.add(p == null ? JsonNull.INSTANCE : new JsonPrimitive(p));
            v.add("parts", parts); v.addProperty("encoded", encodeForSigning(c)); efs.add(v);
        }
        out.add("encode_for_signing", efs);

        // --- payload rendering / digest, as a RECEIVER sees it (parse wire text, then render) ---
        String[] wires = {
            "{\"a\":1,\"b\":null,\"c\":[1,null,{\"d\":null,\"e\":\"x\"}],\"z\":{}}",
            "{\"s\":\"quote\\\" back\\\\ nl\\n tab\\t cr\\r bs\\b ff\\f nul\\u0000 us\\u001f del\\u007f\"}",
            "{\"u\":\"ls\\u2028 ps\\u2029 emoji😀 é <html>&='\",\"slash\":\"a/b\\/c\"}",
            "{\"n1\":1700000000000,\"n2\":-5,\"n3\":1.50,\"n4\":1e3,\"n5\":0,\"t\":true,\"f\":false}",
            "{\"author_avatar_b64\":\"QUJD+/==\",\"empty\":\"\",\"arr\":[],\"nested\":{\"x\":[[null]]}}",
            "[]", "\"just a string\"", "null"
        };
        JsonArray rd = new JsonArray();
        for (String w : wires) {
            JsonElement parsed = JsonParser.parseString(w);
            JsonObject v = new JsonObject(); v.addProperty("wire", w);
            v.addProperty("rendered", rendered(parsed)); v.addProperty("digest", sha256Hex(rendered(parsed)));
            rd.add(v);
        }
        out.add("payload_digest", rd);

        // --- Identity from a fixed seed (deterministic) ---
        byte[] seed = new byte[32]; for (int i = 0; i < 32; i++) seed[i] = (byte) (i * 7 + 3);
        Ed25519PrivateKeyParameters priv = new Ed25519PrivateKeyParameters(seed, 0);
        byte[] rawPub = priv.generatePublicKey().getEncoded();
        String pubB64 = B64.encodeToString(cat(X509, rawPub)), privB64 = B64.encodeToString(cat(PKCS8, seed));
        byte[] xseed = new byte[32]; for (int i = 0; i < 32; i++) xseed[i] = (byte) (255 - i * 3);
        X25519PrivateKeyParameters xpriv = new X25519PrivateKeyParameters(xseed, 0);
        String xPubB64 = B64.encodeToString(SubjectPublicKeyInfoFactory.createSubjectPublicKeyInfo(xpriv.generatePublicKey()).getEncoded());
        String xPrivB64 = B64.encodeToString(PrivateKeyInfoFactory.createPrivateKeyInfo(xpriv).getEncoded());
        JsonObject id = new JsonObject();
        id.addProperty("seed_hex", hex(seed)); id.addProperty("public_key_b64", pubB64); id.addProperty("private_key_b64", privB64);
        id.addProperty("onion", onion(rawPub)); id.addProperty("enc_public_key_b64", xPubB64); id.addProperty("enc_private_key_b64", xPrivB64);
        // getRawEd25519Seed: Tor ADD_ONION expanded secret (SHA-512 + clamp)
        byte[] exp = MessageDigest.getInstance("SHA-512").digest(seed);
        exp[0] &= (byte) 248; exp[31] &= (byte) 127; exp[31] |= (byte) 64;
        id.addProperty("tor_expanded_b64", B64.encodeToString(exp));
        out.add("identity", id);

        // --- Envelope: sign a packet exactly as EnvelopeAuth.sign does ---
        Gson gson = new Gson();
        JsonObject payload = new JsonObject();
        payload.addProperty("author_id", pubB64); payload.addProperty("onion_address", onion(rawPub));
        payload.addProperty("timestamp", 1763700000000L); payload.addProperty("signature", "c2ln");
        String envSigning = encodeForSigning("noslop-env-v1", "pkt-1", "ANNOUNCE_PEER", pubB64, "", "c2ln", "1763700000123", sha256Hex(rendered(payload)));
        JsonObject pkt = new JsonObject();
        pkt.addProperty("id", "pkt-1"); pkt.addProperty("hops", 3); pkt.addProperty("sender_id", pubB64);
        pkt.addProperty("signature", "c2ln"); pkt.addProperty("type", "ANNOUNCE_PEER"); pkt.add("payload", payload);
        pkt.addProperty("env_v", 1); pkt.addProperty("env_ts", 1763700000123L); pkt.addProperty("env_sig", sign(envSigning, seed));
        JsonObject env = new JsonObject(); env.addProperty("signing_string", envSigning); env.addProperty("packet_wire", gson.toJson(pkt));
        out.add("envelope", env);

        // --- Other canonical strings ---
        JsonObject canon = new JsonObject();
        canon.addProperty("announce_v2", encodeForSigning("noslop-announce-v2", pubB64, onion(rawPub), "1763700000000"));
        canon.addProperty("announce_v2_sig", sign(encodeForSigning("noslop-announce-v2", pubB64, onion(rawPub), "1763700000000"), seed));
        String hs = encodeForSigning("noslop-hs-v2", pubB64, "Alice.abc12345", onion(rawPub), xPubB64, "TARGETKEY", "nonce123", "1763700000000", null, "bio 😀");
        canon.addProperty("handshake_v2", hs); canon.addProperty("handshake_v2_sig", sign(hs, seed));
        String mr = encodeForSigning("noslop-media-req-v1", pubB64, "TARGETKEY", "media_1", "0", "262144", "0", "262144", "1763700000000");
        canon.addProperty("media_req", mr); canon.addProperty("media_req_sig", sign(mr, seed));
        String disc = pubB64 + ":Creator.abc:" + onion(rawPub) + ":" + xPubB64 + ":true:https://x.y/z::my bio:1763700000000";
        canon.addProperty("discoverable", disc); canon.addProperty("discoverable_sig", sign(disc, seed));
        out.add("canonical", canon);

        // --- DM v2 with a fixed IV (directional HKDF + AAD), second party from another seed ---
        byte[] yseed = new byte[32]; for (int i = 0; i < 32; i++) yseed[i] = (byte) (i * 11 + 1);
        X25519PrivateKeyParameters ypriv = new X25519PrivateKeyParameters(yseed, 0);
        String yPubB64 = B64.encodeToString(SubjectPublicKeyInfoFactory.createSubjectPublicKeyInfo(ypriv.generatePublicKey()).getEncoded());
        String yPrivB64 = B64.encodeToString(PrivateKeyInfoFactory.createPrivateKeyInfo(ypriv).getEncoded());
        org.bouncycastle.crypto.agreement.X25519Agreement ag = new org.bouncycastle.crypto.agreement.X25519Agreement();
        ag.init(xpriv); byte[] shared = new byte[32]; ag.calculateAgreement(ypriv.generatePublicKey(), shared, 0);
        String senderEd = pubB64, recipientEd = "admin_" + pubB64;
        String s1 = senderEd.compareTo(recipientEd) <= 0 ? senderEd : recipientEd, s2 = senderEd.compareTo(recipientEd) <= 0 ? recipientEd : senderEd;
        MessageDigest sd = MessageDigest.getInstance("SHA-256"); sd.update(s1.getBytes(StandardCharsets.UTF_8)); sd.update(s2.getBytes(StandardCharsets.UTF_8)); byte[] salt = sd.digest();
        org.bouncycastle.crypto.generators.HKDFBytesGenerator hk = new org.bouncycastle.crypto.generators.HKDFBytesGenerator(new org.bouncycastle.crypto.digests.SHA256Digest());
        hk.init(new HKDFParameters(shared, salt, ("noslop-dm-v2|" + senderEd + "->" + recipientEd).getBytes(StandardCharsets.UTF_8)));
        byte[] kdir = new byte[32]; hk.generateBytes(kdir, 0, 32);
        byte[] iv = new byte[12]; for (int i = 0; i < 12; i++) iv[i] = (byte) (i + 100);
        String aad = encodeForSigning("noslop-dm-v2", senderEd, recipientEd, "msg-1", "", "1763700000000");
        org.bouncycastle.crypto.modes.ChaCha20Poly1305 cc = new org.bouncycastle.crypto.modes.ChaCha20Poly1305();
        cc.init(true, new AEADParameters(new KeyParameter(kdir), 128, iv, aad.getBytes(StandardCharsets.UTF_8)));
        byte[] pt = "{\"content\":\"hi 😀\"}".getBytes(StandardCharsets.UTF_8);
        byte[] ct = new byte[cc.getOutputSize(pt.length)]; int l = cc.processBytes(pt, 0, pt.length, ct, 0); cc.doFinal(ct, l);
        JsonObject dm = new JsonObject();
        dm.addProperty("sender_enc_priv_b64", xPrivB64); dm.addProperty("recipient_enc_pub_b64", yPubB64); dm.addProperty("recipient_enc_priv_b64", yPrivB64);
        dm.addProperty("sender_enc_pub_b64", xPubB64); dm.addProperty("sender_ed", senderEd); dm.addProperty("recipient_ed", recipientEd);
        dm.addProperty("msg_id", "msg-1"); dm.addProperty("timestamp", 1763700000000L); dm.addProperty("aad", aad);
        dm.addProperty("key_hex", hex(kdir)); dm.addProperty("iv_b64", B64.encodeToString(iv)); dm.addProperty("plaintext", new String(pt, StandardCharsets.UTF_8));
        dm.addProperty("ciphertext_b64", B64.encodeToString(ct));
        out.add("dm_v2", dm);

        System.out.println(new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(out));
    }
}
