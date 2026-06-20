mod ansi;
mod width;

pub(crate) use ansi::ansi_sequence_len;
pub use width::{
    truncate_to_width, truncate_to_width_with_ellipsis, visible_width, wrap_text_with_ansi,
};
