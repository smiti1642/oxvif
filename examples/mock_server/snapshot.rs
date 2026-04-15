//! Test-pattern BMP snapshot generation.

/// Generate a 160×90 24-bit BMP with a time-varying color and grid overlay.
pub fn generate_test_bmp() -> Vec<u8> {
    let w: u32 = 160;
    let h: u32 = 90;
    let row_size = (w * 3 + 3) & !3;
    let pixel_data_size = row_size * h;
    let file_size = 54 + pixel_data_size;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let r = ((now * 37) % 180 + 40) as u8;
    let g = ((now * 73) % 180 + 40) as u8;
    let b = ((now * 113) % 180 + 40) as u8;

    let mut data = Vec::with_capacity(file_size as usize);

    // BMP file header (14 bytes)
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&file_size.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&54u32.to_le_bytes());

    // DIB header (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&w.to_le_bytes());
    data.extend_from_slice(&h.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&24u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&pixel_data_size.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    // Pixel data (bottom-up)
    for y in 0..h {
        for x in 0..w {
            let is_grid = x % 20 == 0 || y % 20 == 0;
            if is_grid {
                data.push(b / 3);
                data.push(g / 3);
                data.push(r / 3);
            } else {
                data.push(b);
                data.push(g);
                data.push(r);
            }
        }
        let pad = (row_size - w * 3) as usize;
        data.extend(std::iter::repeat_n(0u8, pad));
    }

    data
}
