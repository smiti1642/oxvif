//! Offline snapshot evidence, without network access, decoding or file modification.
//! Run: cargo run --example snapshot_probe --features health -- /path/to/captured-body
use oxvif::health::snapshot::{MAX_IMAGE_BYTES, image_type};
use std::{fs::File, io::Read};

fn inspect(bytes: &[u8]) -> serde_json::Value {
    let end = bytes
        .iter()
        .rposition(|b| !matches!(b, b'\r' | b'\n'))
        .map_or(0, |i| i + 1);
    serde_json::json!({
        "format": 1,
        "bytes": bytes.len(),
        "recognized_signature": image_type(bytes),
        "trailing_cr_lf_bytes": bytes.len() - end,
        "signature_without_trailing_cr_lf": image_type(&bytes[..end]),
        "decoded": false,
    })
}

fn run() -> Result<(), &'static str> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("supply exactly one captured response-body file")?;
    if args.next().is_some() {
        return Err("supply exactly one captured response-body file");
    }
    let file = File::open(path).map_err(|_| "cannot open input file (path withheld)")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect input")?
        .is_file()
    {
        return Err("input must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input")?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("input exceeds 16 MiB");
    }
    println!("{}", inspect(&bytes));
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Snapshot evidence: {error}");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_compatibility_difference_without_changing_acceptance_or_echoing_data() {
        let report = inspect(b"\xff\xd8\xff\xd9\r\n");
        assert!(report["recognized_signature"].is_null());
        assert_eq!(report["signature_without_trailing_cr_lf"], "jpeg");
        assert_eq!(report["trailing_cr_lf_bytes"], 2);
        assert_eq!(report["decoded"], false);
        for body in [
            b"<html>synthetic-sensitive-text</html>".as_slice(),
            b"\xff\xd8",
            b"\r\n",
            b"",
        ] {
            let report = inspect(body);
            assert!(report["recognized_signature"].is_null());
            assert!(report["signature_without_trailing_cr_lf"].is_null());
            assert!(!report.to_string().contains("synthetic-sensitive-text"));
        }
    }
}
