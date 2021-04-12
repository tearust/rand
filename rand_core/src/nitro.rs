use crate::{impls, CryptoRng, Error, RngCore};
use nsm_io::ErrorCode;

const RANDOM_MAX_LEN: usize = 256;

/// A random number generator that retrieves randomness from from the
/// NSM (NitroSecureModule).
#[derive(Clone, Copy, Debug, Default)]
pub struct OsRng;

impl CryptoRng for OsRng {}

impl RngCore for OsRng {
    fn next_u32(&mut self) -> u32 {
        impls::next_u32_via_fill(self)
    }

    fn next_u64(&mut self) -> u64 {
        impls::next_u64_via_fill(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        if let Err(e) = self.try_fill_bytes(dest) {
            panic!("Error: {}", e);
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        fill_bytes(dest)
    }
}

pub fn fill_bytes(dest: &mut [u8]) -> Result<(), Error> {
    let fd = nsm::nsm_lib_init();

    let mut bytes = [0u8; RANDOM_MAX_LEN];
    let mut len = dest.len();
    let err_code = unsafe { nsm::nsm_get_random(fd, bytes.as_mut_ptr(), &mut len) };

    nsm::nsm_lib_exit(fd);

    match err_code {
        ErrorCode::Success => {
            dest.copy_from_slice(&bytes[..len]);
            Ok(())
        }
        _ => Err(Error::from(err_code)),
    }
}
