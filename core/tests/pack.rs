use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use flate2::write::GzEncoder;
use flate2::Compression;
use unsealed_core::pack::{install_pack, read_pack, write_pack, MANIFEST};

/// A fresh directory of its own in the system's temporary directory.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("unsealed-pack-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn gzip(records: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = GzEncoder::new(Vec::new(), Compression::fast());
    for (name, bytes) in records {
        out.write_all(&(name.len() as u32).to_le_bytes()).unwrap();
        out.write_all(name.as_bytes()).unwrap();
        out.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
        out.write_all(bytes).unwrap();
    }
    out.finish().unwrap()
}

#[test]
fn a_pack_holds_the_data_files_and_installs_them_manifest_last() {
    let src = scratch("src");
    fs::create_dir_all(src.join("sets")).unwrap();
    fs::write(src.join(MANIFEST), "{\"m\":1}").unwrap();
    fs::write(src.join("prices.json"), "{\"p\":1}").unwrap();
    fs::write(src.join("sets").join("BLB.json"), "{\"s\":1}").unwrap();
    // Not data: left out.
    fs::write(src.join("notes.txt"), "x").unwrap();
    let pack = src.join("data.pack.gz");
    assert_eq!(write_pack(&src, &pack).unwrap(), 3);

    let files = read_pack(fs::File::open(&pack).unwrap()).unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["prices.json", "sets/BLB.json", MANIFEST]);

    // Installed over older data: files replaced, a set no longer in the pack removed.
    let dest = scratch("dest");
    fs::create_dir_all(dest.join("sets")).unwrap();
    fs::write(dest.join("sets").join("OLD.json"), "{}").unwrap();
    fs::write(dest.join("prices.json"), "{\"p\":0}").unwrap();
    install_pack(&files, &dest).unwrap();
    assert_eq!(fs::read_to_string(dest.join("prices.json")).unwrap(), "{\"p\":1}");
    assert_eq!(fs::read_to_string(dest.join("sets").join("BLB.json")).unwrap(), "{\"s\":1}");
    assert_eq!(fs::read_to_string(dest.join(MANIFEST)).unwrap(), "{\"m\":1}");
    assert!(!dest.join("sets").join("OLD.json").exists());
    assert!(!dest.join("prices.incoming").exists());

    let _ = fs::remove_dir_all(src);
    let _ = fs::remove_dir_all(dest);
}

#[test]
fn a_damaged_or_hostile_pack_is_refused() {
    let manifest: (&str, &[u8]) = (MANIFEST, b"{}");
    assert!(read_pack(Cursor::new(gzip(&[manifest]))).is_ok());
    // No manifest.
    assert!(read_pack(Cursor::new(gzip(&[("prices.json", b"{}")]))).is_err());
    // Paths leaving the data directory.
    for bad in ["../evil.json", "/evil.json", "sets/../../evil.json", "C:\\evil.json", "sets//x.json", "sets/CON.json", "sets/nul.json"] {
        assert!(read_pack(Cursor::new(gzip(&[(bad, b"{}"), manifest]))).is_err(), "{bad}");
    }
    // Cut short: the gzip stream (or a record) ends early.
    let whole = gzip(&[("prices.json", b"0123456789"), manifest]);
    assert!(read_pack(Cursor::new(&whole[..whole.len() - 12])).is_err());
    // Not gzip at all.
    assert!(read_pack(Cursor::new(b"<html>Not Found</html>".to_vec())).is_err());
}
