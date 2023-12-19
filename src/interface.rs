use core::ops::{Deref, DerefMut};

#[cfg(feature = "eh1")]
use embedded_can::{Frame, Id, StandardId};
#[cfg(feature = "eh0")]
use embedded_hal::can::{Frame, Id, StandardId};

use crate::error::ChademoError;

pub(crate) fn raw_to_id(id: u16) -> Id {
    Id::from(Id::Standard(StandardId::new(id).unwrap()))
}
pub(crate) fn standard_id_to_raw(id: Id) -> Result<u16, ChademoError> {
    match id {
        Id::Standard(id) => Ok(id.as_raw()),
        Id::Extended(_) => Err(ChademoError::DecodeBadIdExt),
    }
}

/// CAN data structure from BXcan crate
///  https://github.com/stm32-rs/bxcan/blob/3fc7a0e81975d4f25e61e0da81cd9e7a5e969e81/src/frame.rs#L157C18-L157C18
/// Payload of a CAN data frame.
///
/// Contains 0 to 8 Bytes of data.
///
/// `Data` implements `From<[u8; N]>` for all `N` up to 8, which provides a convenient lossless
/// conversion from fixed-length arrays.
#[derive(Debug, Copy, Clone)]
pub struct Data {
    pub(crate) len: u8,
    pub(crate) bytes: [u8; 8],
}

impl Data {
    /// Creates a data payload from a raw byte slice.
    ///
    /// Returns `None` if `data` contains more than 8 Bytes (which is the maximum).
    ///
    /// `Data` can also be constructed from fixed-length arrays up to length 8 via `From`/`Into`.
    pub fn new(data: &[u8]) -> Option<Self> {
        if data.len() > 8 {
            return None;
        }

        let mut bytes = [0; 8];
        bytes[..data.len()].copy_from_slice(data);

        Some(Self {
            len: data.len() as u8,
            bytes,
        })
    }
}

impl Deref for Data {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl DerefMut for Data {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..usize::from(self.len)]
    }
}

impl AsRef<[u8]> for Data {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl AsMut<[u8]> for Data {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.deref_mut()
    }
}

pub struct ChademoCanFrame {
    data: Data,
    id: u32,
    rtr: bool,
    err: bool,
}

// #[cfg(any(feature=["eh0", "eh1"]))]
impl Frame for ChademoCanFrame {
    fn new(id: impl Into<Id>, data: &[u8]) -> Option<Self> {
        let id = id.into();
        let id = match id {
            // Id::Extended(foo) => foo.as_raw(),
            Id::Standard(id) => id.as_raw() as u32,
            _ => return None,
        };

        Data::new(data).and_then(|data| {
            Some(Self {
                data,
                id,
                // dlc: data.len() as u8,
                rtr: false,
                err: false,
            })
        })
    }

    fn new_remote(_id: impl Into<Id>, _dlc: usize) -> Option<Self> {
        None
    }

    fn is_extended(&self) -> bool {
        self.err
    }

    fn is_remote_frame(&self) -> bool {
        self.rtr
    }

    fn id(&self) -> Id {
        Id::from(Id::Standard(
            StandardId::new(self.id as u16).expect("StandardID construction failed"),
        ))
    }

    fn dlc(&self) -> usize {
        self.data.len()
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(feature = "test")]
impl CanFrameInterface for ChademoCanFrame {
    fn new(id: u32, data: &[u8]) -> Self {
        Self {
            data: Data::new(data).unwrap(),
            id,
            rtr: false,
            err: false,
        }
    }
    fn id(&self) -> u32 {
        self.id
    }
    fn data(&self) -> &[u8] {
        &self.data
    }
}
