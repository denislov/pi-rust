use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiSnapshot {
    format_version: u32,
    generator: &'static str,
    crate_name: String,
    items: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let crate_name = args
        .next()
        .ok_or("missing crate name")?
        .into_string()
        .map_err(|_| "crate name must be UTF-8")?;
    let rustdoc_json = PathBuf::from(args.next().ok_or("missing rustdoc JSON path")?);
    let output = PathBuf::from(args.next().ok_or("missing output path")?);
    if args.next().is_some() {
        return Err("usage: pi-public-api-snapshot <crate> <rustdoc-json> <output>".into());
    }

    let public_api = public_api::Builder::from_rustdoc_json(rustdoc_json).build()?;
    let snapshot = ApiSnapshot {
        format_version: 1,
        generator: "public-api 0.52.0",
        crate_name,
        items: public_api.items().map(ToString::to_string).collect(),
    };

    let mut writer = BufWriter::new(File::create(output)?);
    serde_json::to_writer_pretty(&mut writer, &snapshot)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
