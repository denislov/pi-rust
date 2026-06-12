mod ansi;
mod width;

pub(crate) use ansi::ansi_sequence_len;
pub use width::{truncate_to_width, visible_width};
