pub mod ser {
    use std::num::TryFromIntError;
    #[derive(Debug)]
    pub enum Error {
        UnknownLength,
        UnsupportedDataType {
            r#type: String,
            context: Option<String>,
        },
        ValueTooBig {
            r#type: String,
            max_value: u32,
            observed: Option<u32>,
            _inner: Option<Box<dyn std::error::Error>>,
        },
        ValueWriteFailed(std::io::Error)
    }

    impl core::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::UnknownLength => write!(f, "Error::UnknownLength"),
                Error::UnsupportedDataType { r#type, context } => write!(f, "Error::UnsupportedDataType: {}: {}", r#type, context.clone().unwrap_or_default()),
                Error::ValueTooBig { r#type, max_value, observed, _inner } => {
                    write!(f, "ValueTooBig: {}", r#type)?;
                    if let Some(observed) = *observed {
                        write!(f, " (received: {observed}, expected max: {max_value})")?;
                    }
                    if let Some(inner) = _inner {
                        write!(f, "\n{inner}")?;
                    }
                    Ok(())
                },
                Error::ValueWriteFailed(error) => write!(f, "Error::ValueWriteFailed: {}", error)
            }
        }
    }

    impl From<std::io::Error> for Error {
        fn from(value: std::io::Error) -> Self {
            Self::ValueWriteFailed(value)
        }
    }

    impl std::error::Error for Error {}

    impl serde::ser::Error for Error {
        fn custom<T>(msg:T) -> Self where T:std::fmt::Display {
            Self::UnsupportedDataType {
                r#type: "other".to_string(),
                context: Some(msg.to_string())
            }
        }
    }


    impl From<TryFromIntError> for Error {
        fn from(value: TryFromIntError) -> Self {
            Self::ValueTooBig { 
                r#type: std::any::type_name::<TryFromIntError>().to_string(), 
                max_value: u32::MAX, 
                observed: None,
                _inner: Some(Box::new(value))
            }
        }
    }
}



pub mod de {
    use std::num::TryFromIntError;
    #[derive(Debug)]
    pub enum Error {
        TrailingData {
            buffer: Vec<u8>
        },
        UnexpectedEOF,
        ExpectedU32,
        ValueReadFailed(std::io::Error),
        Other(String),
    }

    impl core::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::ValueReadFailed(error) => write!(f, "ValueReadFailed: {}", error),
                Error::TrailingData { buffer } => write!(f, "TrailingData: ({:03} bytes): {:x?}", buffer.len(), buffer),
                Error::Other(msg) => write!(f, "Other: {msg}"),
                _ => write!(f, "")
            }
        }
    }

    impl From<std::io::Error> for Error {
        fn from(value: std::io::Error) -> Self {
            Self::ValueReadFailed(value)
        }
    }

    impl std::error::Error for Error {}

    impl serde::de::Error for Error {
        fn custom<T>(msg:T) -> Self where T:std::fmt::Display {
            Self::Other(msg.to_string())
        }
    }

    impl From<TryFromIntError> for Error {
        fn from(value: TryFromIntError) -> Self {
            Self::Other(value.to_string())
        }
    }
}
