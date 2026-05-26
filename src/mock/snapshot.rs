//! Test-pattern BMP snapshot with OSD overlay.
//!
//! Generates a 640×360 24-bit BMP per request, with:
//!
//! * A time-varying RGB background plus a faint grid (so consecutive
//!   snapshots differ visibly — useful for confirming the snapshot
//!   loop is actually polling).
//! * Every `Text` OSD currently in `OsdState`, rendered at its
//!   `position_type` corner (or `(position_x, position_y)` when
//!   `position_type == "Custom"`) with the camera's `font_size`
//!   approximated by 1× / 2× / 3× pixel scaling.
//!
//! Format-string interpretation is deliberately small: the four
//! patterns the mock advertises in `GetOSDOptions` (`MM/dd/yyyy`,
//! `yyyy-MM-dd`, `dd.MM.yyyy`, `HH:mm:ss`, `hh:mm:ss tt`) plus a
//! safe fallback for anything else. Unknown format strings render as
//! literal text so the OSD still appears — just not formatted.

use crate::mock::font::{CHAR_H, CHAR_W, glyph};
use crate::mock::state::{OsdEntry, OsdTextEntry, SharedState};

const W: u32 = 640;
const H: u32 = 360;

/// Generate the full 24-bit BMP byte stream.
pub fn generate_test_bmp(state: &SharedState) -> Vec<u8> {
    let row_size = (W * 3 + 3) & !3;
    let pixel_data_size = row_size * H;
    let file_size = 54 + pixel_data_size;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let r = ((now * 37) % 180 + 40) as u8;
    let g = ((now * 73) % 180 + 40) as u8;
    let b = ((now * 113) % 180 + 40) as u8;

    // Top-down RGB framebuffer (row 0 = top). We BMP-encode bottom-up
    // at the end. Working top-down here makes the OSD position math
    // read like screen coordinates rather than upside-down ones.
    let mut pixels = vec![0u8; (W * H * 3) as usize];
    for y in 0..H {
        for x in 0..W {
            let is_grid = x % 80 == 0 || y % 80 == 0;
            let (rr, gg, bb) = if is_grid {
                (r / 3, g / 3, b / 3)
            } else {
                (r, g, b)
            };
            let i = ((y * W + x) * 3) as usize;
            pixels[i] = rr;
            pixels[i + 1] = gg;
            pixels[i + 2] = bb;
        }
    }

    // Render every Text OSD currently in state.
    let osds = state.read().osd.osds.clone();
    let now_secs = now as i64;
    for osd in &osds {
        if osd.osd_type != "Text" {
            continue;
        }
        let Some(text_entry) = osd.text.as_ref() else {
            continue;
        };
        let text = render_osd_text(text_entry, now_secs);
        if text.is_empty() {
            continue;
        }
        let scale = font_size_to_scale(text_entry.font_size);
        let (text_w, text_h) = text_extent(&text, scale);
        let (x, y) = position_to_pixels(osd, text_w, text_h);
        // Simple drop shadow / outline: black at offset, white on top —
        // keeps OSDs readable against the noisy varying background.
        draw_text(&mut pixels, x + 1, y + 1, &text, scale, (0, 0, 0));
        draw_text(&mut pixels, x, y, &text, scale, (255, 255, 255));
    }

    // ── BMP encode ───────────────────────────────────────────────────────
    let mut data = Vec::with_capacity(file_size as usize);

    // BMP file header (14 bytes)
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&file_size.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&54u32.to_le_bytes());

    // DIB header (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&W.to_le_bytes());
    data.extend_from_slice(&H.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&24u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&pixel_data_size.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes());
    data.extend_from_slice(&2835u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    // Pixel data — BMP is bottom-up, so iterate rows in reverse.
    let pad = (row_size - W * 3) as usize;
    for y in (0..H).rev() {
        for x in 0..W {
            let i = ((y * W + x) * 3) as usize;
            // BMP wants BGR, pixels[] is RGB.
            data.push(pixels[i + 2]);
            data.push(pixels[i + 1]);
            data.push(pixels[i]);
        }
        data.extend(std::iter::repeat_n(0u8, pad));
    }

    data
}

