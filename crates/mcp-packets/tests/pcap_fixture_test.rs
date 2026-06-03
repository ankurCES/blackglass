mod fixtures;

#[test]
fn minimal_pcap_has_correct_magic_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixtures::minimal_pcap(dir.path());
    let bytes = std::fs::read(&path).unwrap();
    // 24-byte global header only
    assert_eq!(bytes.len(), 24);
    // little-endian magic 0xa1b2c3d4
    assert_eq!(&bytes[0..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
    // major version 2
    assert_eq!(&bytes[4..6], &[0x02, 0x00]);
    // minor version 4
    assert_eq!(&bytes[6..8], &[0x04, 0x00]);
}
