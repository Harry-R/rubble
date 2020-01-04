//! P-256 crypto operations.
//!
//! BLE uses P-256 for pairing. This module provides an interface for plugging in different
//! implementations of the P-256 operations. The main consumer of this module is the [`security`]
//! module; refer to that for more info about pairing and encryption in BLE.
//!
//! The primary trait in this module is [`P256Provider`]. Rubble comes with 2 built-in
//! implementations of that trait, which can be enabled via these Cargo features:
//!
//! * **`ring`**: Enables the [`RingProvider`] and [`RingSecretKey`] types, which use the
//!   [*ring* library][ring]. Note that *ring* does not support `#![no_std]` operation, so this is
//!   mostly useful for tests and other non-embedded usage.
//! * **`nisty`**: Enables [`NistyProvider`] and [`NistySecretKey`], which use the [nisty] crate and
//!   [micro-ecc] library. Nisty currently supports Cortex-M4 and Cortex-M33 MCUs.
//!
//! [`security`]: ../security/index.html
//! [`P256Provider`]: trait.P256Provider.html
//! [`RingProvider`]: struct.RingProvider.html
//! [`RingSecretKey`]: struct.RingSecretKey.html
//! [ring]: https://github.com/briansmith/ring
//! [`NistyProvider`]: struct.NistyProvider.html
//! [`NistySecretKey`]: struct.NistySecretKey.html
//! [nisty]: https://github.com/nickray/nisty
//! [micro-ecc]: https://github.com/kmackay/micro-ecc

use {
    core::fmt,
    rand_core::{CryptoRng, RngCore},
};

/// A P-256 public key (point on the curve) in uncompressed format.
///
/// The encoding is as specified in *[SEC 1: Elliptic Curve Cryptography]*, but without the leading
/// byte: The first 32 Bytes are the big-endian encoding of the point's X coordinate, and the
/// remaining 32 Bytes are the Y coordinate, encoded the same way.
///
/// Note that this type does not provide any validity guarantees (unlike [`PrivateKey`]
/// implementors): It is possible to represent invalid public P-256 keys, such as the point at
/// infinity, with this type. The other APIs in this module are designed to take that into account.
///
/// [SEC 1: Elliptic Curve Cryptography]: http://www.secg.org/sec1-v2.pdf
/// [`PrivateKey`]: trait.PrivateKey.html
pub struct PublicKey(pub [u8; 64]);

/// A shared secret resulting from an ECDH key agreement.
///
/// This is returned by implementations of [`SecretKey::agree`].
///
/// [`SecretKey::agree`]: trait.SecretKey.html#tymethod.agree
pub struct SharedSecret(pub [u8; 32]);

/// Error returned by [`SecretKey::agree`] when the public key of the other party is invalid.
///
/// [`SecretKey::agree`]: trait.SecretKey.html#tymethod.agree
#[derive(Debug)]
pub struct InvalidPublicKey {}

impl InvalidPublicKey {
    /// Creates a new `InvalidPublicKey` error.
    pub fn new() -> Self {
        Self {}
    }
}

impl fmt::Display for InvalidPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid public key")
    }
}

/// Trait for P-256 operation providers.
pub trait P256Provider {
    /// Provider-defined secret key type.
    type SecretKey: SecretKey;

    /// Generates a P-256 key pair using cryptographically strong randomness.
    ///
    /// Implementors must ensure that they only return valid private/public key pairs from this
    /// method.
    ///
    /// Rubble will pass a cryptographically secure random number generator `rng` to this function
    /// that may be used to obtain entropy for key generation. Implementations may also use their
    /// own RNG if they so choose.
    fn generate_keypair<R>(&mut self, rng: &mut R) -> (Self::SecretKey, PublicKey)
    where
        R: RngCore + CryptoRng;
}

