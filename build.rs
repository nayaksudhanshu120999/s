use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    slint_build::compile("ui/palette.slint").expect("Failed to compile Slint UI definition");

    #[cfg(target_os = "windows")]
    {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
        let ico_path = out_dir.join("cmdpal_app.ico");
        if generate_ico_file(&ico_path).is_ok() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(ico_path.to_string_lossy().as_ref());
            let _ = res.compile();
        }
    }
}

#[cfg(target_os = "windows")]
fn generate_ico_file(path: & std::path::Path) -> std::io::Result<()> {
    const W: u32 = 32;
    const H: u32 = 32;
    let mut xor_mask = Vec::with_capacity((W * H * 4) as usize);

    // Bottom-up 32-bit BGRA pixel buffer for ICO format
    for y_inv in 0..H {
        let y = H - 1 - y_inv;
        for x in 0..W {
            let (r, g, b, a) = render_icon_pixel(x, y, W);
            xor_mask.push(b);
            xor_mask.push(g);
            xor_mask.push(r);
            xor_mask.push(a);
        }
    }

    let and_mask_row_bytes = ((W + 31) / 32) * 4;
    let and_mask = vec![0u8; (and_mask_row_bytes * H) as usize];
    let bmp_header_size = 40u32;
    let image_size = bmp_header_size + (xor_mask.len() as u32) + (and_mask.len() as u32);

    let mut file = File::create(path)?;

    // ICONDIR header (6 bytes)
    file.write_all(&0u16.to_le_bytes())?; // Reserved
    file.write_all(&1u16.to_le_bytes())?; // Type: 1 = Icon
    file.write_all(&1u16.to_le_bytes())?; // Count: 1 image

    // ICONDIRENTRY (16 bytes)
    file.write_all(&[W as u8, H as u8, 0, 0])?;
    file.write_all(&1u16.to_le_bytes())?; // Color planes
    file.write_all(&32u16.to_le_bytes())?; // Bits per pixel
    file.write_all(&image_size.to_le_bytes())?;
    file.write_all(&22u32.to_le_bytes())?; // Offset (6 + 16)

    // BITMAPINFOHEADER (40 bytes)
    file.write_all(&bmp_header_size.to_le_bytes())?;
    file.write_all(&(W as i32).to_le_bytes())?;
    file.write_all(&((H * 2) as i32).to_le_bytes())?; // Combined XOR + AND height
    file.write_all(&1u16.to_le_bytes())?; // Planes
    file.write_all(&32u16.to_le_bytes())?; // BitCount
    file.write_all(&0u32.to_le_bytes())?; // Compression (BI_RGB)
    file.write_all(&(xor_mask.len() as u32).to_le_bytes())?;
    file.write_all(&0i32.to_le_bytes())?;
    file.write_all(&0i32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&0u32.to_le_bytes())?;

    file.write_all(&xor_mask)?;
    file.write_all(&and_mask)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn render_icon_pixel(x: u32, y: u32, size: u32) -> (u8, u8, u8, u8) {
    let fx = x as f32 + 0.5;
    let fy = y as f32 + 0.5;
    let s = size as f32;
    let pad = s * 0.06;
    let radius = s * 0.22;

    let min_c = pad + radius;
    let max_c = s - pad - radius;
    let dx = if fx < min_c {
        min_c - fx
    } else if fx > max_c {
        fx - max_c
    } else {
        0.0
    };
    let dy = if fy < min_c {
        min_c - fy
    } else if fy > max_c {
        fy - max_c
    } else {
        0.0
    };
    let dist = (dx * dx + dy * dy).sqrt();
    if fx < pad || fx > s - pad || fy < pad || fy > s - pad || dist > radius {
        return (0, 0, 0, 0);
    }

    // Indigo-to-violet gradient background
    let t = (fx + fy) / (2.0 * s);
    let mut r = (79.0 * (1.0 - t) + 124.0 * t) as u8;
    let mut g = (70.0 * (1.0 - t) + 58.0 * t) as u8;
    let mut b = (229.0 * (1.0 - t) + 237.0 * t) as u8;

    // Subtle inner border highlight
    if dist > radius - 1.2
        || fx < pad + 1.2
        || fx > s - pad - 1.2
        || fy < pad + 1.2
        || fy > s - pad - 1.2
    {
        r = r.saturating_add(35);
        g = g.saturating_add(35);
        b = b.saturating_add(25);
    }

    // Crisp ">_" command prompt glyph in white
    let nx = fx / s;
    let ny = fy / s;

    let chevron_top = ny >= 0.30 && ny <= 0.52 && (nx - (0.26 + (ny - 0.30) * 0.95)).abs() < 0.065;
    let chevron_bot = ny > 0.52 && ny <= 0.74 && (nx - (0.47 - (ny - 0.52) * 0.95)).abs() < 0.065;
    let underscore = ny >= 0.66 && ny <= 0.75 && nx >= 0.52 && nx <= 0.76;

    if chevron_top || chevron_bot || underscore {
        return (255, 255, 255, 255);
    }

    (r, g, b, 255)
}
