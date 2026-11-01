// Known vectors for Horizon Studio's voice chat tests, written the way Simple
// Voice Chat writes them: FriendlyByteBuf's encodings (big-endian, VarInt
// lengths, UUIDs as two longs) and Secret.encrypt (AES/GCM/NoPadding, a 12-byte
// IV in front, a 128-bit tag), with a fixed IV instead of a random one.
import javax.crypto.Cipher;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

public class Vectors {
    static class Buf {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        DataOutputStream out = new DataOutputStream(bytes);
        Buf writeByte(int b) throws Exception { out.writeByte(b); return this; }
        Buf writeBoolean(boolean b) throws Exception { out.writeBoolean(b); return this; }
        Buf writeShort(int s) throws Exception { out.writeShort(s); return this; }
        Buf writeInt(int i) throws Exception { out.writeInt(i); return this; }
        Buf writeLong(long l) throws Exception { out.writeLong(l); return this; }
        Buf writeFloat(float f) throws Exception { out.writeFloat(f); return this; }
        Buf writeDouble(double d) throws Exception { out.writeDouble(d); return this; }
        Buf writeBytes(byte[] b) throws Exception { out.write(b); return this; }
        // FriendlyByteBuf.writeVarInt / VarInt.write
        Buf writeVarInt(int value) throws Exception {
            while ((value & -128) != 0) {
                out.writeByte(value & 127 | 128);
                value >>>= 7;
            }
            out.writeByte(value);
            return this;
        }
        Buf writeUUID(UUID u) throws Exception { writeLong(u.getMostSignificantBits()); writeLong(u.getLeastSignificantBits()); return this; }
        Buf writeByteArray(byte[] b) throws Exception { writeVarInt(b.length); writeBytes(b); return this; }
        Buf writeUtf(String s) throws Exception { return writeByteArray(s.getBytes(StandardCharsets.UTF_8)); }
        byte[] array() { return bytes.toByteArray(); }
    }

