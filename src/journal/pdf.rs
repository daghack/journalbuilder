//! Writing pages into one PDF, each page an image.
//!
//! Pages are written as they come, so only one page's image is held at a
//! time. Opaque images (pages on paper) are stored as JPEG; others as
//! compressed RGB with their alpha as a soft mask, so they stay transparent.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use image::RgbaImage;
use std::fs::File;
use std::io::{self, BufWriter, Seek, Write};

/// JPEG quality for opaque pages.
const JPEG_QUALITY: u8 = 92;

pub struct PdfWriter {
    out: BufWriter<File>,
    /// Where each object starts in the file, by object number less one.
    offsets: Vec<u64>,
    /// The page objects.
    pages: Vec<u32>,
}

/// The catalog, the page tree and the document info are objects 1-3; pages'
/// objects follow.
const CATALOG: u32 = 1;
const PAGE_TREE: u32 = 2;
const INFO: u32 = 3;

impl PdfWriter {
    pub fn create(path: &str) -> io::Result<Self> {
        let mut out = BufWriter::new(File::create(path)?);
        // The binary comment marks the file as binary for tools that check.
        out.write_all(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")?;
        Ok(PdfWriter { out, offsets: vec![0; 3], pages: Vec::new() })
    }

    fn start_object(&mut self, number: u32) -> io::Result<()> {
        let at = self.out.stream_position()?;
        let index = number as usize - 1;
        if self.offsets.len() <= index {
            self.offsets.resize(index + 1, 0);
        }
        self.offsets[index] = at;
        writeln!(self.out, "{} 0 obj", number)
    }

    fn next_object(&self) -> u32 {
        self.offsets.len() as u32 + 1
    }

    /// An object holding a stream of `data`, its dictionary `dict` less the
    /// length.
    fn stream(&mut self, number: u32, dict: &str, data: &[u8]) -> io::Result<()> {
        self.start_object(number)?;
        write!(self.out, "<< {} /Length {} >>\nstream\n", dict, data.len())?;
        self.out.write_all(data)?;
        self.out.write_all(b"\nendstream\nendobj\n")
    }

    fn object(&mut self, number: u32, body: &str) -> io::Result<()> {
        self.start_object(number)?;
        write!(self.out, "{}\nendobj\n", body)
    }

    /// Add a page showing `image`, `width` by `height` points.
    pub fn add_page(&mut self, image: &RgbaImage, width: f32, height: f32) -> io::Result<()> {
        let (w, h) = image.dimensions();
        let page = self.next_object();
        let (contents, picture) = (page + 1, page + 2);
        // Reserve the numbers before writing, in case a soft mask follows.
        self.offsets.resize(picture as usize, 0);
        let opaque = image.pixels().all(|p| p.0[3] == 255);
        let mask = if opaque { None } else { Some(picture + 1) };
        let resources = format!("<< /XObject << /Im0 {} 0 R >> >>", picture);
        self.object(page, &format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources {} /Contents {} 0 R >>",
            PAGE_TREE, width, height, resources, contents))?;
        let draw = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", width, height);
        self.stream(contents, "", draw.as_bytes())?;
        let image_dict = format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8", w, h);
        if opaque {
            let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
                .encode_image(&rgb)
                .map_err(io::Error::other)?;
            self.stream(picture, &format!("{} /Filter /DCTDecode", image_dict), &jpeg)?;
        } else {
            let mask = mask.expect("not opaque");
            let rgb: Vec<u8> = image.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
            let alpha: Vec<u8> = image.pixels().map(|p| p.0[3]).collect();
            self.stream(picture, &format!("{} /Filter /FlateDecode /SMask {} 0 R", image_dict, mask), &deflate(&rgb)?)?;
            self.stream(mask, &format!("/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode", w, h), &deflate(&alpha)?)?;
        }
        self.pages.push(page);
        Ok(())
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Write the page tree, catalog and cross-reference table, titling the
    /// document `title`.
    pub fn finish(mut self, title: &str) -> io::Result<()> {
        let kids: Vec<String> = self.pages.iter().map(|p| format!("{} 0 R", p)).collect();
        self.object(PAGE_TREE, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), self.pages.len()))?;
        self.object(CATALOG, &format!("<< /Type /Catalog /Pages {} 0 R >>", PAGE_TREE))?;
        self.object(INFO, &format!("<< /Title {} /Producer (journalbuild) >>", pdf_string(title)))?;
        let xref = self.out.stream_position()?;
        write!(self.out, "xref\n0 {}\n0000000000 65535 f \n", self.offsets.len() + 1)?;
        for offset in &self.offsets {
            writeln!(self.out, "{:010} 00000 n ", offset)?;
        }
        write!(self.out, "trailer\n<< /Size {} /Root {} 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
            self.offsets.len() + 1, CATALOG, INFO, xref)?;
        self.out.flush()
    }
}

fn deflate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    encoder.finish()
}

/// A PDF literal string, with its specials escaped and anything outside
/// printable ASCII left out.
fn pdf_string(text: &str) -> String {
    let escaped: String = text.chars().filter(|c| (' '..='~').contains(c)).flat_map(|c| match c {
        '(' | ')' | '\\' => vec!['\\', c],
        c => vec![c],
    }).collect();
    format!("({})", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn pages_and_cross_references_are_well_formed() {
        let dir = std::env::temp_dir().join(format!("journal-pdf-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.pdf");
        let mut pdf = PdfWriter::create(path.to_str().unwrap()).unwrap();
        // An opaque page and a transparent one.
        pdf.add_page(&RgbaImage::from_pixel(8, 6, Rgba([200, 180, 140, 255])), 40.0, 30.0).unwrap();
        let mut clear = RgbaImage::new(5, 5);
        clear.put_pixel(2, 2, Rgba([255, 0, 0, 255]));
        pdf.add_page(&clear, 25.0, 25.0).unwrap();
        assert_eq!(pdf.page_count(), 2);
        pdf.finish("A (test) journal").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 2") && text.contains("/DCTDecode") && text.contains("/SMask"));
        assert!(text.contains("/Title (A \\(test\\) journal)"));
        // Every cross-reference points at its object. (Offsets count bytes,
        // so the table is read from the bytes, not the lossy text.)
        let tail = &text[text.rfind("startxref\n").unwrap()..];
        let xref_at: usize = tail.lines().nth(1).unwrap().parse().unwrap();
        let table = String::from_utf8_lossy(&bytes[xref_at..]).into_owned();
        assert!(table.starts_with("xref\n0 "));
        for (number, line) in table.lines().skip(3).take_while(|l| l.ends_with(" n ")).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            let expected = format!("{} 0 obj", number + 1);
            assert!(bytes[offset..].starts_with(expected.as_bytes()), "object {} isn't at {}", number + 1, offset);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn titles_are_escaped() {
        assert_eq!(pdf_string("a\\b(c)é"), "(a\\\\b\\(c\\))");
    }
}
