// hstudio_crypt against the standards' own test vectors: RFC 1321 (MD5),
// FIPS 180 (SHA-1, SHA-2), FIPS 202 (SHA-3), RFC 2202 and RFC 4231 (HMAC),
// FIPS 197 and NIST SP 800-38A (AES and its modes), McGrew and Viega's GCM
// test cases, RFC 4648 (base64) and RFC 8439 (ChaCha20, under the random
// bytes); LZ4 blocks written by the reference implementation, round trips,
// and broken input of every kind, which must fail cleanly.

use horizon_studio::game::hstudio_crypt::{
    Aes, Hash, Hasher, Mode, Rng, base64_decode, base64_encode, cbc_decrypt, cbc_encrypt, cfb_decrypt, cfb_encrypt,
    chacha20_block, ctr_decrypt, ctr_encrypt, decrypt, ecb_decrypt, ecb_encrypt, encrypt, from_hex, gcm_decrypt,
    gcm_encrypt, hex, hmac, hmac_hex, lz4_bound, lz4_compress, lz4_decompress, md5, ofb_decrypt, ofb_encrypt,
    random_bytes, sha1, sha3_224, sha3_256, sha3_384, sha3_512, sha224, sha256, sha384, sha512,
};

/// Hex as bytes; whitespace (line breaks in long vectors) is skipped.
fn h(s: &str) -> Vec<u8> {
    from_hex(s).unwrap()
}

/// FIPS 180's two-block messages, and its million a's.
const M448: &[u8] = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
const M896: &[u8] =
    b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu";

fn million_a() -> Vec<u8> {
    vec![b'a'; 1_000_000]
}

// ---------------------------------------------------------------- hashes

