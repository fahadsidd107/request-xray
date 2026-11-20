use std::{error::Error, fmt, str::FromStr};

/// An ID failed length, encoding, or nonzero validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidId {
    ByteLength { expected: usize, actual: usize },
    HexLength { expected: usize, actual: usize },
    NonHexDigit,
    AllZero,
}

impl fmt::Display for InvalidId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLength { expected, actual } => {
                write!(formatter, "expected {expected} ID bytes, got {actual}")
            }
            Self::HexLength { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} hex digits, got {actual} bytes"
                )
            }
            Self::NonHexDigit => formatter.write_str("ID contains a non-hexadecimal digit"),
            Self::AllZero => formatter.write_str("ID must contain a nonzero byte"),
        }
    }
}

impl Error for InvalidId {}

fn hex_nibble(byte: u8) -> Result<u8, InvalidId> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(InvalidId::NonHexDigit),
    }
}

macro_rules! define_id {
    ($name:ident, $length:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $length]);

        impl $name {
            pub fn as_bytes(&self) -> &[u8; $length] {
                &self.0
            }
        }

        impl TryFrom<[u8; $length]> for $name {
            type Error = InvalidId;

            fn try_from(bytes: [u8; $length]) -> Result<Self, Self::Error> {
                if bytes == [0; $length] {
                    return Err(InvalidId::AllZero);
                }
                Ok(Self(bytes))
            }
        }

        impl TryFrom<&[u8]> for $name {
            type Error = InvalidId;

            fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
                let fixed: [u8; $length] = bytes.try_into().map_err(|_| InvalidId::ByteLength {
                    expected: $length,
                    actual: bytes.len(),
                })?;
                Self::try_from(fixed)
            }
        }

        impl FromStr for $name {
            type Err = InvalidId;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                let hex = input.as_bytes();
                if hex.len() != $length * 2 {
                    return Err(InvalidId::HexLength {
                        expected: $length * 2,
                        actual: hex.len(),
                    });
                }
                let mut bytes = [0; $length];
                for (byte, pair) in bytes.iter_mut().zip(hex.chunks_exact(2)) {
                    *byte = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
                }
                Self::try_from(bytes)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

define_id!(TraceId, 16, "A nonzero 16-byte trace identifier.");
define_id!(SpanId, 8, "A nonzero 8-byte span identifier.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_preserves_bytes_and_formats_lowercase() {
        let id: TraceId = "00112233445566778899AABBCCDDEEFF".parse().unwrap();
        assert_eq!(id.to_string(), "00112233445566778899aabbccddeeff");
        assert_eq!(TraceId::try_from(id.as_bytes().as_slice()), Ok(id));
        assert_eq!(TraceId::try_from(*id.as_bytes()), Ok(id));
        assert_eq!(id.as_bytes()[15], 255);
    }

    #[test]
    fn span_id_preserves_leading_zeroes() {
        let id = SpanId::try_from([0, 0, 0, 0, 0, 0, 0, 1]).unwrap();
        assert_eq!(id.to_string(), "0000000000000001");
        assert_eq!(id.to_string().parse::<SpanId>(), Ok(id));
    }

    #[test]
    fn invalid_ids_are_rejected() {
        assert_eq!(TraceId::try_from([0; 16]), Err(InvalidId::AllZero));
        assert_eq!(
            "0000000000000000".parse::<SpanId>(),
            Err(InvalidId::AllZero)
        );
        assert_eq!(
            SpanId::try_from([1; 9].as_slice()),
            Err(InvalidId::ByteLength {
                expected: 8,
                actual: 9
            })
        );
        assert_eq!(
            "1".parse::<TraceId>(),
            Err(InvalidId::HexLength {
                expected: 32,
                actual: 1
            })
        );
        for invalid in ["0x00000000000001", " 000000000000001", "00000000000000é"] {
            assert_eq!(invalid.parse::<SpanId>(), Err(InvalidId::NonHexDigit));
        }
    }
}