// ── OSD text rendering ──────────────────────────────────────────────────────

/// Format an OSD text entry against the current time.
fn render_osd_text(t: &OsdTextEntry, now_secs: i64) -> String {
    let (y, mo, d, h, mi, s) = unix_to_utc(now_secs);
    match t.text_type.as_str() {
        "Plain" => t.plain_text.clone().unwrap_or_default(),
        "Date" => format_date(t.date_format.as_deref().unwrap_or("yyyy-MM-dd"), y, mo, d),
        "Time" => format_time(t.time_format.as_deref().unwrap_or("HH:mm:ss"), h, mi, s),
        "DateAndTime" => {
            let date = format_date(t.date_format.as_deref().unwrap_or("yyyy-MM-dd"), y, mo, d);
            let time = format_time(t.time_format.as_deref().unwrap_or("HH:mm:ss"), h, mi, s);
            format!("{date} {time}")
        }
        _ => t.plain_text.clone().unwrap_or_default(),
    }
}

/// Map ONVIF font_size hints to render scale. Real cameras advertise
/// a continuous range; we bucket into three readable scales and let
/// the test cases pick one explicitly via the OSD's font_size.
fn font_size_to_scale(font_size: Option<u32>) -> u32 {
    match font_size.unwrap_or(20) {
        ..=14 => 1,
        15..=28 => 2,
        _ => 3,
    }
}

/// Pixel extent of `text` rendered at `scale`.
fn text_extent(text: &str, scale: u32) -> (u32, u32) {
    let n = text.chars().count() as u32;
    (n * CHAR_W as u32 * scale, CHAR_H as u32 * scale)
}

/// Translate OSD position into top-left text origin in pixel space.
/// `Custom` uses the OSD's normalised `(position_x, position_y)` in
/// `[-1.0, 1.0]` (ONVIF convention) — `(0, 0)` is centre, `(1, -1)`
/// is bottom-right.
fn position_to_pixels(osd: &OsdEntry, text_w: u32, text_h: u32) -> (i32, i32) {
    const PAD: i32 = 12;
    let right_x = (W as i32) - text_w as i32 - PAD;
    let bottom_y = (H as i32) - text_h as i32 - PAD;
    match osd.position_type.as_str() {
        "UpperLeft" => (PAD, PAD),
        "UpperRight" => (right_x, PAD),
        "LowerLeft" => (PAD, bottom_y),
        "LowerRight" => (right_x, bottom_y),
        "Custom" => {
            let nx = osd.position_x.unwrap_or(0.0).clamp(-1.0, 1.0);
            let ny = osd.position_y.unwrap_or(0.0).clamp(-1.0, 1.0);
            // Map (-1, 1) → (0, W) for X and (1, -1) → (0, H) for Y
            // (ONVIF: y up is positive, screen: y down is positive).
            let cx = ((nx + 1.0) * 0.5 * W as f32) as i32;
            let cy = ((1.0 - ny) * 0.5 * H as f32) as i32;
            (cx - text_w as i32 / 2, cy - text_h as i32 / 2)
        }
        _ => (PAD, PAD),
    }
}

/// Blit `text` into `pixels` at top-left `(x, y)`, scaled and tinted.
/// Out-of-bounds pixels are silently clipped.
fn draw_text(pixels: &mut [u8], x: i32, y: i32, text: &str, scale: u32, color: (u8, u8, u8)) {
    let mut cursor_x = x;
    for c in text.chars() {
        draw_glyph(pixels, cursor_x, y, c, scale, color);
        cursor_x += (CHAR_W as i32) * scale as i32;
    }
}

