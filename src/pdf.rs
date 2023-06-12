use std::error::Error;

use euclid::Transform2D;
use lopdf::Document;
use pdf_extract::{self, output_doc, ConvertToFmt, MediaBox, OutputDev, OutputError, Transform};

type ArtBox = (f64, f64, f64, f64);

struct PlainTextOutput<W: ConvertToFmt> {
  writer: W::Writer,
  last_end: f64,
  last_y: f64,
  first_char: bool,
  flip_ctm: Transform,
}

impl<W: ConvertToFmt> PlainTextOutput<W> {
  fn new(writer: W) -> PlainTextOutput<W> {
    PlainTextOutput {
      writer: writer.convert(),
      last_end: 100000.,
      first_char: false,
      last_y: 0.,
      flip_ctm: Transform::identity(),
    }
  }
}

// There are some structural hints that PDFs can use to signal word and line endings:
// however relying on these is not likely to be sufficient.
impl<W: pdf_extract::ConvertToFmt> OutputDev for PlainTextOutput<W> {
  fn begin_page(
    &mut self,
    _page_num: u32,
    media_box: &MediaBox,
    _: Option<ArtBox>,
  ) -> Result<(), OutputError> {
    self.flip_ctm = Transform2D::row_major(1., 0., 0., -1., 0., media_box.ury - media_box.lly);
    Ok(())
  }

  fn end_page(&mut self) -> Result<(), OutputError> {
    Ok(())
  }

  fn output_character(
    &mut self,
    trm: &Transform,
    width: f64,
    _spacing: f64,
    font_size: f64,
    char: &str,
  ) -> Result<(), OutputError> {
    let position = trm.post_transform(&self.flip_ctm);
    let transformed_font_size_vec = trm.transform_vector(euclid::vec2(font_size, font_size));
    // get the length of one sized of the square with the same area with a rectangle of size (x, y)
    let transformed_font_size = (transformed_font_size_vec.x * transformed_font_size_vec.y).sqrt();
    let (x, y) = (position.m31, position.m32);
    use std::fmt::Write;

    // println!("{}", char);
    if self.first_char {
      if (y - self.last_y).abs() > transformed_font_size * 1.5 {
        write!(self.writer, "\n")?;
      }

      // we've moved to the left and down
      if x < self.last_end && (y - self.last_y).abs() > transformed_font_size * 0.5 {
        write!(self.writer, "\n")?;
      }

      // we've moved to the next column
      if x > self.last_end && y < self.last_y {
        write!(self.writer, "\n")?;
      }

      // we've moved a good amount to the right
      if x > self.last_end + transformed_font_size * 0.1 {
        write!(self.writer, "\t")?;
      }
    }

    // let norm = unicode_normalization::UnicodeNormalization::nfkc(char);
    write!(self.writer, "{}", char)?;
    self.first_char = false;
    self.last_y = y;
    self.last_end = x + width * transformed_font_size;
    Ok(())
  }

  fn begin_word(&mut self) -> Result<(), OutputError> {
    self.first_char = true;
    Ok(())
  }

  fn end_word(&mut self) -> Result<(), OutputError> {
    Ok(())
  }

  fn end_line(&mut self) -> Result<(), OutputError> {
    // write!(self.file, "\n");
    Ok(())
  }
}

pub(crate) fn extract(bytes: &[u8]) -> Result<String, Box<dyn Error>> {
  let mut out = String::new();
  let mut output = PlainTextOutput::new(&mut out);
  let doc = Document::load_mem(&bytes)?;
  output_doc(&doc, &mut output)?;
  Ok(out)
}
