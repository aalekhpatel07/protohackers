mod encode;
mod decode;
pub mod error;

pub use encode::{
    Protohackers as ProtohackersSerializer,
    to_vec,
    to_writer,
};
pub use decode::Protohackers as ProtohackersDeserializer;