fn draw_glyph(pixels: &mut [u8], x: i32, y: i32, c: char, scale: u32, color: (u8, u8, u8)) {
    let g = glyph(c);
    for (row, byte) in g.iter().enumerate() {
        for col in 0..CHAR_W {
            // MSB = leftmost pixel.
            if (byte >> (7 - col)) & 1 == 0 {
                continue;
            }
            // Scale: each lit source pixel → scale×scale block in output.
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + (col as u32 * scale + dx) as i32;
                    let py = y + (row as u32 * scale + dy) as i32;
                    if px < 0 || py < 0 || px >= W as i32 || py >= H as i32 {
                        continue;
                    }
                    let idx = ((py as u32 * W + px as u32) * 3) as usize;
                    pixels[idx] = color.0;
                    pixels[idx + 1] = color.1;
                    pixels[idx + 2] = color.2;
                }
            }
        }
    }
}

// ── Format strings ──────────────────────────────────────────────────────────
//
// Just enough to handle the patterns advertised in `resp_osd_options`.
// Anything else falls through unchanged so the OSD still draws (just
// without substitution).

fn format_date(fmt: &str, y: i32, mo: u32, d: u32) -> String {
    fmt.replace("yyyy", &format!("{y:04}"))
        .replace("MM", &format!("{mo:02}"))
        .replace("dd", &format!("{d:02}"))
}

fn format_time(fmt: &str, h: u32, mi: u32, s: u32) -> String {
    let h12 = if h == 0 {
        12
    } else if h > 12 {
        h - 12
    } else {
        h
    };
    let am_pm = if h < 12 { "AM" } else { "PM" };
    fmt.replace("HH", &format!("{h:02}"))
        .replace("hh", &format!("{h12:02}"))
        .replace("mm", &format!("{mi:02}"))
        .replace("ss", &format!("{s:02}"))
        .replace("tt", am_pm)
}

// ── Time arithmetic ─────────────────────────────────────────────────────────
//
// Convert UNIX seconds to UTC (year, month, day, hour, minute, second)
// without pulling in chrono / time / jiff. Uses Howard Hinnant's
// `civil_from_days` algorithm — proleptic Gregorian, year range covers
// a few hundred millennia in either direction. Plenty for a mock.

/// Returns `(year, month, day, hour, minute, second)` in UTC.
fn unix_to_utc(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400) as u32;
    let (y, mo, d) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    (y, mo, d, h, mi, s)
}

/// Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms".
/// `days` = days since 1970-01-01 (negative ok). Returns
/// `(year, month [1..=12], day [1..=31])` in the proleptic Gregorian
/// calendar.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u32; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::state::{MockState, OsdState, OsdTextEntry};

    fn fresh_state() -> MockState {
        MockState::for_tests()
    }

    #[test]
    fn unix_to_utc_known_epochs() {
        // Epoch itself → 1970-01-01 00:00:00.
        assert_eq!(unix_to_utc(0), (1970, 1, 1, 0, 0, 0));
        // 2026-05-07 12:34:56 UTC = 1778157296.
        assert_eq!(unix_to_utc(1_778_157_296), (2026, 5, 7, 12, 34, 56));
        // Pre-epoch sanity: 1969-12-31 23:59:59.
        assert_eq!(unix_to_utc(-1), (1969, 12, 31, 23, 59, 59));
    }

    #[test]
    fn date_format_substitutes_known_tokens() {
        assert_eq!(format_date("yyyy-MM-dd", 2026, 5, 7), "2026-05-07");
        assert_eq!(format_date("MM/dd/yyyy", 2026, 5, 7), "05/07/2026");
        assert_eq!(format_date("dd.MM.yyyy", 2026, 5, 7), "07.05.2026");
    }

    #[test]
    fn time_format_handles_12_and_24_hour() {
        assert_eq!(format_time("HH:mm:ss", 14, 5, 9), "14:05:09");
        assert_eq!(format_time("hh:mm:ss tt", 14, 5, 9), "02:05:09 PM");
        assert_eq!(format_time("hh:mm:ss tt", 0, 30, 0), "12:30:00 AM");
        assert_eq!(format_time("hh:mm:ss tt", 12, 0, 0), "12:00:00 PM");
    }

    #[test]
    fn font_size_buckets() {
        assert_eq!(font_size_to_scale(None), 2);
        assert_eq!(font_size_to_scale(Some(10)), 1);
        assert_eq!(font_size_to_scale(Some(20)), 2);
        assert_eq!(font_size_to_scale(Some(48)), 3);
    }

    #[test]
    fn position_corners_pin_to_padding() {
        let osd = OsdEntry {
            token: "t".into(),
            video_source_config_token: "v".into(),
            osd_type: "Text".into(),
            position_type: "UpperLeft".into(),
            position_x: None,
            position_y: None,
            text: None,
            image_path: None,
        };
        assert_eq!(position_to_pixels(&osd, 100, 16), (12, 12));

        let mut o = osd.clone();
        o.position_type = "LowerRight".into();
        let (x, y) = position_to_pixels(&o, 100, 16);
        assert_eq!(x, W as i32 - 100 - 12);
        assert_eq!(y, H as i32 - 16 - 12);
    }

    #[test]
    fn position_custom_centres_when_zero_zero() {
        let osd = OsdEntry {
            token: "t".into(),
            video_source_config_token: "v".into(),
            osd_type: "Text".into(),
            position_type: "Custom".into(),
            position_x: Some(0.0),
            position_y: Some(0.0),
            text: None,
            image_path: None,
        };
        let (x, y) = position_to_pixels(&osd, 100, 16);
        // Centre minus half text dimensions.
        assert_eq!(x, W as i32 / 2 - 50);
        assert_eq!(y, H as i32 / 2 - 8);
    }

    #[test]
    fn generate_bmp_has_correct_header_and_size() {
        let s = fresh_state();
        let bmp = generate_test_bmp(&s);
        assert!(bmp.starts_with(b"BM"));
        let row_size = (W * 3 + 3) & !3;
        let expected = 54 + (row_size * H) as usize;
        assert_eq!(bmp.len(), expected);
        // Width / height encoded little-endian at offsets 18 / 22.
        assert_eq!(u32::from_le_bytes(bmp[18..22].try_into().unwrap()), W);
        assert_eq!(u32::from_le_bytes(bmp[22..26].try_into().unwrap()), H);
    }

    #[test]
    fn generate_bmp_with_no_osds_still_works() {
        // Wipe default OSDs — bg pattern should still render cleanly.
        let s = fresh_state();
        s.modify(|d| {
            d.osd = OsdState {
                osds: vec![],
                next_token_id: 1,
            }
        });
        let bmp = generate_test_bmp(&s);
        assert!(bmp.starts_with(b"BM"));
    }

    #[test]
    fn generate_bmp_renders_plain_text_pixels() {
        // A Plain OSD with white-on-bg "OK" should plant some white
        // pixels into the framebuffer area near UpperLeft. Easiest
        // verification: the BMP byte stream contains a run of 0xFF
        // RGB triples (the white outline) somewhere in the upper rows.
        let s = fresh_state();
        s.modify(|d| {
            d.osd = OsdState {
                osds: vec![OsdEntry {
                    token: "OSD_test".into(),
                    video_source_config_token: "VSC_1".into(),
                    osd_type: "Text".into(),
                    position_type: "UpperLeft".into(),
                    position_x: None,
                    position_y: None,
                    text: Some(OsdTextEntry {
                        text_type: "Plain".into(),
                        plain_text: Some("OK".into()),
                        date_format: None,
                        time_format: None,
                        font_size: Some(20),
                        font_color: None,
                    }),
                    image_path: None,
                }],
                next_token_id: 2,
            }
        });
        let bmp = generate_test_bmp(&s);
        // BMP body starts at 54. Look for at least one pure-white BGR
        // triple — proves draw_text painted something.
        let body = &bmp[54..];
        let has_white = body.windows(3).any(|w| w == [0xFF, 0xFF, 0xFF]);
        assert!(has_white, "expected text rendering to leave white pixels");
    }
}