    // Secret.encrypt, with the IV given.
    static byte[] encrypt(byte[] key, byte[] iv, byte[] data) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "AES"), new GCMParameterSpec(128, iv));
        byte[] enc = cipher.doFinal(data);
        byte[] payload = new byte[iv.length + enc.length];
        System.arraycopy(iv, 0, payload, 0, iv.length);
        System.arraycopy(enc, 0, payload, iv.length, enc.length);
        return payload;
    }

    static String hex(byte[] b) {
        StringBuilder s = new StringBuilder();
        for (byte x : b) s.append(String.format("%02x", x));
        return s.toString();
    }

    public static void main(String[] args) throws Exception {
        byte[] key = new byte[16];
        for (int i = 0; i < 16; i++) key[i] = (byte) (0xA0 + i);
        byte[] iv = new byte[12];
        for (int i = 0; i < 12; i++) iv[i] = (byte) (0x10 + i);
        UUID connection = UUID.fromString("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0");
        UUID player = UUID.fromString("12345678-9abc-def0-1234-56789abcdef0");
        UUID other = UUID.fromString("cafebabe-0000-4000-8000-00000000beef");

        // MicPacket.toBytes in NetworkMessage.write: type 0x01, ByteArray, Long, Boolean.
        byte[] opus = new byte[]{(byte) 0xf8, (byte) 0xff, (byte) 0xfe, 0x01, 0x02};
        byte[] mic = new Buf().writeByte(0x01).writeByteArray(opus).writeLong(7L).writeBoolean(false).array();
        byte[] micEnc = encrypt(key, iv, mic);
        // ClientNetworkMessage: MAGIC, UUID, ByteArray(encrypted)
        byte[] micDatagram21 = new Buf().writeByte(0xFF).writeUUID(connection).writeByteArray(micEnc).array();
        byte[] micDatagram20 = new Buf().writeByte(0xFF).writeUUID(player).writeByteArray(micEnc).array();
        System.out.println("mic_plain=" + hex(mic));
        System.out.println("mic_datagram_21=" + hex(micDatagram21));
        System.out.println("mic_datagram_20=" + hex(micDatagram20));

        // PlayerSoundPacket: whisper and a category; NetworkMessage.writeServer: MAGIC, ByteArray.
        byte[] sound = new Buf().writeByte(0x02).writeUUID(other).writeUUID(other).writeByteArray(opus)
                .writeLong(42L).writeFloat(24.0f).writeByte(0x01 | 0x02).writeUtf("proximity").array();
        byte[] soundDatagram = new Buf().writeByte(0xFF).writeByteArray(encrypt(key, iv, sound)).array();
        System.out.println("sound_plain=" + hex(sound));
        System.out.println("sound_datagram=" + hex(soundDatagram));

        // LocationSoundPacket and GroupSoundPacket, no category.
        byte[] location = new Buf().writeByte(0x04).writeUUID(other).writeUUID(player).writeDouble(1.5).writeDouble(-64.0).writeDouble(1e3)
                .writeByteArray(opus).writeLong(3L).writeFloat(16.0f).writeByte(0).array();
        System.out.println("location_plain=" + hex(location));
        byte[] group = new Buf().writeByte(0x03).writeUUID(other).writeUUID(player).writeByteArray(opus).writeLong(9L).writeByte(0).array();
        System.out.println("group_plain=" + hex(group));

        // AuthenticatePacket (compat 20: player UUID, secret), ConnectionCheck (21: nonce), Ping.
        System.out.println("auth20_plain=" + hex(new Buf().writeByte(0x05).writeUUID(player).writeBytes(key).array()));
        byte[] nonce = new byte[16];
        for (int i = 0; i < 16; i++) nonce[i] = (byte) (0x60 + i);
        System.out.println("check21_plain=" + hex(new Buf().writeByte(0x09).writeBytes(nonce).array()));
        System.out.println("ping_plain=" + hex(new Buf().writeByte(0x07).writeUUID(other).writeLong(1730000000000L).array()));

        // SecretPacket.toBytes, compat 21 then 20.
        Buf s21 = new Buf().writeBytes(key).writeUUID(connection).writeInt(24454).writeUUID(player).writeByte(0)
                .writeInt(1275).writeDouble(48.0).writeInt(1000).writeBoolean(true).writeUtf("").writeBoolean(false);
        Buf s20 = new Buf().writeBytes(key).writeInt(24454).writeUUID(player).writeByte(0)
                .writeInt(1275).writeDouble(48.0).writeInt(1000).writeBoolean(true).writeUtf("voice.example.org:24455").writeBoolean(true);
        System.out.println("secret21=" + hex(s21.array()));
        System.out.println("secret20=" + hex(s20.array()));

        // PlayerStatesPacket with two states: compat 21 (flags byte), compat 20.
        UUID groupId = UUID.fromString("00000000-0000-0000-0000-0000000000aa");
        Buf st21 = new Buf().writeInt(2);
        st21.writeByte(0x01 | 0x10).writeUUID(player).writeUtf("Steve");
        st21.writeByte(0x04 | 0x20).writeUUID(other).writeUtf("Alex").writeUUID(groupId);
        System.out.println("states21=" + hex(st21.array()));
        Buf st20 = new Buf().writeInt(2);
        st20.writeBoolean(false).writeBoolean(true).writeUUID(player).writeUtf("Steve").writeBoolean(false);
        st20.writeBoolean(true).writeBoolean(false).writeUUID(other).writeUtf("Alex").writeBoolean(true).writeUUID(groupId);
        System.out.println("states20=" + hex(st20.array()));

        // ClientGroup (add_group), RequestSecretPacket, UpdateStatePacket(20) / client_state(21).
        System.out.println("group=" + hex(new Buf().writeUUID(groupId).writeUtf("Friends").writeBoolean(true).writeBoolean(false).writeBoolean(false).writeShort(2).array()));
        System.out.println("request21=" + hex(new Buf().writeInt(21).array()));
        System.out.println("state21=" + hex(new Buf().writeBoolean(true).writeBoolean(false).array()));
        System.out.println("state20=" + hex(new Buf().writeBoolean(false).array()));
    }
}
