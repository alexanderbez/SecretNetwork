use crate::consts::*;
use crate::crypto::keys::{AESKey, KeyPair, Seed};
use crate::crypto::traits::*;
use enclave_ffi_types::{CryptoError, EnclaveError};
use lazy_static::lazy_static;
use log::*;

pub struct Keychain {
    consensus_seed: Option<Seed>,
    consensus_state_ikm: Option<AESKey>,
    consensus_seed_exchange_keypair: Option<KeyPair>,
    consensus_io_exchange_keypair: Option<KeyPair>,
    registration_key: Option<KeyPair>,
}

lazy_static! {
    pub static ref KEY_MANAGER: Keychain = Keychain::new();
}

impl Keychain {
    pub fn new() -> Self {
        let consensus_seed = match Seed::unseal(CONSENSUS_SEED_SEALING_PATH) {
            Ok(k) => Some(k),
            Err(e) => None,
        };

        let registration_key = match KeyPair::unseal(REGISTRATION_KEY_SEALING_PATH) {
            Ok(k) => Some(k),
            Err(e) => None,
        };

        let mut x = Keychain {
            consensus_seed,
            registration_key,
            consensus_state_ikm: None,
            consensus_seed_exchange_keypair: None,
            consensus_io_exchange_keypair: None,
        };

        x.generate_consensus_master_keys();

        return x;
    }

    pub fn create_consensus_seed(&mut self) -> Result<(), CryptoError> {
        match Seed::new() {
            Ok(seed) => {
                if let Err(e) = self.set_consensus_seed(seed) {
                    return Err(CryptoError::KeyError);
                }
            }
            Err(err) => return Err(err),
        };
        Ok(())
    }

    pub fn create_registration_key(&mut self) -> Result<(), CryptoError> {
        match KeyPair::new() {
            Ok(key) => {
                if let Err(e) = self.set_registration_key(key) {
                    return Err(CryptoError::KeyError);
                }
            }
            Err(err) => return Err(err),
        };
        Ok(())
    }

    pub fn is_registration_key_set(&self) -> bool {
        return self.registration_key.is_some();
    }

    pub fn is_consensus_state_ikm_set(&self) -> bool {
        return self.consensus_state_ikm.is_some();
    }

    pub fn is_consensus_seed_exchange_keypair_set(&self) -> bool {
        return self.consensus_seed_exchange_keypair.is_some();
    }

    pub fn is_consensus_io_exchange_keypair_set(&self) -> bool {
        return self.consensus_io_exchange_keypair.is_some();
    }

    pub fn is_consensus_seed_set(&self) -> bool {
        return self.consensus_seed.is_some();
    }

    pub fn get_consensus_state_ikm(&self) -> Result<AESKey, CryptoError> {
        if self.consensus_state_ikm.is_some() {
            Ok(self.consensus_state_ikm.unwrap())
        } else {
            error!("Error accessing base_state_key (does not exist, or was not initialized)");
            Err(CryptoError::ParsingError)
        }
    }

    pub fn get_consensus_seed(&self) -> Result<Seed, CryptoError> {
        if self.consensus_seed.is_some() {
            Ok(self.consensus_seed.unwrap())
        } else {
            error!("Error accessing consensus_seed (does not exist, or was not initialized)");
            Err(CryptoError::ParsingError)
        }
    }

    pub fn seed_exchange_key(&self) -> Result<KeyPair, CryptoError> {
        if self.consensus_seed_exchange_keypair.is_some() {
            // KeyPair does not implement copy (due to internal type not implementing it
            Ok(self.consensus_seed_exchange_keypair.clone().unwrap())
        } else {
            error!("Error accessing consensus_seed_exchange_keypair (does not exist, or was not initialized)");
            Err(CryptoError::ParsingError)
        }
    }

