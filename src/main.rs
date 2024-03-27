use std::fs;

use baked_font::Glyph;
use image::{DynamicImage, EncodableLayout};
use skia_safe::{Canvas, Color, Color4f, ColorSpace, EncodedImageFormat, Font, FontMgr, FontStyle, Paint, Point, Size, Surface, surfaces};
use skia_safe::paint::Style;
use skia_safe::utils::text_utils::Align;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("Usage: {} (font family) (font size) (padding) (output file) (glyph text files...)", args[0]);
        std::process::exit(1);
    }
    
    let output = &args[4];
    
    let mut seq_list = Vec::new();
    for arg in args.iter().skip(5) {
        println!("Reading file: {}", arg);
        let data = fs::read_to_string(arg).unwrap();
        seq_list.extend(data.lines()
            .map(|x| x.to_string())
            .filter(|x| !x.is_empty()));
    }

    let family = &args[1];
    let size = args[2].parse::<f32>().unwrap();
    let padding = args[3].parse::<f32>().unwrap();
    println!("Font family: {}", family);
    println!("Font size: {}", size);
    println!("Padding: {}", padding);
    let (image, bc, bl) =
        bake_font(family, size, padding, seq_list);

    let width = image.width();

    let image = convert_image_image(&image).to_rgba8();
    let bitmap = image.as_bytes().iter().enumerate()
        .filter(|(i, _)| i % 4 == 3)
        .map(|(_, x)| *x)
        .collect::<Vec<_>>();

    let mut map16 = vec![Glyph {
        pos: (0, 0),
        size: (0, 0),
        offset: (0, 0),
    }; 65536];
    let mut dict32 = std::collections::BTreeMap::new();

    let flattened = bl.iter()
        .flat_map(|(y, line)| line.iter()
            .zip(std::iter::repeat(*y)))
        .zip(bc.iter())
        .map(|((x, y), c)|
            (c.seq.clone(),
             *x as u32, y as u32,
             c.size.width as u8, c.size.height as u8,
             -c.offset.x as i8, -c.offset.y as i8))
        .collect::<Vec<_>>();

    for (seq, x, y, w, h, ox, oy) in flattened {
        let glyph = Glyph {
            pos: (x, y),
            size: (w, h),
            offset: (-ox, size as i8 - oy),
        };
        let utf16 = seq.encode_utf16().collect::<Vec<_>>();
        if utf16.len() < 4 {
            map16[utf16[0] as usize] = glyph;
        } else {
            let key = [utf16[0], utf16[1]];
            dict32.insert(key, glyph);
        }
    }
    println!("Glyph count (map16): {}", map16.iter().filter(|x| x.size.0 != 0).count());
    println!("Glyph count (dict32): {}", dict32.len());

    let font = baked_font::Font {
        bitmap,
        width: width as u32,
        map16,
        dict32,
    };
    let data = postcard::to_allocvec(&font).unwrap();
    println!("Font size (uncompressed): {}", data.len());
    let data = zstd::encode_all(std::io::Cursor::new(data), 19).unwrap();
    println!("Font size (Zstd@19 compressed): {}", data.len());
    fs::write(output, data).unwrap();
}

fn create_raster_surface(width: i32, height: i32) -> Surface {
    surfaces::raster_n32_premul((width, height)).unwrap()
}

fn convert_image_image(image: &skia_safe::Image) -> DynamicImage {
    let data = image.encode(None, EncodedImageFormat::PNG, None).unwrap();
    let bytes = data.as_bytes();
    image::load(std::io::Cursor::new(bytes), image::ImageFormat::Png).unwrap()
}

fn bake_font(
    family: &str, size: f32, padding: f32, seq_list: Vec<String>
) -> (skia_safe::Image, Vec<BakingChar>, BakingLayout) {
    let font_mgr = FontMgr::default();
    let type_face = font_mgr
        .match_family_style(family, FontStyle::normal()).unwrap();
    let font = Font::from_typeface(&type_face, size);
    let mut paint = Paint::new(Color4f::from(Color::BLACK), &ColorSpace::new_srgb());
    paint.set_style(Style::Fill);

    let bc = build_chars(seq_list, &font, &paint, 0.0);
    let width = optimal_width(&bc);
    let layout = build_layout(&bc, width, padding);
    let height = {
        let last_y = layout.last().unwrap().0;
        let last_line_count = layout.last().unwrap().1.len();
        bc.iter()
            .rev()
            .take(last_line_count)
            .map(|x| x.size.height)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap() + last_y
    };

    let mut surface = create_raster_surface(width as i32, height as i32);
    let canvas = surface.canvas();
    render(&canvas, &bc, &layout, &font, &paint);
    
    let snapshot = surface.image_snapshot();
    (snapshot, bc, layout)
}

struct BakingChar {
    seq: String,
    offset: Point,
    size: Size
}

fn build_chars(base: Vec<String>, font: &Font, paint: &Paint, padding: f32) -> Vec<BakingChar> {
    base
        .into_iter()
        .map(|seq| {
            let (w, rect) = font.measure_str(&seq, Some(paint));
            BakingChar {
                seq,
                offset: Point::new(rect.x() - padding, rect.y() - padding),
                size: Size::new(w + padding * 2.0, rect.height() + padding * 2.0)
            }
        })
        .collect()
}

type BakingLayout = Vec<(f32, Vec<f32>)>;

fn optimal_width(chars: &[BakingChar]) -> f32 {
    let max = chars
        .into_iter()
        .map(|x| x.size.width)
        .reduce(|acc, x| acc.max(x)).unwrap();
    (max * (chars.len() as f32).sqrt()).ceil()
}

fn build_layout(chars: &[BakingChar], line_width: f32, padding: f32) -> BakingLayout {
    let mut layout = vec![(padding, Vec::new())];
    let mut max: f32 = 0.0;
    let mut off: f32 = padding;
    let mut line = Vec::new();
    for char in chars {
        if off + char.size.width + padding > line_width {
            layout.last_mut().unwrap().1 = line;
            layout.push((max + layout.last().unwrap().0, Vec::new()));
            max = 0.0;
            off = padding;
            line = Vec::new();
        }
        line.push(off);
        off += char.size.width + padding;
        max = max.max(char.size.height + padding);
    }
    if line.is_empty() {
        layout.pop().unwrap();
    } else {
        layout.last_mut().unwrap().1 = line;
    }
    layout
}

fn render(canvas: &Canvas, bc: &[BakingChar], bl: &BakingLayout, font: &Font, paint: &Paint) {
    let offsets = bl.iter()
        .flat_map(|(y, line)| line.iter()
            .map(|x| (*x, *y)));
    for (bc, (x, y)) in bc.iter().zip(offsets) {
        render_at(canvas, bc, x, y, font, paint);
    }
}

fn render_at(canvas: &Canvas, bc: &BakingChar, x: f32, y: f32, font: &Font, paint: &Paint) {
    canvas.draw_text_align(&bc.seq, Point::new(x, y) - bc.offset, font, paint, Align::Left);
}