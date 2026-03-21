use std::io::Write;

use super::{ContextPack, OutputError};

/// ContextPack を pretty-printed JSON として出力する
pub fn format_context_pack(pack: &ContextPack, writer: &mut dyn Write) -> Result<(), OutputError> {
    serde_json::to_writer_pretty(writer, pack)?;
    Ok(())
}