    pub fn get_consensus_io_exchange_keypair(&self) -> Result<KeyPair, CryptoError> {
        if self.consensus_io_exchange_keypair.is_some() {
            // KeyPair does not implement copy (due to internal type not implementing it
            Ok(self.consensus_io_exchange_keypair.clone().unwrap())
        } else {
            error!("Error accessing consensus_io_exchange_keypair (does not exist, or was not initialized)");
            Err(CryptoError::ParsingError)
        }
    }

    pub fn get_registration_key(&self) -> Result<KeyPair, CryptoError> {
        if self.registration_key.is_some() {
            // KeyPair does not implement copy (due to internal type not implementing it
            Ok(self.registration_key.clone().unwrap())
        } else {
            error!("Error accessing registration_key (does not exist, or was not initialized)");
            Err(CryptoError::ParsingError)
        }
    }

    pub fn set_registration_key(&mut self, kp: KeyPair) -> Result<(), EnclaveError> {
        if let Err(e) = kp.seal(REGISTRATION_KEY_SEALING_PATH) {
            error!("Error sealing registration key");
            return Err(e);
        }
        Ok(self.registration_key = Some(kp.clone()))
    }

    pub fn set_consensus_seed_exchange_keypair(&mut self, kp: KeyPair) {
        self.consensus_seed_exchange_keypair = Some(kp.clone())
    }

    pub fn set_consensus_io_exchange_keypair(&mut self, kp: KeyPair) {
        self.consensus_io_exchange_keypair = Some(kp.clone())
    }

    pub fn set_consensus_state_ikm(&mut self, consensus_state_ikm: AESKey) {
        self.consensus_state_ikm = Some(consensus_state_ikm.clone());
    }

    pub fn set_consensus_seed(&mut self, consensus_seed: Seed) -> Result<(), EnclaveError> {
        if let Err(e) = consensus_seed.seal(CONSENSUS_SEED_SEALING_PATH) {
            error!("Error sealing consensus_seed");
            return Err(e);
        }
        Ok(self.consensus_seed = Some(consensus_seed.clone()))
    }

    pub fn generate_consensus_master_keys(&mut self) -> Result<(), EnclaveError> {
        if !self.is_consensus_seed_set() {
            debug!("Seed not initialized! Cannot derive enclave keys");
            return Ok(());
        }

        // consensus_seed_exchange_keypair

        let consensus_seed_exchange_keypair_bytes = self
            .consensus_seed
            .unwrap()
            .derive_key_from_this(&CONSENSUS_SEED_EXCHANGE_KEYPAIR_DERIVE_ORDER.to_be_bytes());
        let consensus_seed_exchange_keypair = KeyPair::new_from_slice(
            &consensus_seed_exchange_keypair_bytes.get(),
        )
        .map_err(|err| {
            error!(
                "[Enclave] Error creating consensus_seed_exchange_keypair: {:?}",
                err
            );
            EnclaveError::FailedUnseal /* change error type? */
        })?;
        info!(
            "consensus_seed_exchange_keypair: {:?}",
            consensus_seed_exchange_keypair
        );
        self.set_consensus_seed_exchange_keypair(consensus_seed_exchange_keypair);

        // consensus_io_exchange_keypair

        let consensus_io_exchange_keypair_bytes = self
            .consensus_seed
            .unwrap()
            .derive_key_from_this(&CONSENSUS_IO_EXCHANGE_KEYPAIR_DERIVE_ORDER.to_be_bytes());
        let consensus_io_exchange_keypair =
            KeyPair::new_from_slice(&consensus_io_exchange_keypair_bytes.get()).map_err(|err| {
                error!(
                    "[Enclave] Error creating consensus_io_exchange_keypair: {:?}",
                    err
                );
                EnclaveError::FailedUnseal /* change error type? */
            })?;
        info!(
            "consensus_io_exchange_keypair: {:?}",
            consensus_io_exchange_keypair
        );
        self.set_consensus_io_exchange_keypair(consensus_io_exchange_keypair);

        // consensus_state_ikm

        let consensus_state_ikm_bytes = self
            .consensus_seed
            .unwrap()
            .derive_key_from_this(&CONSENSUS_STATE_IKM_DERIVE_ORDER.to_be_bytes());
        let consensus_state_ikm = AESKey::new_from_slice(consensus_state_ikm_bytes.get());
        info!("consensus_state_ikm: {:?}", consensus_state_ikm);
        self.set_consensus_state_ikm(consensus_state_ikm);

        Ok(())
    }
}

