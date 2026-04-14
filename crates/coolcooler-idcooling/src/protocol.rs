/// USB packet size. All packets are exactly this size, zero-padded.
pub const PACKET_SIZE: usize = 1024;

/// Header size for DRA (draw) commands.
pub const HEADER_SIZE: usize = 32;

/// JPEG data capacity in the first packet (1024 - 32 = 992).
const JPEG_FIRST_CHUNK: usize = PACKET_SIZE - HEADER_SIZE;

/// Protocol magic bytes — all packets start with this.
const MAGIC: &[u8; 5] = b"CRT\x00\x00";

/// Draw image command.
const CMD_DRAW: &[u8; 4] = b"DRA\x00";

/// Connection keepalive command.
const CMD_CONNECT: &[u8; 7] = b"CONNECT";

/// Constant flag byte at header offset 12 (purpose unknown, always 0xB1 in captures).
const FLAG_BYTE: u8 = 0xB1;

/// Build the 32-byte DRA header for an image frame.
///
/// The frame size field (bytes 9-11) is a big-endian uint24
/// encoding `jpeg_size + HEADER_SIZE`.
pub fn build_draw_header(jpeg_size: usize) -> [u8; HEADER_SIZE] {
    let total_size = (jpeg_size + HEADER_SIZE) as u32;
    let be = total_size.to_be_bytes();

    let mut header = [0u8; HEADER_SIZE];
    header[0..5].copy_from_slice(MAGIC);
    header[5..9].copy_from_slice(CMD_DRAW);
    // uint24 big-endian: lower 3 bytes of the u32 BE representation
    header[9] = be[1];
    header[10] = be[2];
    header[11] = be[3];
    header[12] = FLAG_BYTE;

    header
}

/// Build a CONNECT keepalive packet (1024 bytes, zero-padded).
pub fn build_connect_packet() -> [u8; PACKET_SIZE] {
    let mut packet = [0u8; PACKET_SIZE];
    packet[0..5].copy_from_slice(MAGIC);
    packet[5..12].copy_from_slice(CMD_CONNECT);
    packet
}

/// Split JPEG data into protocol packets ready for USB transfer.
///
/// Returns a list of 1024-byte packets:
/// - First packet: 32-byte DRA header + up to 992 bytes of JPEG data
/// - Continuation packets: up to 1024 bytes of JPEG data (zero-padded if final)
pub fn build_frame_packets(jpeg_data: &[u8]) -> Vec<[u8; PACKET_SIZE]> {
    let header = build_draw_header(jpeg_data.len());
    let continuation_count = jpeg_data
        .len()
        .saturating_sub(JPEG_FIRST_CHUNK)
        .div_ceil(PACKET_SIZE);
    let mut packets = Vec::with_capacity(1 + continuation_count);

    // First packet: header + first chunk of JPEG
    let mut first = [0u8; PACKET_SIZE];
    first[..HEADER_SIZE].copy_from_slice(&header);
    let first_len = jpeg_data.len().min(JPEG_FIRST_CHUNK);
    first[HEADER_SIZE..HEADER_SIZE + first_len].copy_from_slice(&jpeg_data[..first_len]);
    packets.push(first);

    // Continuation packets
    let mut offset = JPEG_FIRST_CHUNK;
    while offset < jpeg_data.len() {
        let mut pkt = [0u8; PACKET_SIZE];
        let chunk_len = (jpeg_data.len() - offset).min(PACKET_SIZE);
        pkt[..chunk_len].copy_from_slice(&jpeg_data[offset..offset + chunk_len]);
        packets.push(pkt);
        offset += PACKET_SIZE;
    }

    packets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_header_magic_and_command() {
        let header = build_draw_header(1000);
        assert_eq!(&header[0..5], b"CRT\x00\x00");
        assert_eq!(&header[5..9], b"DRA\x00");
        assert_eq!(header[12], 0xB1);
    }

    #[test]
    fn draw_header_size_field() {
        // jpeg_size=1000 → total=1032=0x000408
        let header = build_draw_header(1000);
        let size = u32::from_be_bytes([0, header[9], header[10], header[11]]);
        assert_eq!(size, 1032);
    }

    #[test]
    fn draw_header_size_field_large() {
        // jpeg_size=50000 → total=50032=0x00C380
        let header = build_draw_header(50000);
        let size = u32::from_be_bytes([0, header[9], header[10], header[11]]);
        assert_eq!(size, 50032);
    }

    #[test]
    fn connect_packet_layout() {
        let pkt = build_connect_packet();
        assert_eq!(pkt.len(), PACKET_SIZE);
        assert_eq!(&pkt[0..5], b"CRT\x00\x00");
        assert_eq!(&pkt[5..12], b"CONNECT");
        assert!(pkt[12..].iter().all(|&b| b == 0));
    }

    #[test]
    fn frame_packets_small_jpeg() {
        // JPEG smaller than 992 bytes → single packet
        let jpeg = vec![0xFFu8; 500];
        let packets = build_frame_packets(&jpeg);
        assert_eq!(packets.len(), 1);
        assert_eq!(&packets[0][0..5], b"CRT\x00\x00");
        assert_eq!(packets[0][32], 0xFF);
        // Remaining bytes after JPEG data should be zero
        assert!(packets[0][32 + 500..].iter().all(|&b| b == 0));
    }

    #[test]
    fn frame_packets_exact_first_chunk() {
        // JPEG exactly 992 bytes → single packet
        let jpeg = vec![0x42u8; JPEG_FIRST_CHUNK];
        let packets = build_frame_packets(&jpeg);
        assert_eq!(packets.len(), 1);
    }

    #[test]
    fn frame_packets_one_byte_over() {
        // JPEG 993 bytes → first packet + 1 continuation
        let jpeg = vec![0x42u8; JPEG_FIRST_CHUNK + 1];
        let packets = build_frame_packets(&jpeg);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[1][0], 0x42);
        assert!(packets[1][1..].iter().all(|&b| b == 0));
    }

    #[test]
    fn frame_packets_large_jpeg() {
        // 2500 bytes → first (992) + continuation (1024) + continuation (484)
        let jpeg: Vec<u8> = (0..2500).map(|i| (i % 256) as u8).collect();
        let packets = build_frame_packets(&jpeg);
        assert_eq!(packets.len(), 3);

        // Verify first packet header
        assert_eq!(&packets[0][0..5], b"CRT\x00\x00");

        // Verify JPEG data continuity
        assert_eq!(packets[0][32], 0); // first byte of JPEG (0 % 256)
        assert_eq!(packets[1][0], jpeg[992]); // continuation starts where first left off
        assert_eq!(packets[2][0], jpeg[992 + 1024]); // second continuation
    }

    #[test]
    fn frame_packets_empty_jpeg() {
        let packets = build_frame_packets(&[]);
        assert_eq!(packets.len(), 1);
        // Header present, no JPEG data
        assert_eq!(&packets[0][0..5], b"CRT\x00\x00");
        let size = u32::from_be_bytes([0, packets[0][9], packets[0][10], packets[0][11]]);
        assert_eq!(size, HEADER_SIZE as u32);
    }
}