#[test]
fn md5_matches_rfc_1321() {
    for (msg, want) in [
        (&b""[..], "d41d8cd98f00b204e9800998ecf8427e"),
        (b"a", "0cc175b9c0f1b6a831c399e269772661"),
        (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
        (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (b"abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"),
        (
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        (
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            "57edf4a22be3c955ac49da2e2107b67a",
        ),
    ] {
        assert_eq!(hex(&md5(msg)), want, "{:?}", String::from_utf8_lossy(msg));
        assert_eq!(Hash::Md5.hex(msg), want);
    }
}

#[test]
fn sha1_and_sha2_match_fips_180() {
    let mil = million_a();
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(hex(&sha1(M448)), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
    assert_eq!(hex(&sha1(&mil)), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");

    assert_eq!(
        hex(&sha224(b"abc")),
        "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7"
    );
    assert_eq!(
        hex(&sha224(M448)),
        "75388b16512776cc5dba5da1fd890150b0c6455cb4f58b1952522525"
    );
    assert_eq!(
        hex(&sha224(&mil)),
        "20794655980c91d8bbb4c1ea97618a4bf03f42581948b2ee4ee7ad67"
    );

    assert_eq!(
        hex(&sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(&sha256(M448)),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(
        hex(&sha256(&mil)),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );

    assert_eq!(
        hex(&sha384(b"abc")),
        "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"
    );
    assert_eq!(
        hex(&sha384(M896)),
        "09330c33f71147e83d192fc782cd1b4753111b173b3b05d22fa08086e3b0f712fcc7c71a557e2db966c3e9fa91746039"
    );
    assert_eq!(
        hex(&sha384(&mil)),
        "9d0e1809716474cb086e834e310a4a1ced149e9c00f248527972cec5704c2a5b07b8b3dc38ecc4ebae97ddd87f3d8985"
    );

    assert_eq!(
        hex(&sha512(b"abc")),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
         2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
    );
    assert_eq!(
        hex(&sha512(M896)),
        "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018\
         501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909"
    );
    assert_eq!(
        hex(&sha512(&mil)),
        "e718483d0ce769644e2e42c7bc15b4638e1f98b13b2044285632a803afa973eb\
         de0ff244877ea60a4cb0432ce577c31beb009c5c2c49aa2e4eadb217ad8cc09b"
    );
}

#[test]
fn sha3_matches_fips_202() {
    // NIST's examples: the empty message, "abc", 1600 bits of 0xa3, and
    // the two-block and million-a messages (as NIST's CAVS lists them).
    let a3 = vec![0xa3; 200];
    let mil = million_a();
    let cases: [(&[u8], [&str; 4]); 5] = [
        (
            b"",
            [
                "6b4e03423667dbb73b6e15454f0eb1abd4597f9a1b078e3f5b5a6bc7",
                "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a",
                "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004",
                "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a6\
                 15b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
            ],
        ),
        (
            b"abc",
            [
                "e642824c3f8cf24ad09234ee7d3c766fc9a3a5168d0c94ad73b46fdf",
                "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
                "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25",
                "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e\
                 10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
            ],
        ),
        (
            &a3,
            [
                "9376816aba503f72f96ce7eb65ac095deee3be4bf9bbc2a1cb7e11e0",
                "79f38adec5c20307a98ef76e8324afbfd46cfd81b22e3973c65fa1bd9de31787",
                "1881de2ca7e41ef95dc4732b8f5f002b189cc1e42b74168ed1732649ce1dbcdd76197a31fd55ee989f2d7050dd473e8f",
                "e76dfad22084a8b1467fcf2ffa58361bec7628edf5f3fdc0e4805dc48caeeca8\
                 1b7c13c30adf52a3659584739a2df46be589c51ca1a4a8416df6545a1ce8ba00",
            ],
        ),
        (
            M448,
            [
                "8a24108b154ada21c9fd5574494479ba5c7e7ab76ef264ead0fcce33",
                "41c0dba2a9d6240849100376a8235e2c82e1b9998a999e21db32dd97496d3376",
                "991c665755eb3a4b6bbdfb75c78a492e8c56a22c5c4d7e429bfdbc32b9d4ad5aa04a1f076e62fea19eef51acd0657c22",
                "04a371e84ecfb5b8b77cb48610fca8182dd457ce6f326a0fd3d7ec2f1e91636d\
                 ee691fbe0c985302ba1b0d8dc78c086346b533b49c030d99a27daf1139d6e75e",
            ],
        ),
        (
            &mil,
            [
                "d69335b93325192e516a912e6d19a15cb51c6ed5c15243e7a7fd653c",
                "5c8875ae474a3634ba4fd55ec85bffd661f32aca75c6d699d0cdcb6c115891c1",
                "eee9e24d78c1855337983451df97c8ad9eedf256c6334f8e948d252d5e0e76847aa0774ddb90a842190d2c558b4b8340",
                "3c3a876da14034ab60627c077bb98f7e120a2a5370212dffb3385a18d4f38859\
                 ed311d0a9d5141ce9cc5c66ee689b266a8aa18ace8282a0e0db596c90b0a7b87",
            ],
        ),
    ];
    for (msg, [d224, d256, d384, d512]) in cases {
        assert_eq!(hex(&sha3_224(msg)), d224, "{} bytes", msg.len());
        assert_eq!(hex(&sha3_256(msg)), d256, "{} bytes", msg.len());
        assert_eq!(hex(&sha3_384(msg)), d384, "{} bytes", msg.len());
        assert_eq!(hex(&sha3_512(msg)), d512, "{} bytes", msg.len());
    }
}

#[test]
fn hashes_go_by_name_and_take_data_in_parts() {
    for (name, want) in [
        ("md5", Hash::Md5),
        ("SHA1", Hash::Sha1),
        ("sha-1", Hash::Sha1),
        ("sha224", Hash::Sha224),
        ("sha256", Hash::Sha256),
        ("SHA-384", Hash::Sha384),
        ("sha512", Hash::Sha512),
        ("sha3-224", Hash::Sha3_224),
        ("sha3_256", Hash::Sha3_256),
        ("SHA3-384", Hash::Sha3_384),
        (" sha3-512 ", Hash::Sha3_512),
    ] {
        assert_eq!(Hash::from_name(name), Some(want), "{name}");
    }
    for bad in ["", "sha", "sha3", "md4", "sha3-1", "blake2b"] {
        assert_eq!(Hash::from_name(bad), None, "{bad}");
    }
    // Fed in two parts split anywhere (across the block buffers' edges
    // too), every hash gives the one-shot digest, of its own size.
    let data: Vec<u8> = (0..300u32).map(|i| (i * 7 + 3) as u8).collect();
    for hash in Hash::ALL {
        assert_eq!(Hash::from_name(hash.name()), Some(hash));
        let whole = hash.digest(&data);
        assert_eq!(whole.len(), hash.size(), "{}", hash.name());
        for split in 0..=data.len() {
            let mut h = Hasher::new(hash);
            h.update(&data[..split]);
            h.update(&data[split..]);
            assert_eq!(h.finish(), whole, "{} split at {split}", hash.name());
        }
        let mut h = Hasher::new(hash);
        for b in &data {
            h.update(std::slice::from_ref(b));
        }
        assert_eq!(h.finish(), whole, "{} byte by byte", hash.name());
    }
}

// ---------------------------------------------------------------- HMAC

#[test]
fn hmac_sha256_matches_rfc_4231() {
    let big_key = vec![0xaa; 131];
    for (key, data, want) in [
        (
            vec![0x0b; 20],
            b"Hi There".to_vec(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
        ),
        (
            b"Jefe".to_vec(),
            b"what do ya want for nothing?".to_vec(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        ),
        (
            vec![0xaa; 20],
            vec![0xdd; 50],
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe",
        ),
        (
            (1..=25).collect(),
            vec![0xcd; 50],
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b",
        ),
        // Test case 5 gives the first 128 bits.
        (
            vec![0x0c; 20],
            b"Test With Truncation".to_vec(),
            "a3b6167473100ee06e0c796c2955552b",
        ),
        (
            big_key.clone(),
            b"Test Using Larger Than Block-Size Key - Hash Key First".to_vec(),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
        ),
        (
            big_key,
            b"This is a test using a larger than block-size key and a larger than block-size data. The key needs to \
              be hashed before being used by the HMAC algorithm."
                .to_vec(),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2",
        ),
    ] {
        let mac = hmac_hex(Hash::Sha256, &key, &data);
        assert_eq!(&mac[..want.len()], want);
    }
}

#[test]
fn hmac_works_with_every_hash() {
    let jefe = (&b"Jefe"[..], &b"what do ya want for nothing?"[..]);
    // RFC 2202 (MD5, SHA-1) and RFC 4231 (SHA-224, SHA-384, SHA-512).
    assert_eq!(
        hmac_hex(Hash::Md5, &[0x0b; 16], b"Hi There"),
        "9294727a3638bb1c13f48ef8158bfc9d"
    );
    assert_eq!(hmac_hex(Hash::Md5, jefe.0, jefe.1), "750c783e6ab0b503eaa86e310a5db738");
    assert_eq!(
        hmac_hex(Hash::Sha1, &[0x0b; 20], b"Hi There"),
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    assert_eq!(
        hmac_hex(Hash::Sha1, jefe.0, jefe.1),
        "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
    );
    assert_eq!(
        hmac_hex(Hash::Sha224, jefe.0, jefe.1),
        "a30e01098bc6dbbf45690f3a7e9e6d0f8bbea2a39e6148008fd05e44"
    );
    assert_eq!(
        hmac_hex(Hash::Sha384, jefe.0, jefe.1),
        "af45d2e376484031617f78d2b58a6b1b9c7ef464f5a01b47e42ec3736322445e8e2240ca5e69e2c78b3239ecfab21649"
    );
    assert_eq!(
        hmac_hex(Hash::Sha512, &[0x0b; 20], b"Hi There"),
        "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cde\
         daa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854"
    );
    assert_eq!(
        hmac_hex(Hash::Sha512, jefe.0, jefe.1),
        "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea250554\
         9758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"
    );
    // HMAC-SHA3 pads keys to the sponge's rate (as NIST SP 800-224 does);
    // these are OpenSSL's, through Python's hmac. The 200-byte key is
    // longer than every rate, so it is hashed first.
    let long = (
        &[0xaa; 200][..],
        &b"Test Using Larger Than Block-Size Key - Hash Key First"[..],
    );
    for (hash, short, hashed) in [
        (
            Hash::Sha3_224,
            "7fdb8dd88bd2f60d1b798634ad386811c2cfc85bfaf5d52bbace5e66",
            "5e73d57bd011f0f92fef3c3b92ea4bcb4821c6d83c37db34f29e0760",
        ),
        (
            Hash::Sha3_256,
            "c7d4072e788877ae3596bbb0da73b887c9171f93095b294ae857fbe2645e1ba5",
            "49ad92b02124fdac9627ae45e008a696182ab6bfb8470457777c744aeb9df06f",
        ),
        (
            Hash::Sha3_384,
            "f1101f8cbf9766fd6764d2ed61903f21ca9b18f57cf3e1a23ca13508a93243ce48c045dc007f26a21b3f5e0e9df4c20a",
            "3e7b62d091d75f484892bc2ed26d7b0ed37c9529f0227197cc8522971eb6f7215dd4e0cc6ea306987e0cbfe914f3a916",
        ),
        (
            Hash::Sha3_512,
            "5a4bfeab6166427c7a3647b747292b8384537cdb89afb3bf5665e4c5e709350b\
             287baec921fd7ca0ee7a0c31d022a95e1fc92ba9d77df883960275beb4e62024",
            "fafc7b7fe3332ce153966b27f6586fa5b49ec5d8dff3d7fd26a011451ca4c9de\
             437913879159d9c5181a9a6f377ef18b48399756decea695b04fe90a9d3b93d1",
        ),
    ] {
        assert_eq!(hmac_hex(hash, jefe.0, jefe.1), short, "{}", hash.name());
        assert_eq!(hmac_hex(hash, long.0, long.1), hashed, "{}", hash.name());
    }
    for hash in Hash::ALL {
        assert_eq!(hmac(hash, b"k", b"m").len(), hash.size());
        assert_ne!(hmac(hash, b"k", b"m"), hmac(hash, b"K", b"m"));
    }
}

// ---------------------------------------------------------------- base64 and hex

#[test]
fn base64_matches_rfc_4648() {
    for (plain, coded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64_encode(plain.as_bytes()), coded);
        assert_eq!(base64_decode(coded).unwrap(), plain.as_bytes());
        // Without its padding, too.
        assert_eq!(base64_decode(coded.trim_end_matches('=')).unwrap(), plain.as_bytes());
    }
    let all: Vec<u8> = (0..=255).collect();
    let coded = base64_encode(&all);
    assert!(coded.starts_with("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4v"));
    assert!(coded.ends_with("+/w=="));
    assert_eq!(base64_decode(&coded).unwrap(), all);
    let mut rng = Rng::from_seed([1; 32]);
    for len in 0..100 {
        let data = rng.bytes(len);
        assert_eq!(base64_decode(base64_encode(&data)).unwrap(), data);
    }
}

#[test]
fn base64_skips_whitespace_and_rejects_the_rest() {
    assert_eq!(base64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
    assert_eq!(base64_decode(" Zm 9v\tYm E= \r\n").unwrap(), b"fooba");
    assert_eq!(base64_decode("Zg\n=\n=\n").unwrap(), b"f");
    for (bad, says) in [
        ("Zm9v!", "'!'"),
        ("Zm9v-", "'-'"),
        ("Zm9v_", "'_'"),
        ("Zm9v\0", "byte 0x00"),
        ("Zm9vYé", "byte 0xc3"),
        ("Z", "one character off"),
        ("Zm9vY", "one character off"),
        ("Zg===", "padding"),
        ("Zm9v=", "padding"),
        ("Zm8==", "padding"),
        ("Zg=a", "after its '=' padding"),
        ("Zm9=v", "after its '=' padding"),
    ] {
        let err = base64_decode(bad).unwrap_err();
        assert!(err.contains(says), "{bad:?}: {err}");
    }
}

#[test]
fn hex_round_trips() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    assert_eq!(from_hex("000FA5ff").unwrap(), [0x00, 0x0f, 0xa5, 0xff]);
    assert_eq!(from_hex("00 0f\na5 ff").unwrap(), [0x00, 0x0f, 0xa5, 0xff]);
    assert_eq!(from_hex("").unwrap(), b"");
    assert!(from_hex("abc").unwrap_err().contains("odd"));
    assert!(from_hex("zz").unwrap_err().contains("'z'"));
}

// ---------------------------------------------------------------- AES

#[test]
fn aes_matches_fips_197() {
    let plain: [u8; 16] = h("00112233445566778899aabbccddeeff").try_into().unwrap();
    for (key, want, rounds) in [
        (
            "000102030405060708090a0b0c0d0e0f",
            "69c4e0d86a7b0430d8cdb78070b4c55a",
            10,
        ),
        (
            "000102030405060708090a0b0c0d0e0f1011121314151617",
            "dda97ca4864cdfe06eaf70a0ec0d7191",
            12,
        ),
        (
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
            "8ea2b7ca516745bfeafc49904b496089",
            14,
        ),
    ] {
        let aes = Aes::new(&h(key)).unwrap();
        assert_eq!(aes.rounds(), rounds);
        let mut block = plain;
        aes.encrypt_block(&mut block);
        assert_eq!(hex(&block), want, "key {key}");
        aes.decrypt_block(&mut block);
        assert_eq!(block, plain);
    }
    // Appendix B's worked example.
    let aes = Aes::new(&h("2b7e151628aed2a6abf7158809cf4f3c")).unwrap();
    let mut block: [u8; 16] = h("3243f6a8885a308d313198a2e0370734").try_into().unwrap();
    aes.encrypt_block(&mut block);
    assert_eq!(hex(&block), "3925841d02dc09fbdc118597196a0b32");

    for len in [0, 8, 15, 17, 23, 25, 31, 33, 64] {
        let err = Aes::new(&vec![0; len]).err().unwrap();
        assert!(err.contains("16, 24 or 32 bytes"), "{len}: {err}");
    }
}

/// NIST SP 800-38A, appendix F: the four-block plaintext, the three keys,
/// and each mode's ciphertext under each.
const SP_PLAIN: &str = "6bc1bee22e409f96e93d7e117393172a ae2d8a571e03ac9c9eb76fac45af8e51
                        30c81c46a35ce411e5fbc1191a0a52ef f69f2445df4f9b17ad2b417be66c3710";
const SP_KEYS: [&str; 3] = [
    "2b7e151628aed2a6abf7158809cf4f3c",
    "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b",
    "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4",
];
const SP_IV: &str = "000102030405060708090a0b0c0d0e0f";
const SP_COUNTER: &str = "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
const SP_CIPHER: [(Mode, [&str; 3]); 5] = [
    (
        Mode::Ecb,
        [
            "3ad77bb40d7a3660a89ecaf32466ef97 f5d3d58503b9699de785895a96fdbaaf
             43b1cd7f598ece23881b00e3ed030688 7b0c785e27e8ad3f8223207104725dd4",
            "bd334f1d6e45f25ff712a214571fa5cc 974104846d0ad3ad7734ecb3ecee4eef
             ef7afd2270e2e60adce0ba2face6444e 9a4b41ba738d6c72fb16691603c18e0e",
            "f3eed1bdb5d2a03c064b5a7e3db181f8 591ccb10d410ed26dc5ba74a31362870
             b6ed21b99ca6f4f9f153e7b1beafed1d 23304b7a39f9f3ff067d8d8f9e24ecc7",
        ],
    ),
    (
        Mode::Cbc,
        [
            "7649abac8119b246cee98e9b12e9197d 5086cb9b507219ee95db113a917678b2
             73bed6b8e3c1743b7116e69e22229516 3ff1caa1681fac09120eca307586e1a7",
            "4f021db243bc633d7178183a9fa071e8 b4d9ada9ad7dedf4e5e738763f69145a
             571b242012fb7ae07fa9baac3df102e0 08b0e27988598881d920a9e64f5615cd",
            "f58c4c04d6e5f1ba779eabfb5f7bfbd6 9cfc4e967edb808d679f777bc6702c7d
             39f23369a9d9bacfa530e26304231461 b2eb05e2c39be9fcda6c19078c6a9d1b",
        ],
    ),
    (
        Mode::Cfb,
        [
            "3b3fd92eb72dad20333449f8e83cfb4a c8a64537a0b3a93fcde3cdad9f1ce58b
             26751f67a3cbb140b1808cf187a4f4df c04b05357c5d1c0eeac4c66f9ff7f2e6",
            "cdc80d6fddf18cab34c25909c99a4174 67ce7f7f81173621961a2b70171d3d7a
             2e1e8a1dd59b88b1c8e60fed1efac4c9 c05f9f9ca9834fa042ae8fba584b09ff",
            "dc7e84bfda79164b7ecd8486985d3860 39ffed143b28b1c832113c6331e5407b
             df10132415e54b92a13ed0a8267ae2f9 75a385741ab9cef82031623d55b1e471",
        ],
    ),
    (
        Mode::Ofb,
        [
            "3b3fd92eb72dad20333449f8e83cfb4a 7789508d16918f03f53c52dac54ed825
             9740051e9c5fecf64344f7a82260edcc 304c6528f659c77866a510d9c1d6ae5e",
            "cdc80d6fddf18cab34c25909c99a4174 fcc28b8d4c63837c09e81700c1100401
             8d9a9aeac0f6596f559c6d4daf59a5f2 6d9f200857ca6c3e9cac524bd9acc92a",
            "dc7e84bfda79164b7ecd8486985d3860 4febdc6740d20b3ac88f6ad82a4fb08d
             71ab47a086e86eedf39d1c5bba97c408 0126141d67f37be8538f5a8be740e484",
        ],
    ),
    (
        Mode::Ctr,
        [
            "874d6191b620e3261bef6864990db6ce 9806f66b7970fdff8617187bb9fffdff
             5ae4df3edbd5d35e5b4f09020db03eab 1e031dda2fbe03d1792170a0f3009cee",
            "1abc932417521ca24f2b0459fe7e6e0b 090339ec0aa6faefd5ccc2c6f4ce8e94
             1e36b26bd1ebc670d1bd1d665620abf7 4f78a7f6d29809585a97daec58c6b050",
            "601ec313775789a5b7a7f504bbf3d228 f443e3ca4d62b59aca84e990cacaf5c5
             2b0930daa23de94ce87017ba2d84988d dfc9c58db67aada613c2dd08457941a6",
        ],
    ),
];

#[test]
fn aes_modes_match_sp_800_38a() {
    let plain = h(SP_PLAIN);
    for (mode, ciphers) in SP_CIPHER {
        let iv = h(if mode == Mode::Ctr { SP_COUNTER } else { SP_IV });
        for (key, cipher) in SP_KEYS.iter().zip(ciphers) {
            let (key, cipher) = (h(key), h(cipher));
            let what = format!("{} with a {}-bit key", mode.name(), key.len() * 8);
            let out = encrypt(mode, &key, &iv, &plain).unwrap();
            match mode {
                // PKCS#7 adds a whole block of 16s to whole blocks; NIST's
                // vectors are the blocks before it.
                Mode::Ecb | Mode::Cbc => {
                    assert_eq!(out.len(), 80, "{what}");
                    assert_eq!(out[..64], cipher[..], "{what}");
                    let mut theirs = cipher.clone();
                    theirs.extend_from_slice(&out[64..]);
                    assert_eq!(decrypt(mode, &key, &iv, &theirs).unwrap(), plain, "{what}");
                }
                _ => {
                    assert_eq!(out, cipher, "{what}");
                    assert_eq!(decrypt(mode, &key, &iv, &cipher).unwrap(), plain, "{what}");
                    // A stream mode's output is as long as its input, so any
                    // prefix of the plaintext encrypts to that prefix of
                    // NIST's ciphertext, partial blocks too.
                    for len in 0..plain.len() {
                        let part = encrypt(mode, &key, &iv, &plain[..len]).unwrap();
                        assert_eq!(part, cipher[..len], "{what}, {len} bytes");
                        assert_eq!(
                            decrypt(mode, &key, &iv, &part).unwrap(),
                            plain[..len],
                            "{what}, {len} bytes"
                        );
                    }
                }
            }
        }
    }
    // The per-mode functions are what `encrypt` and `decrypt` call.
    let (key, iv) = (h(SP_KEYS[0]), h(SP_IV));
    assert_eq!(
        ecb_encrypt(&key, &plain).unwrap(),
        encrypt(Mode::Ecb, &key, &iv, &plain).unwrap()
    );
    assert_eq!(
        cbc_encrypt(&key, &iv, &plain).unwrap()[..16],
        h(SP_CIPHER[1].1[0])[..16]
    );
    assert_eq!(cfb_encrypt(&key, &iv, &plain).unwrap(), h(SP_CIPHER[2].1[0]));
    assert_eq!(ofb_encrypt(&key, &iv, &plain).unwrap(), h(SP_CIPHER[3].1[0]));
    assert_eq!(ctr_encrypt(&key, &h(SP_COUNTER), &plain).unwrap(), h(SP_CIPHER[4].1[0]));
    let cbc = cbc_encrypt(&key, &iv, &plain).unwrap();
    assert_eq!(cbc_decrypt(&key, &iv, &cbc).unwrap(), plain);
    assert_eq!(ecb_decrypt(&key, &ecb_encrypt(&key, &plain).unwrap()).unwrap(), plain);
    assert_eq!(cfb_decrypt(&key, &iv, &h(SP_CIPHER[2].1[0])).unwrap(), plain);
    assert_eq!(ofb_decrypt(&key, &iv, &h(SP_CIPHER[3].1[0])).unwrap(), plain);
    assert_eq!(ctr_decrypt(&key, &h(SP_COUNTER), &h(SP_CIPHER[4].1[0])).unwrap(), plain);
}

#[test]
fn ctr_counts_as_one_128_bit_number() {
    let aes_key = h(SP_KEYS[0]);
    let aes = Aes::new(&aes_key).unwrap();
    let block = |hex_counter: &str| {
        let mut b: [u8; 16] = h(hex_counter).try_into().unwrap();
        aes.encrypt_block(&mut b);
        b
    };
    for (start, next) in [
        ("ffffffffffffffffffffffffffffffff", "00000000000000000000000000000000"),
        ("0000000000000000ffffffffffffffff", "00000000000000010000000000000000"),
        ("000000000000000000000000ffffffff", "00000000000000000000000100000000"),
    ] {
        let keystream = ctr_encrypt(&aes_key, &h(start), &[0; 32]).unwrap();
        assert_eq!(keystream[..16], block(start), "{start}");
        assert_eq!(keystream[16..], block(next), "{start}");
    }
}

#[test]
fn every_mode_round_trips_any_length() {
    let mut rng = Rng::from_seed([2; 32]);
    for mode in Mode::ALL {
        assert_eq!(Mode::from_name(mode.name()), Some(mode));
        assert_eq!(Mode::from_name(&mode.name().to_lowercase()), Some(mode));
        for key_len in [16, 24, 32] {
            let key = rng.bytes(key_len);
            let iv = rng.bytes(mode.iv_size());
            for len in [0, 1, 15, 16, 17, 31, 32, 33, 100, 1000] {
                let data = rng.bytes(len);
                let out = encrypt(mode, &key, &iv, &data).unwrap();
                let want_len = match mode {
                    Mode::Ecb | Mode::Cbc => (len / 16 + 1) * 16,
                    Mode::Gcm => len + 16,
                    _ => len,
                };
                assert_eq!(out.len(), want_len, "{} {key_len} {len}", mode.name());
                if len >= 16 {
                    assert_ne!(out[..16], data[..16]);
                }
                assert_eq!(
                    decrypt(mode, &key, &iv, &out).unwrap(),
                    data,
                    "{} {key_len} {len}",
                    mode.name()
                );
            }
        }
    }
    assert_eq!(Mode::from_name(" ctr "), Some(Mode::Ctr));
    assert_eq!(Mode::from_name("xts"), None);
    assert_eq!(Mode::from_name(""), None);
    assert_eq!(
        Mode::ALL.map(Mode::iv_size),
        [0, 16, 16, 16, 16, 12],
        "ECB, CBC, CFB, OFB, CTR, GCM"
    );
    // ECB takes no IV, so one given is ignored.
    let key = [9; 16];
    assert_eq!(
        encrypt(Mode::Ecb, &key, b"ignored", b"same").unwrap(),
        encrypt(Mode::Ecb, &key, &[], b"same").unwrap()
    );
}

#[test]
fn aes_mistakes_are_errors() {
    let key = [7u8; 16];
    let iv = [3u8; 16];
    for mode in [Mode::Cbc, Mode::Cfb, Mode::Ofb, Mode::Ctr] {
        for len in [0, 12, 15, 17, 32] {
            let err = encrypt(mode, &key, &vec![0; len], b"data").unwrap_err();
            assert!(err.contains("16-byte IV"), "{} {len}: {err}", mode.name());
            assert!(decrypt(mode, &key, &vec![0; len], &[0; 16]).is_err());
        }
    }
    for mode in Mode::ALL {
        let err = encrypt(mode, &[0; 10], &iv, b"data").unwrap_err();
        assert!(err.contains("AES key"), "{}: {err}", mode.name());
    }
    // ECB and CBC take whole blocks, and their padding must be right.
    for mode in [Mode::Ecb, Mode::Cbc] {
        for len in [0, 1, 15, 17, 33] {
            let err = decrypt(mode, &key, &iv, &vec![0; len]).unwrap_err();
            assert!(err.contains("whole 16-byte blocks"), "{} {len}: {err}", mode.name());
        }
    }
    let aes = Aes::new(&key).unwrap();
    for last in [
        [0u8; 16],
        [17; 16],
        [255; 16],
        *b"aaaaaaaaaaaaaa\x02\x03",
        *b"aaaaaaaaaaa\x05\x05\x05\x04\x05",
    ] {
        let mut block = last;
        aes.encrypt_block(&mut block);
        let err = ecb_decrypt(&key, &block).unwrap_err();
        assert!(err.contains("padding"), "{last:?}: {err}");
    }
    let mut full = [16u8; 16];
    aes.encrypt_block(&mut full);
    assert_eq!(
        ecb_decrypt(&key, &full).unwrap(),
        b"",
        "a whole block of padding is nothing"
    );
    // With the wrong key the padding comes out wrong.
    let sealed = cbc_encrypt(&key, &iv, b"a secret message").unwrap();
    assert!(cbc_decrypt(&[8; 16], &iv, &sealed).unwrap_err().contains("padding"));
}

// ---------------------------------------------------------------- GCM

struct GcmCase {
    name: &'static str,
    key: &'static str,
    iv: &'static str,
    plain: &'static str,
    aad: &'static str,
    cipher: &'static str,
    tag: &'static str,
}

const GCM_KEY: &str = "feffe9928665731c6d6a8f9467308308";
const GCM_KEY_256: &str = "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308";
const GCM_PLAIN: &str = "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72
                         1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255";
const GCM_PLAIN_60: &str = "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72
                            1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39";
const GCM_AAD: &str = "feedfacedeadbeeffeedfacedeadbeefabaddad2";

/// McGrew and Viega, "The Galois/Counter Mode of Operation (GCM)": test
/// cases 1 to 6 (AES-128; 5 and 6 with 8- and 60-byte IVs) and 13 to 16
/// (AES-256).
const GCM_CASES: [GcmCase; 10] = [
    GcmCase {
        name: "1",
        key: "00000000000000000000000000000000",
        iv: "000000000000000000000000",
        plain: "",
        aad: "",
        cipher: "",
        tag: "58e2fccefa7e3061367f1d57a4e7455a",
    },
    GcmCase {
        name: "2",
        key: "00000000000000000000000000000000",
        iv: "000000000000000000000000",
        plain: "00000000000000000000000000000000",
        aad: "",
        cipher: "0388dace60b6a392f328c2b971b2fe78",
        tag: "ab6e47d42cec13bdf53a67b21257bddf",
    },
    GcmCase {
        name: "3",
        key: GCM_KEY,
        iv: "cafebabefacedbaddecaf888",
        plain: GCM_PLAIN,
        aad: "",
        cipher: "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e
                 21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985",
        tag: "4d5c2af327cd64a62cf35abd2ba6fab4",
    },
    GcmCase {
        name: "4",
        key: GCM_KEY,
        iv: "cafebabefacedbaddecaf888",
        plain: GCM_PLAIN_60,
        aad: GCM_AAD,
        cipher: "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e
                 21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091",
        tag: "5bc94fbc3221a5db94fae95ae7121a47",
    },
    GcmCase {
        name: "5",
        key: GCM_KEY,
        iv: "cafebabefacedbad",
        plain: GCM_PLAIN_60,
        aad: GCM_AAD,
        cipher: "61353b4c2806934a777ff51fa22a4755699b2a714fcdc6f83766e5f97b6c7423
                 73806900e49f24b22b097544d4896b424989b5e1ebac0f07c23f4598",
        tag: "3612d2e79e3b0785561be14aaca2fccb",
    },
    GcmCase {
        name: "6",
        key: GCM_KEY,
        iv: "9313225df88406e555909c5aff5269aa6a7a9538534f7da1e4c303d2a318a728
             c3c0c95156809539fcf0e2429a6b525416aedbf5a0de6a57a637b39b",
        plain: GCM_PLAIN_60,
        aad: GCM_AAD,
        cipher: "8ce24998625615b603a033aca13fb894be9112a5c3a211a8ba262a3cca7e2ca7
                 01e4a9a4fba43c90ccdcb281d48c7c6fd62875d2aca417034c34aee5",
        tag: "619cc5aefffe0bfa462af43c1699d050",
    },
    GcmCase {
        name: "13",
        key: "0000000000000000000000000000000000000000000000000000000000000000",
        iv: "000000000000000000000000",
        plain: "",
        aad: "",
        cipher: "",
        tag: "530f8afbc74536b9a963b4f1c4cb738b",
    },
    GcmCase {
        name: "14",
        key: "0000000000000000000000000000000000000000000000000000000000000000",
        iv: "000000000000000000000000",
        plain: "00000000000000000000000000000000",
        aad: "",
        cipher: "cea7403d4d606b6e074ec5d3baf39d18",
        tag: "d0d1c8a799996bf0265b98b5d48ab919",
    },
    GcmCase {
        name: "15",
        key: GCM_KEY_256,
        iv: "cafebabefacedbaddecaf888",
        plain: GCM_PLAIN,
        aad: "",
        cipher: "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa
                 8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad",
        tag: "b094dac5d93471bdec1a502270e3cc6c",
    },
    GcmCase {
        name: "16",
        key: GCM_KEY_256,
        iv: "cafebabefacedbaddecaf888",
        plain: GCM_PLAIN_60,
        aad: GCM_AAD,
        cipher: "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa
                 8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662",
        tag: "76fc6ece0f4e1768cddf8853bb2d551b",
    },
];

#[test]
fn gcm_matches_mcgrew_and_viega() {
    for c in GCM_CASES {
        let (key, iv, plain, aad) = (h(c.key), h(c.iv), h(c.plain), h(c.aad));
        let (cipher, tag) = gcm_encrypt(&key, &iv, &aad, &plain).unwrap();
        assert_eq!(hex(&cipher), hex(&h(c.cipher)), "test case {}", c.name);
        assert_eq!(hex(&tag), c.tag, "test case {}", c.name);
        assert_eq!(
            gcm_decrypt(&key, &iv, &aad, &cipher, &tag).unwrap(),
            plain,
            "test case {}",
            c.name
        );
        if aad.is_empty() {
            // `encrypt` puts the tag after the ciphertext.
            let sealed = encrypt(Mode::Gcm, &key, &iv, &plain).unwrap();
            assert_eq!(sealed, [cipher, tag.to_vec()].concat(), "test case {}", c.name);
            assert_eq!(decrypt(Mode::Gcm, &key, &iv, &sealed).unwrap(), plain);
        }
    }
}

#[test]
fn gcm_rejects_any_change() {
    let c = &GCM_CASES[3];
    let (key, iv, plain, aad) = (h(c.key), h(c.iv), h(c.plain), h(c.aad));
    let (cipher, tag) = (h(c.cipher), h(c.tag));
    assert_eq!(gcm_decrypt(&key, &iv, &aad, &cipher, &tag).unwrap(), plain);
    let rejected =
        |key: &[u8], iv: &[u8], aad: &[u8], cipher: &[u8], tag: &[u8]| gcm_decrypt(key, iv, aad, cipher, tag).is_err();
    // Every bit of the tag, the ciphertext and the additional data counts.
    for bit in 0..tag.len() * 8 {
        let mut t = tag.clone();
        t[bit / 8] ^= 1 << (bit % 8);
        assert!(rejected(&key, &iv, &aad, &cipher, &t), "tag bit {bit}");
        let err = gcm_decrypt(&key, &iv, &aad, &cipher, &t).unwrap_err();
        assert!(err.contains("tag doesn't match"), "{err}");
    }
    for bit in 0..cipher.len() * 8 {
        let mut x = cipher.clone();
        x[bit / 8] ^= 1 << (bit % 8);
        assert!(rejected(&key, &iv, &aad, &x, &tag), "ciphertext bit {bit}");
    }
    for bit in 0..aad.len() * 8 {
        let mut a = aad.clone();
        a[bit / 8] ^= 1 << (bit % 8);
        assert!(rejected(&key, &iv, &a, &cipher, &tag), "aad bit {bit}");
    }
    // Shorter or longer data, no or extra additional data, another nonce or key.
    assert!(rejected(&key, &iv, &aad, &cipher[..59], &tag));
    assert!(rejected(&key, &iv, &aad, &[&cipher[..], &[0]].concat(), &tag));
    assert!(rejected(&key, &iv, &[], &cipher, &tag));
    assert!(rejected(&key, &iv, &[&aad[..], &[0]].concat(), &cipher, &tag));
    assert!(rejected(&key, &iv, &aad[..19], &cipher, &tag));
    assert!(rejected(&key, &h("cafebabefacedbaddecaf889"), &aad, &cipher, &tag));
    assert!(rejected(&key, &iv[..8], &aad, &cipher, &tag));
    assert!(rejected(&h(GCM_KEY_256), &iv, &aad, &cipher, &tag));
    assert!(rejected(&[0; 16], &iv, &aad, &cipher, &tag));
    // A tag is 16 bytes: shortened ones (which would be easier to guess) are refused.
    for len in [0, 4, 8, 12, 15] {
        let err = gcm_decrypt(&key, &iv, &aad, &cipher, &tag[..len]).unwrap_err();
        assert!(err.contains("16 bytes"), "{len}: {err}");
    }
    assert!(rejected(&key, &iv, &aad, &cipher, &[&tag[..], &[0]].concat()));
    // Random tags on random messages never pass.
    let mut rng = Rng::from_seed([3; 32]);
    for len in 0..200 {
        let data = rng.bytes(len);
        let (sealed, good) = gcm_encrypt(&key, &iv, b"", &data).unwrap();
        let guess = rng.bytes(16);
        assert_ne!(guess, good);
        assert!(rejected(&key, &iv, b"", &sealed, &guess));
        // `decrypt`, which takes the tag at the end, likewise.
        let mut joined = sealed.clone();
        joined.extend_from_slice(&guess);
        assert!(decrypt(Mode::Gcm, &key, &iv, &joined).is_err());
    }
    for len in 0..16 {
        let err = decrypt(Mode::Gcm, &key, &iv, &vec![0; len]).unwrap_err();
        assert!(err.contains("16-byte tag"), "{len}: {err}");
    }
    assert!(gcm_encrypt(&key, &[], b"", b"x").unwrap_err().contains("nonce"));
    assert!(rejected(&key, &[], &aad, &cipher, &tag));
}

// ---------------------------------------------------------------- random bytes

#[test]
fn chacha20_matches_rfc_8439() {
    // Section 2.3.2's block, and appendix A.1's first two blocks (which
    // are what an all-zero seed gives `Rng`).
    let key: [u8; 32] = std::array::from_fn(|i| i as u8);
    let nonce: [u8; 12] = h("000000090000004a00000000").try_into().unwrap();
    assert_eq!(
        hex(&chacha20_block(&key, 1, &nonce)),
        "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4e\
         d2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e"
    );
    let a1 = "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7\
              da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586";
    let a2 = "9f07e7be5551387a98ba977c732d080dcb0f29a048e3656912c6533e32ee7aed\
              29b721769ce64e43d57133b074d839d531ed1f28510afb45ace10a1f4b794d6f";
    assert_eq!(hex(&chacha20_block(&[0; 32], 0, &[0; 12])), a1);
    assert_eq!(hex(&chacha20_block(&[0; 32], 1, &[0; 12])), a2);
    assert_eq!(hex(&Rng::from_seed([0; 32]).bytes(128)), format!("{a1}{a2}"));
}

#[test]
fn rng_repeats_from_a_seed_and_varies_without_one() {
    let mut a = Rng::from_seed([5; 32]);
    let mut b = Rng::from_seed([5; 32]);
    let whole = a.bytes(1000);
    // In pieces of any size, the same stream.
    let mut pieces = Vec::new();
    for n in [1, 2, 3, 61, 64, 65, 100, 0, 704] {
        let mut buf = vec![0; n];
        b.fill(&mut buf);
        pieces.extend(buf);
    }
    assert_eq!(pieces, whole);
    assert_ne!(Rng::from_seed([6; 32]).bytes(32), whole[..32]);
    let mut c = Rng::from_seed([5; 32]);
    assert_eq!(c.next_u32().to_le_bytes(), whole[..4]);
    assert_eq!(c.next_u64().to_le_bytes(), whole[4..12]);

    assert_eq!(random_bytes(0), b"");
    assert_eq!(random_bytes(77).len(), 77);
    assert_ne!(random_bytes(32), random_bytes(32));
    assert_ne!(Rng::new().bytes(32), Rng::new().bytes(32));
    let here = random_bytes(32);
    let there = std::thread::spawn(|| random_bytes(32)).join().unwrap();
    assert_ne!(here, there);
    // Every byte value about equally often: 1/256 of 256 KiB is 1024.
    let mut counts = [0u32; 256];
    for b in random_bytes(256 * 1024) {
        counts[usize::from(b)] += 1;
    }
    assert!(counts.iter().all(|&n| (800..1250).contains(&n)), "{counts:?}");
}

// ---------------------------------------------------------------- LZ4

/// Walks a block's sequences, checking the rules the reference decoder
/// relies on: every match starts at least 12 bytes before the end and the
/// last 5 bytes are literals. Returns how many matches there were.
fn check_end_rules(block: &[u8], size: usize) -> usize {
    let (mut i, mut pos, mut matches) = (0, 0, 0);
    let len = |i: &mut usize, nibble: u8| {
        let mut n = usize::from(nibble);
        if nibble == 15 {
            loop {
                let b = block[*i];
                *i += 1;
                n += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        n
    };
    loop {
        let token = block[i];
        i += 1;
        let lit = len(&mut i, token >> 4);
        i += lit;
        pos += lit;
        if i == block.len() {
            assert_eq!(pos, size);
            return matches;
        }
        i += 2;
        let m = len(&mut i, token & 15) + 4;
        assert!(pos + 12 <= size, "a match starts {pos} bytes into {size}");
        assert!(pos + m + 5 <= size, "a match ends {} bytes into {size}", pos + m);
        pos += m;
        matches += 1;
    }
}

/// Text from a small vocabulary: compressible as real text is.
fn words(rng: &mut Rng, len: usize) -> Vec<u8> {
    let words = [
        "horizon", "studio", "lua", "mod", "the", "a", "block", "chunk", "render", "zoom",
    ];
    let mut out = Vec::new();
    while out.len() < len {
        out.extend_from_slice(words[(rng.next_u32() % 10) as usize].as_bytes());
        out.push(if rng.next_u32().is_multiple_of(9) { b'\n' } else { b' ' });
    }
    out.truncate(len);
    out
}

#[test]
fn lz4_reads_the_reference_implementations_blocks() {
    // Written by LZ4_compress_default in liblz4 1.9.4 (the reference
    // implementation), and read back by its LZ4_decompress_safe.
    let text = b"Horizon Studio packs Horizon Studio's scripts with LZ4, and Horizon Studio unpacks them again.";
    let block = h(
        "fa06486f72697a6f6e2053747564696f207061636b73201500fb092773207363726970747320\
                   7769746820 4c5a342c20616e6427003220756e3e00b07468656d20616761696e2e",
    );
    assert_eq!(lz4_decompress(&block, text.len()).unwrap(), text);
    // The same writer's block for 1000 a's, the bytes 0 to 255, "0123456789"
    // three times and "!", taken apart as the format spec describes it.
    // 1 literal, then a match 1 back (each byte repeats the one before) of
    // 15 + 255 * 3 + 215 + 4 = 999 bytes:
    let mut block = vec![0x1f, b'a', 0x01, 0x00, 0xff, 0xff, 0xff, 0xd7];
    // 15 + 241 = 256 literals, then 6 + 4 = 10 bytes from 208 back (0x30 to 0x39):
    block.extend([0xf6, 0xf1]);
    block.extend(0..=255);
    block.extend([0xd0, 0x00]);
    // No literals, and the same 10 bytes again, from 218 back:
    block.extend([0x06, 0xda, 0x00]);
    // The last 11 bytes, as literals:
    block.push(0xb0);
    block.extend(b"0123456789!");
    let mut want = vec![b'a'; 1000];
    want.extend(0..=255);
    want.extend(b"0123456789".repeat(3));
    want.push(b'!');
    assert_eq!(lz4_decompress(&block, want.len()).unwrap(), want);
    // Empty input is the one token 0; "hello" is its 5 literals.
    assert_eq!(lz4_decompress(&[0x00], 0).unwrap(), b"");
    assert_eq!(lz4_decompress(&h("5068656c6c6f"), 5).unwrap(), b"hello");
    assert_eq!(lz4_compress(b""), [0x00]);
    assert_eq!(lz4_compress(b"hello"), h("5068656c6c6f"));
    // Short runs, which compress here byte for byte as the reference does:
    // 12 bytes are too few for a match; 13 are the fewest with one (1
    // literal, 7 bytes from 1 back, and the 5 literals a block ends with).
    for (n, block) in [
        (12, "c0616161616161616161616161"),
        (13, "13610100506161616161"),
        (20, "1a610100506161616161"),
    ] {
        assert_eq!(hex(&lz4_compress(&vec![b'a'; n])), block, "{n} a's");
        assert_eq!(lz4_decompress(&h(block), n).unwrap(), vec![b'a'; n]);
    }
    // Built by hand from the spec (and checked with LZ4_decompress_safe):
    // "ab", then 10 bytes from 2 back, which overlap what they write, then
    // 5 literals.
    let hand = [0x26, b'a', b'b', 0x02, 0x00, 0x50, b'x', b'y', b'z', b'1', b'2'];
    assert_eq!(lz4_decompress(&hand, 17).unwrap(), b"ababababababxyz12");
    // `size` is the most it may make, as LZ4_decompress_safe's capacity is:
    // more room is fine, too little is an error.
    assert_eq!(lz4_decompress(&hand, 1000).unwrap(), b"ababababababxyz12");
    assert!(lz4_decompress(&hand, 16).unwrap_err().contains("more than 16 bytes"));
    assert!(lz4_decompress(&block, want.len() - 1).is_err());
}

#[test]
fn lz4_round_trips() {
    let mut rng = Rng::from_seed([4; 32]);
    let mut mixed = Vec::new();
    for i in 0..600u32 {
        let n = (rng.next_u32() % 300) as usize;
        if i.is_multiple_of(3) {
            mixed.extend(rng.bytes(n));
        } else {
            mixed.extend(std::iter::repeat_n((i % 251) as u8, n));
        }
    }
    let far = rng.bytes(70_000);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("one byte", vec![42]),
        ("12 bytes, all literals", vec![b'a'; 12]),
        ("13 bytes, the shortest with a match", vec![b'a'; 13]),
        ("short text", b"to be or not to be, that is the question".to_vec()),
        ("every byte value", (0..=255).collect()),
        ("1 MiB of zeros", vec![0; 1 << 20]),
        (
            "a 7-byte pattern",
            (0..100_000u32).map(|i| (i % 7) as u8 * 31).collect(),
        ),
        ("random", rng.bytes(100_000)),
        ("text", words(&mut rng, 1_500_000)),
        ("runs and noise", mixed),
        // Repeats 70000 bytes apart, beyond the 65535 an offset reaches.
        ("far repeats", [far.clone(), far].concat()),
    ];
    for (name, data) in &cases {
        let block = lz4_compress(data);
        assert!(block.len() <= lz4_bound(data.len()), "{name}");
        let back = lz4_decompress(&block, data.len()).unwrap();
        assert!(back == *data, "{name}: it came back different");
        let matches = check_end_rules(&block, data.len());
        match *name {
            "1 MiB of zeros" => assert!(block.len() < 4200, "{name}: {}", block.len()),
            "a 7-byte pattern" => assert!(block.len() < 500, "{name}: {}", block.len()),
            "text" => assert!(block.len() < data.len() / 2, "{name}: {}", block.len()),
            "random" => assert!(block.len() < data.len() + data.len() / 200, "{name}: {}", block.len()),
            "12 bytes, all literals" => assert_eq!(matches, 0),
            "13 bytes, the shortest with a match" => assert_eq!(matches, 1),
            _ => {}
        }
    }
    // Every length up to 300 of a repetitive input, around the end rules.
    let data = b"abcdefgh".repeat(40);
    for len in 0..data.len() {
        let block = lz4_compress(&data[..len]);
        assert_eq!(lz4_decompress(&block, len).unwrap(), data[..len], "{len}");
        check_end_rules(&block, len);
    }
}

#[test]
fn lz4_rejects_broken_blocks() {
    for (block, size, says) in [
        (&[][..], 10, "at least one byte"),
        (&[0x10], 10, "cut short"),
        (&[0xf0], 10, "cut short"),
        (&[0xf0, 0xff, 0xff], 1000, "cut short"),
        (&[0x10, b'a', 0x01], 10, "cut short"),
        (&[0x1f, b'a', 0x01, 0x00], 100, "cut short"),
        (&[0x1f, b'a', 0x01, 0x00, 0xff], 1000, "cut short"),
        (&[0x10, b'a', 0x00, 0x00, 0x00], 10, "corrupt"),
        (&[0x10, b'a', 0x02, 0x00, 0x00], 10, "corrupt"),
        (&[0x00, 0x01, 0x00, 0x00], 10, "corrupt"),
        (&[0x10, b'a', 0x01, 0x00], 10, "ends with a match"),
        (
            &[0x10, b'a', 0x01, 0x00, 0x10, b'b', 0x01, 0x00],
            20,
            "ends with a match",
        ),
        (b"\x50hello", 4, "more than 4 bytes"),
        (b"\x50hello", 0, "more than 0 bytes"),
        (&[0x1f, b'a', 0x01, 0x00, 0xff, 0x00, 0x00], 200, "more than 200 bytes"),
    ] {
        let err = lz4_decompress(block, size).unwrap_err();
        assert!(err.contains(says), "{block:02x?}: {err}");
    }
    assert_eq!(lz4_decompress(&[0x10, b'a'], 1).unwrap(), b"a");
    assert_eq!(
        lz4_decompress(&[0x1f, b'a', 0x01, 0x00, 0xff, 0x00, 0x00], 275).unwrap(),
        [b'a'; 275]
    );
    // A huge `size` reserves nothing up front: only what the block makes.
    assert_eq!(lz4_decompress(&[0x00], usize::MAX).unwrap(), b"");
    assert_eq!(lz4_decompress(b"\x50hello", usize::MAX).unwrap(), b"hello");
    // Lengths that claim far more than the block holds fail, whatever `size` says.
    let mut claims = vec![0xf0];
    claims.extend([0xff; 10_000]);
    claims.push(0x00);
    assert!(lz4_decompress(&claims, usize::MAX).unwrap_err().contains("cut short"));
    // A match length of 2.5 million is real output (LZ4 expands up to 255
    // times), so it is only made when `size` allows it.
    let mut long = vec![0x1f, b'a', 0x01, 0x00];
    long.extend([0xff; 10_000]);
    long.extend([0x00, 0x00]);
    assert!(lz4_decompress(&long, 1 << 20).unwrap_err().contains("more than"));
    assert_eq!(
        lz4_decompress(&long, usize::MAX).unwrap().len(),
        1 + 15 + 255 * 10_000 + 4
    );
}

#[test]
fn lz4_survives_fuzzing() {
    let mut rng = Rng::from_seed([8; 32]);
    let sources = [
        b"Horizon Studio packs Horizon Studio's scripts with LZ4, and Horizon Studio unpacks them again.".to_vec(),
        b"abcdefgh".repeat(100),
        words(&mut rng, 3000),
        rng.bytes(500),
        vec![0; 2000],
    ];
    let mut decoded = 0;
    for data in &sources {
        let block = lz4_compress(data);
        for round in 0..2000 {
            let mut broken = block.clone();
            match round % 5 {
                // Flip a few bits.
                0 => {
                    for _ in 0..1 + rng.next_u32() % 4 {
                        let i = rng.next_u32() as usize % broken.len();
                        broken[i] ^= 1 << (rng.next_u32() % 8);
                    }
                }
                // Overwrite a byte.
                1 => {
                    let i = rng.next_u32() as usize % broken.len();
                    broken[i] = rng.next_u32() as u8;
                }
                // Cut it short.
                2 => broken.truncate(rng.next_u32() as usize % broken.len()),
                // Insert or append bytes.
                3 => {
                    let i = rng.next_u32() as usize % (broken.len() + 1);
                    let n = 1 + rng.next_u32() as usize % 8;
                    broken = [&broken[..i], &rng.bytes(n), &broken[i..]].concat();
                }
                // Or throw it away for noise.
                _ => {
                    let n = rng.next_u32() as usize % 300;
                    broken = rng.bytes(n);
                }
            }
            for size in [data.len(), rng.next_u32() as usize % (2 * data.len() + 1), usize::MAX] {
                // Never a panic; when it decodes, never past `size`.
                if let Ok(out) = lz4_decompress(&broken, size) {
                    assert!(out.len() <= size);
                    decoded += 1;
                }
            }
        }
    }
    // Some broken blocks still decode (a flipped literal is still a block).
    assert!(decoded > 0);
}
