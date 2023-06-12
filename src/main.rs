use std::{collections::HashMap, error::Error, io};

mod pdf;

#[derive(Eq, Hash, PartialEq, Copy, Clone)]
enum PDFType {
  Unknown,
  ESPP,
  RSU,
}

// Function pointer definition must be wrapped in a struct to be recursive
struct StateFunction(fn(&mut Parser) -> Option<StateFunction>);

struct Section {
  name: String,
  body: Vec<(String, String)>,
}

pub struct Parser {
  lines: Vec<String>,
  data: Vec<Section>,
  pdf_type: PDFType,
  pos: usize,
  current_section: String,
  current_body: Vec<(String, String)>,
}

impl Parser {
  fn new(input: &str) -> Self {
    Self {
      lines: input.lines().map(|v| v.trim().to_string()).collect(),
      data: Vec::new(),
      pdf_type: PDFType::Unknown,
      pos: 0,
      current_section: "".into(),
      current_body: Vec::new(),
    }
  }

  fn parse(&mut self) {
    let mut state = Some(StateFunction(Parser::parse_section));
    while let Some(next_state) = state {
      state = next_state.0(self)
    }
  }

  fn next(&mut self) -> Option<String> {
    if self.pos >= self.lines.len() {
      None
    } else {
      let l = self.lines[self.pos].clone();
      self.pos += 1;
      Some(l)
    }
  }

  fn peek(&mut self) -> Option<String> {
    if self.pos >= self.lines.len() - 1 {
      None
    } else {
      let l = self.lines[self.pos].clone();
      Some(l)
    }
  }

  fn skip_empty_lines(&mut self) {
    while self.pos < self.lines.len() {
      if self.lines[self.pos] != "" {
        return;
      }
      self.pos += 1;
    }
  }

  fn parse_section(p: &mut Parser) -> Option<StateFunction> {
    while let Some(l) = p.next() {
      match l.as_str() {
        "EMPLOYEE STOCK PLAN RELEASE CONFIRMATION" => {
          p.pdf_type = PDFType::RSU;
          ()
        },
        "EMPLOYEE STOCK PLAN PURCHASE CONFIRMATION" => {
          p.pdf_type = PDFType::ESPP;
          ()
        },
        "Release Details" => (),
        "Registration:" => (),
        "Purchase Details" => (),
        "" => (),
        _ => {
          p.skip_empty_lines();
          p.current_section = l;
          return Some(StateFunction(Parser::parse_section_body));
        },
      };
    }

    None
  }