/// Secret key operations required by Rubble.
///
/// This API imposes no requirements on the representation or location of secret keys. This means
/// that it should be possible to implement this trait even for keys stored in some secure key
/// storage like a smartcard.
pub trait SecretKey: Sized {
    /// Performs ECDH key agreement using an ephemeral secret key `self` and the public key of the
    /// other party.
    ///
    /// Here, "ephemeral" just means that this method takes `self` by value. This allows
    /// implementing `SecretKey` for providers that enforce single-use keys using Rust ownership
    /// (like *ring*).
    ///
    /// # Errors
    ///
    /// If `foreign_key` is an invalid public key, implementors must return an error.
    fn agree(self, foreign_key: &PublicKey) -> Result<SharedSecret, InvalidPublicKey>;
}

/// Runs Rubble's P-256 provider testsuite against `provider`.
///
/// Note that this is just a quick smoke test that does not provide any assurance about security
/// properties. The P-256 provider should have a dedicated test suite.
pub fn run_tests(mut provider: impl P256Provider) {
    static RNG: &[u8] = &[
        0x1e, 0x66, 0x81, 0xb6, 0xa3, 0x4e, 0x06, 0x97, 0x75, 0xbe, 0xd4, 0x5c, 0xf9, 0x52, 0x3f,
        0xf1, 0x5b, 0x6a, 0x72, 0xe2, 0xb8, 0x35, 0xb3, 0x29, 0x5e, 0xe0, 0xbb, 0x92, 0x35, 0xa5,
        0xb9, 0x60, 0xc9, 0xaf, 0xe2, 0x72, 0x12, 0xf1, 0xc4, 0xfc, 0x10, 0x2d, 0x63, 0x2f, 0x05,
        0xd6, 0xe5, 0x0a, 0xbf, 0x2c, 0xb9, 0x02, 0x3a, 0x67, 0x23, 0x63, 0x36, 0x7a, 0x62, 0xe6,
        0x63, 0xce, 0x28, 0x98,
    ];

    // Pretend-RNG that returns a fixed sequence of pregenerated numbers. Do not do this outside of
    // tests.
    struct Rng(&'static [u8]);

    impl RngCore for Rng {
        fn next_u32(&mut self) -> u32 {
            rand_core::impls::next_u32_via_fill(self)
        }
        fn next_u64(&mut self) -> u64 {
            rand_core::impls::next_u64_via_fill(self)
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            if self.0.len() < dest.len() {
                panic!("p256::run_tests: ran out of pregenerated entropy");
            }

            for chunk in dest.chunks_mut(self.0.len()) {
                chunk.copy_from_slice(&self.0[..chunk.len()]);
                self.0 = &self.0[chunk.len()..];
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    impl CryptoRng for Rng {}

    // Test that different key pairs will be generated:
    let mut rng = Rng(RNG);
    let (secret1, public1) = provider.generate_keypair(&mut rng);
    let (secret2, public2) = provider.generate_keypair(&mut rng);
    assert_ne!(&public1.0[..], &public2.0[..]);

    // Test that ECDH agreement results in the same shared secret:
    let shared1 = secret1.agree(&public2).unwrap();
    let shared2 = secret2.agree(&public1).unwrap();
    assert_eq!(shared1.0, shared2.0);

    // Now, test that ECDH agreement with invalid public keys fails correctly.

    // Point at infinity is an invalid public key:
    let infty = PublicKey([0; 64]);
    let (secret, _) = provider.generate_keypair(&mut Rng(RNG));
    assert!(secret.agree(&infty).is_err());

    // Malicious public key not on the curve:
    // (taken from https://web-in-security.blogspot.com/2015/09/practical-invalid-curve-attacks.html)
    let x = [
        0xb7, 0x0b, 0xf0, 0x43, 0xc1, 0x44, 0x93, 0x57, 0x56, 0xf8, 0xf4, 0x57, 0x8c, 0x36, 0x9c,
        0xf9, 0x60, 0xee, 0x51, 0x0a, 0x5a, 0x0f, 0x90, 0xe9, 0x3a, 0x37, 0x3a, 0x21, 0xf0, 0xd1,
        0x39, 0x7f,
    ];
    let y = [
        0x4a, 0x2e, 0x0d, 0xed, 0x57, 0xa5, 0x15, 0x6b, 0xb8, 0x2e, 0xb4, 0x31, 0x4c, 0x37, 0xfd,
        0x41, 0x55, 0x39, 0x5a, 0x7e, 0x51, 0x98, 0x8a, 0xf2, 0x89, 0xcc, 0xe5, 0x31, 0xb9, 0xc1,
        0x71, 0x92,
    ];
    let mut key = [0; 64];
    key[..32].copy_from_slice(&x);
    key[32..].copy_from_slice(&y);

    let (secret, _) = provider.generate_keypair(&mut Rng(RNG));
    assert!(secret.agree(&PublicKey(key)).is_err());
}

#[cfg(feature = "ring")]
pub use self::ring::*;

#[cfg(feature = "ring")]
mod ring {
    use {
        super::*,
        ::ring::{
            agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256},
            rand::SystemRandom,
        },
    };

    /// A P-256 provider that uses *ring* under the hood.
    pub struct RingProvider {
        rng: SystemRandom,
    }

    impl RingProvider {
        /// Creates a new `RingProvider` that uses the system's RNG for key generation.
        pub fn new() -> Self {
            Self {
                rng: SystemRandom::new(),
            }
        }
    }

    impl P256Provider for RingProvider {
        type SecretKey = RingSecretKey;

        fn generate_keypair<R>(&mut self, _: &mut R) -> (Self::SecretKey, PublicKey)
        where
            R: RngCore + CryptoRng,
        {
            let secret = EphemeralPrivateKey::generate(&ECDH_P256, &self.rng).unwrap();
            let public = secret.compute_public_key().unwrap();

            let mut pub_bytes = [0; 64];
            // Strip the first octet (indicates the key type; see RFC 5480)
            pub_bytes.copy_from_slice(&public.as_ref()[1..]);

            let secret = RingSecretKey(secret);
            let public = PublicKey(pub_bytes);

            (secret, public)
        }
    }

    /// A secret key generated by a `RingProvider`.
    pub struct RingSecretKey(EphemeralPrivateKey);

    impl SecretKey for RingSecretKey {
        fn agree(self, foreign_key: &PublicKey) -> Result<SharedSecret, InvalidPublicKey> {
            // Convert `foreign_key` to ring's format:
            let mut encoded = [0; 65];
            encoded[0] = 0x04; // indicates uncompressed format (see RFC 5480)
            encoded[1..].copy_from_slice(&foreign_key.0);
            let public = UnparsedPublicKey::new(&ECDH_P256, &encoded[..]);

            let mut shared_secret = [0; 32];
            agree_ephemeral(self.0, &public, InvalidPublicKey::new(), |b| {
                shared_secret.copy_from_slice(b);
                Ok(())
            })?;

            Ok(SharedSecret(shared_secret))
        }
    }
}

#[cfg(feature = "nisty")]
pub use self::nisty::*;

#[cfg(feature = "nisty")]
mod nisty {
    use {
        super::*,
        rand_core::{CryptoRng, RngCore},
    };

    pub struct NistyProvider {}

    impl NistyProvider {
        /// Creates a new nisty P-256 operation provider.
        pub fn new() -> Self {
            Self {}
        }
    }

    impl P256Provider for NistyProvider {
        type SecretKey = NistySecretKey;

        fn generate_keypair<R>(&mut self, rng: &mut R) -> (Self::SecretKey, PublicKey)
        where
            R: RngCore + CryptoRng,
        {
            let mut seed = [0; 32];
            rng.fill_bytes(&mut seed);

            let keypair = ::nisty::Keypair::generate_patiently(&seed);
            let (secret, public) = keypair.split();

            let secret = NistySecretKey(secret);
            let public = PublicKey(public.to_bytes());

            (secret, public)
        }
    }

    /// A secret key generated by a `NistyProvider`.
    pub struct NistySecretKey(::nisty::SecretKey);

    impl SecretKey for NistySecretKey {
        fn agree(self, foreign_key: &PublicKey) -> Result<SharedSecret, InvalidPublicKey> {
            let public = ::nisty::PublicKey::try_from_bytes(&foreign_key.0)
                .map_err(|_| InvalidPublicKey::new())?;

            // `agree` only returns an error if the public key is the point at infinity, which is
            // ruled out by the conversion above.
            let shared_secret = self.0.agree(&public).unwrap().to_bytes();

            Ok(SharedSecret(shared_secret))
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        nisty::{Keypair, PublicKey},
        ring::{
            agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256},
            rand::SystemRandom,
        },
    };

    /// Performs ECDH key agreement between ring and nisty.
    ///
    /// This involves generating key pairs and converting the public keys to the format expected by
    /// the other library. It serves as a sort of sanity check, to ensure that such a key agreement
    /// is possible.
    #[test]
    fn raw_ring_nisty_agreement() {
        // Generate a nisty key pair by iterating a fixed seed. It doesn't matter which key we use
        // here.
        const NISTY_SEED: [u8; 32] = [0xAA; 32];
        let n_pair = nisty::Keypair::generate_patiently(NISTY_SEED);
        let (n_secret, n_public) = n_pair.split();

        // Now generate the ring key pair.
        let rng = SystemRandom::new();
        let r_secret = EphemeralPrivateKey::generate(&ECDH_P256, &rng).unwrap();
        let r_public = r_secret.compute_public_key().unwrap();

        // Convert the nisty public key to ring's expected format.
        let mut encoded = [0; 65];
        encoded[0] = 0x04; // uncompressed
        encoded[1..].copy_from_slice(n_public.as_bytes());
        let n_public = UnparsedPublicKey::new(&ECDH_P256, &encoded[..]);

        // Convert ring's public key to nisty's expected format.
        let mut bytes = [0; 64];
        bytes.copy_from_slice(&r_public.as_ref()[1..]);
        let r_public = nisty::PublicKey::try_from_bytes(&bytes).unwrap();

        // Do the ring-side agreement.
        let mut r_shared = [0; 32];
        agree_ephemeral(r_secret, &n_public, (), |b| {
            r_shared.copy_from_slice(b);
            Ok(())
        })
        .unwrap();

        // Do the nisty-side agreement.
        let n_shared = n_secret.agree(&r_public).unwrap().to_bytes();

        // The derived secret must be identical, or we messed something up.
        assert_eq!(r_shared, n_shared);
    }

    /// Performs key agreement between the ring and nisty `P256Provider` implementations.
    ///
    /// This uses the Rubble API, so the `ring` and `nisty` Cargo features must be enabled for the
    /// test to work.
    #[test]
    #[cfg(all(feature = "ring", feature = "nisty"))]
    fn ring_nisty_agreement() {
        use super::{NistyProvider, P256Provider, RingProvider, SecretKey};
        use rand_core::{CryptoRng, RngCore};

        // Pretend-RNG that returns a constant value. Do not do this outside of tests.
        struct Rng;

        impl RngCore for Rng {
            fn next_u32(&mut self) -> u32 {
                0xAAAAAAAA
            }
            fn next_u64(&mut self) -> u64 {
                0xAAAAAAAAAAAAAAAA
            }
            fn fill_bytes(&mut self, dest: &mut [u8]) {
                for byte in dest {
                    *byte = 0xAA;
                }
            }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
                self.fill_bytes(dest);
                Ok(())
            }
        }

        impl CryptoRng for Rng {}

        let mut ring = RingProvider::new();
        let (r_secret, r_public) = ring.generate_keypair(&mut Rng);

        let mut nisty = NistyProvider::new();
        let (n_secret, n_public) = nisty.generate_keypair(&mut Rng);

        let r_shared = r_secret.agree(&n_public).unwrap();
        let n_shared = n_secret.agree(&r_public).unwrap();

        assert_eq!(r_shared.0, n_shared.0);
    }

    #[test]
    #[cfg(not(all(feature = "ring", feature = "nisty")))]
    #[ignore]
    fn ring_nisty_agreement() {
        panic!("this test requires the `ring` and `nisty` features to be enabled");
    }

    /// Uses nisty to verify the Bluetooth test vectors.
    ///
    /// See "7.1.2 P-256 sample data" in the spec.
    #[test]
    fn nisty_test_vectors() {
        fn parse_into(mut slice: &mut [u8], s: &str) {
            for s_word in s.split_whitespace() {
                assert_eq!(s_word.len(), 8);

                let target = &mut slice[..4];
                for i in 0..4 {
                    target[i] = u8::from_str_radix(&s_word[i * 2..i * 2 + 2], 16).unwrap();
                }
                slice = &mut slice[4..];
            }

            assert!(slice.is_empty());
        }

        // Strings copied straight from the spec
        const PRIV_A: &str =
            "3f49f6d4 a3c55f38 74c9b3e3 d2103f50 4aff607b eb40b799 5899b8a6 cd3c1abd";
        const PUB_A_X: &str =
            "20b003d2 f297be2c 5e2c83a7 e9f9a5b9 eff49111 acf4fddb cc030148 0e359de6";
        const PUB_A_Y: &str =
            "dc809c49 652aeb6d 63329abf 5a52155c 766345c2 8fed3024 741c8ed0 1589d28b";

        const PRIV_B: &str =
            "55188b3d 32f6bb9a 900afcfb eed4e72a 59cb9ac2 f19d7cfb 6b4fdd49 f47fc5fd";
        const PUB_B_X: &str =
            "1ea1f0f0 1faf1d96 09592284 f19e4c00 47b58afd 8615a69f 559077b2 2faaa190";
        const PUB_B_Y: &str =
            "4c55f33e 429dad37 7356703a 9ab85160 472d1130 e28e3676 5f89aff9 15b1214a";

        const DHKEY: &str =
            "ec0234a3 57c8ad05 341010a6 0a397d9b 99796b13 b4f866f1 868d34f3 73bfa698";

        let mut priv_a = [0; 32];
        parse_into(&mut priv_a, PRIV_A);
        let key_a = Keypair::try_from_bytes(&priv_a).unwrap();

        let mut pub_a_bytes = [0; 64];
        parse_into(&mut pub_a_bytes[..32], PUB_A_X);
        parse_into(&mut pub_a_bytes[32..], PUB_A_Y);
        let pub_a = PublicKey::try_from_bytes(&pub_a_bytes).unwrap();

        assert_eq!(key_a.public, pub_a);

        let mut priv_b = [0; 32];
        parse_into(&mut priv_b, PRIV_B);
        let key_b = Keypair::try_from_bytes(&priv_b).unwrap();

        let mut pub_b_bytes = [0; 64];
        parse_into(&mut pub_b_bytes[..32], PUB_B_X);
        parse_into(&mut pub_b_bytes[32..], PUB_B_Y);
        let pub_b = PublicKey::try_from_bytes(&pub_b_bytes).unwrap();

        assert_eq!(key_b.public, pub_b);

        let shared_a = key_a.secret.agree(&pub_b).unwrap();
        let shared_b = key_b.secret.agree(&pub_a).unwrap();
        let mut dhkey = [0; 32];
        parse_into(&mut dhkey, DHKEY);
        assert_eq!(shared_a, shared_b);
        assert_eq!(shared_a.as_bytes(), &dhkey);
    }

    #[test]
    #[cfg(feature = "nisty")]
    fn nisty_testsuite() {
        super::run_tests(super::NistyProvider::new());
    }

    #[test]
    #[cfg(not(feature = "nisty"))]
    #[ignore]
    fn nisty_testsuite() {
        panic!("this test requires the `nisty` feature to be enabled");
    }

    #[test]
    #[cfg(feature = "ring")]
    fn ring_testsuite() {
        super::run_tests(super::RingProvider::new());
    }

    #[test]
    #[cfg(not(feature = "ring"))]
    #[ignore]
    fn ring_testsuite() {
        panic!("this test requires the `ring` feature to be enabled");
    }
}