#[cfg(feature = "test")]
pub mod tests {

    use super::{
        Keychain, CONSENSUS_SEED_SEALING_PATH, KEY_MANAGER, REGISTRATION_KEY_SEALING_PATH,
    };
    use crate::crypto::{KeyPair, Seed};
    use enclave_ffi_types::CryptoError;

    // todo: fix test vectors to actually work
    fn test_initial_keychain_state() {
        // clear previous data (if any)
        std::sgxfs::remove(CONSENSUS_SEED_SEALING_PATH);
        std::sgxfs::remove(REGISTRATION_KEY_SEALING_PATH);

        let keys = Keychain::new();

        // todo: replace with actual checks
        assert_eq!(keys.get_registration_key(), Err(CryptoError));
        assert_eq!(keys.get_consensus_seed(), Err(CryptoError));
        assert_eq!(keys.get_consensus_io_exchange_keypair(), Err(CryptoError));
        assert_eq!(keys.get_consensus_state_ikm(), Err(CryptoError));
    }

    // todo: fix test vectors to actually work
    fn test_initialize_keychain_seed() {
        // clear previous data (if any)
        std::sgxfs::remove(CONSENSUS_SEED_SEALING_PATH);
        std::sgxfs::remove(REGISTRATION_KEY_SEALING_PATH);

        let mut keys = Keychain::new();

        let seed = Seed::new_from_slice(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

        keys.set_consensus_seed(seed);
        keys.generate_consensus_master_keys();
        // todo: replace with actual checks
        assert_eq!(keys.get_registration_key(), Err(CryptoError));
        assert_eq!(keys.get_consensus_seed().unwrap(), seed);
    }

    // todo: fix test vectors to actually work
    fn test_initialize_keychain_registration() {
        // clear previous data (if any)
        std::sgxfs::remove(CONSENSUS_SEED_SEALING_PATH);
        std::sgxfs::remove(REGISTRATION_KEY_SEALING_PATH);

        let mut keys = Keychain::new();

        let kp = KeyPair::new_from_slice(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();

        keys.set_registration_key(kp);
        // todo: replace with actual checks
        assert_eq!(keys.get_registration_key().unwrap(), kp);
    }

    // todo: fix test vectors to actually work
    fn test_initialize_keys() {
        // clear previous data (if any)
        std::sgxfs::remove(CONSENSUS_SEED_SEALING_PATH);
        std::sgxfs::remove(REGISTRATION_KEY_SEALING_PATH);

        let mut keys = Keychain::new();

        let seed = Seed::new_from_slice(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

        keys.set_consensus_seed(seed);
        keys.generate_consensus_master_keys();
        // todo: replace with actual checks
        assert_eq!(keys.get_consensus_io_exchange_keypair().unwrap(), seed);
        assert_eq!(keys.get_consensus_state_ikm().unwrap(), seed);
    }

    // todo: fix test vectors to actually work
    fn test_key_manager() {
        // clear previous data (if any)
        std::sgxfs::remove(CONSENSUS_SEED_SEALING_PATH);
        std::sgxfs::remove(REGISTRATION_KEY_SEALING_PATH);

        let seed = Seed::new_from_slice(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        let mut keys = Keychain::new();
        keys.set_consensus_seed(seed);
        keys.generate_consensus_master_keys();

        // todo: replace with actual checks
        assert_eq!(
            KEY_MANAGER.get_consensus_io_exchange_keypair().unwrap(),
            seed
        );
        assert_eq!(KEY_MANAGER.get_consensus_state_ikm().unwrap(), seed);
    }
}