  fn parse_section_body(p: &mut Parser) -> Option<StateFunction> {
    while let Some(l) = p.next() {
      // println!("{} -> {:?}", l, p.peek());
      if l == "" {
        // This happens with some PDFs where we have a section but
        // there are gaps within the section.
        if let Some(p) = p.peek() {
          if p.contains("\t") {
            return Some(StateFunction(Parser::parse_section_body));
          }
        }

        p.data.push(Section {
          name: p.current_section.clone(),
          body: p.current_body.clone(),
        });
        p.current_section = "".into();
        p.current_body = vec![];
        return Some(StateFunction(Parser::parse_section));
      }

      let parts: Vec<String> = l.split("\t").map(|v| v.trim().to_string()).collect();
      // println!("{:#?}", parts);
      p.current_body.push((
        parts[0].clone(),
        parts
          .get(1)
          .map(|v| v.to_owned())
          .unwrap_or_else(|| "".to_owned()),
      ));
    }

    None
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let mut pdf_map: HashMap<PDFType, Vec<Parser>> = HashMap::new();

  for entry in glob::glob("./input/*.pdf").expect("failed to read glob pattern") {
    let path = match entry {
      Ok(path) => path,
      Err(e) => {
        println!("{:?}", e);
        continue;
      },
    };

    let bytes = std::fs::read(path)?;
    let out = pdf::extract(&bytes)?;
    // println!("{}", out);

    let mut parser = Parser::new(&out);
    parser.parse();

    let entry = pdf_map.entry(parser.pdf_type).or_default();
    entry.push(parser);
  }

  for (pdf_type, parsers) in pdf_map {
    match pdf_type {
      PDFType::RSU => {
        let mut wtr = csv::Writer::from_writer(io::stdout());

        wtr.write_record(&[
          "Award Date",
          "Release Date",
          "Shares Released",
          "Market Value Per Share",
          "Sale Price Per Share",
          "Market Value",
          "Shares Sold",
          "Shares Issued",
          "Total Sale Price",
          "Total Tax",
          "Fee",
          "Total Due Participant",
        ])?;

        for parser in parsers {
          let data = sections_to_map(parser.data);
          wtr.write_record(&[
            &data["Release Summary"]["Award Date"],
            &data["Release Summary"]["Release Date"],
            &data["Release Summary"]["Shares Released"],
            &data["Release Summary"]["Market Value Per Share"],
            &data["Release Summary"]["Sale Price Per Share"],
            &data["Calculation of Gain"]["Market Value"],
            &data["Stock Distribution"]["Shares Sold"],
            &data["Stock Distribution"]["Shares Issued"],
            &data["Cash Distribution"]["Total Sale Price"],
            &data["Cash Distribution"]["Total Tax"],
            &data["Cash Distribution"]["Fee"],
            &data["Cash Distribution"]["Total Due Participant"],
          ])?;
        }

        wtr.flush()?;
        println!();
      },
      PDFType::ESPP => {
        let mut wtr = csv::Writer::from_writer(io::stdout());

        wtr.write_record(&[
          "Grant Date",
          "Purchase Begin Date",
          "Purchase Date",
          "Shares Purchased",
          "Previous Carry Forward",
          "Current Contributions",
          "Total Contributions",
          "Total Price",
          "Amount Refunded",
          "Grant Date Market Value",
          "Purchase Value per Share",
          "Purchase Price per Share",
          "Total Value",
          "Taxable Gain",
        ])?;

        for parser in parsers {
          let data = sections_to_map(parser.data);
          wtr.write_record(&[
            &data["Purchase Summary"]["Grant Date"],
            &data["Purchase Summary"]["Purchase Begin Date"],
            &data["Purchase Summary"]["Purchase Date"],
            &data["Shares Purchased to Date in Current Offering"]["Shares Purchased"],
            &data["Contributions"]["Previous Carry Forward"],
            &data["Contributions"]["Current Contributions"],
            &data["Contributions"]["Total Contributions"],
            &data["Contributions"]["Total Price"],
            &data["Contributions"]["Amount Refunded"],
            &data["Calculation of Shares Purchased"]["Grant Date Market Value"],
            &data["Calculation of Shares Purchased"]["Purchase Value per Share"],
            &data["Calculation of Shares Purchased"]["Purchase Price per Share"],
            &data["Calculation of Gain"]["Total Value"],
            &data["Calculation of Gain"]["Taxable Gain"],
          ])?;
        }

        wtr.flush()?;
        println!();
      },
      _ => {
        eprintln!("unknown pdf type");
      },
    }
  }

  Ok(())
}

fn sections_to_map(sections: Vec<Section>) -> HashMap<String, HashMap<String, String>> {
  let mut map = HashMap::new();
  for section in sections {
    let entry: &mut HashMap<String, String> = map.entry(section.name).or_default();
    let mut body_iter = section.body.iter().peekable();
    while body_iter.peek().is_some() {
      let (name, value) = body_iter
        .next()
        .map(|(n, v)| (n.clone(), v.clone()))
        .unwrap();

      // This occurs when we have a continuation on a newline of some key/value.
      if value.is_empty() && body_iter.peek().is_some() {
        let (name2, value2) = body_iter
          .next()
          .map(|(n, v)| (n.clone(), v.clone()))
          .unwrap();
        // We don't care if the name starts with a parenthesis.
        if name2.starts_with("(") {
          entry.insert(name, value2);
        } else {
          entry.insert(format!("{} {}", name, name2), value2);
        }
      } else {
        entry.insert(name, value);
      }
    }
  }
  map
}
