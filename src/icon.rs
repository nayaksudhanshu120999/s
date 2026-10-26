use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub fn create_slint_app_icon() -> Image {
    let size = 64u32;
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    let slice = buffer.make_mut_slice();

    for y in 0..size {
        for x in 0..size {
            let (r, g, b, a) = render_pixel(x, y, size);
            slice[(y * size + x) as usize] = Rgba8Pixel::new(r, g, b, a);
        }
    }

    Image::from_rgba8(buffer)
}

pub fn create_win32_tray_icon_buffers(size: u32) -> (Vec<u8>, Vec<u8>) {
    let mut bgra = Vec::with_capacity((size * size * 4) as usize);
    let and_mask = vec![0u8; ((size * size) / 8) as usize];

    for y in 0..size {
        for x in 0..size {
            let (r, g, b, a) = render_pixel(x, y, size);
            bgra.push(b);
            bgra.push(g);
            bgra.push(r);
            bgra.push(a);
        }
    }

    (and_mask, bgra)
}

fn render_pixel(x: u32, y: u32, size: u32) -> (u8, u8, u8, u8) {
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

    // Modern indigo-to-violet gradient
    let t = (fx + fy) / (2.0 * s);
    let mut r = (79.0 * (1.0 - t) + 124.0 * t) as u8;
    let mut g = (70.0 * (1.0 - t) + 58.0 * t) as u8;
    let mut b = (229.0 * (1.0 - t) + 237.0 * t) as u8;

    // Subtle border glow
    if dist > radius - 1.4
        || fx < pad + 1.4
        || fx > s - pad - 1.4
        || fy < pad + 1.4
        || fy > s - pad - 1.4
    {
        r = r.saturating_add(35);
        g = g.saturating_add(35);
        b = b.saturating_add(25);
    }

    // Command prompt ">_" glyph
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
