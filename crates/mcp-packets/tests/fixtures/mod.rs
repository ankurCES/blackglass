/// Returns path to a minimal valid pcap file (global header only, 0 packets).
pub fn minimal_pcap(dir: &std::path::Path) -> std::path::PathBuf {
    let p = dir.join("min.pcap");
    // pcap global header: magic(LE), major=2, minor=4, thiszone=0, sigfigs=0, snaplen=65535, network=1(Ethernet)
    let hdr: [u8; 24] = [
        0xd4, 0xc3, 0xb2, 0xa1, // magic number (little-endian)
        0x02, 0x00,              // major version 2
        0x04, 0x00,              // minor version 4
        0x00, 0x00, 0x00, 0x00, // thiszone (GMT offset)
        0x00, 0x00, 0x00, 0x00, // sigfigs (accuracy of timestamps)
        0xff, 0xff, 0x00, 0x00, // snaplen 65535
        0x01, 0x00, 0x00, 0x00, // network: LINKTYPE_ETHERNET
    ];
    std::fs::write(&p, hdr).unwrap();
    p
}
