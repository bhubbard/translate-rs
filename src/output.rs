use crate::types::{OutputFormat, TranslationRecord};
use std::io::{self, Write};

pub struct OutputWriter {
    format: OutputFormat,
    json_started: bool,
    json_count: usize,
}

impl OutputWriter {
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            json_started: false,
            json_count: 0,
        }
    }

    pub fn write_record(&mut self, record: &TranslationRecord) -> io::Result<()> {
        let mut stdout = io::stdout().lock();
        match self.format {
            OutputFormat::Plain => {
                write!(stdout, "{}", record.dst)?;
            }
            OutputFormat::Ndjson => {
                writeln!(stdout, "{}", stable_json_record(record))?;
            }
            OutputFormat::Json => {
                if !self.json_started {
                    write!(stdout, "[")?;
                    self.json_started = true;
                }
                if self.json_count > 0 {
                    write!(stdout, ",")?;
                }
                write!(stdout, "{}", stable_json_record(record))?;
                self.json_count += 1;
            }
        }
        stdout.flush()
    }

    pub fn write_literal(&mut self, literal: &str) -> io::Result<()> {
        if self.format == OutputFormat::Plain {
            let mut stdout = io::stdout().lock();
            write!(stdout, "{literal}")?;
            stdout.flush()?;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.format == OutputFormat::Json {
            let mut stdout = io::stdout().lock();
            if self.json_started {
                writeln!(stdout, "]")?;
            } else {
                writeln!(stdout, "[]")?;
            }
            stdout.flush()?;
            self.json_started = false;
            self.json_count = 0;
        }
        Ok(())
    }
}

pub fn render_records(
    records: &[TranslationRecord],
    format: OutputFormat,
    plain_trailing_newline: bool,
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    match format {
        OutputFormat::Plain => {
            for record in records {
                write!(stdout, "{}", record.dst)?;
                if plain_trailing_newline {
                    writeln!(stdout)?;
                }
            }
        }
        OutputFormat::Ndjson => {
            for record in records {
                writeln!(stdout, "{}", stable_json_record(record))?;
            }
        }
        OutputFormat::Json => {
            if records.len() == 1 {
                writeln!(stdout, "{}", stable_json_record(&records[0]))?;
            } else {
                let body: Vec<String> = records.iter().map(stable_json_record).collect();
                writeln!(stdout, "[{}]", body.join(","))?;
            }
        }
    }
    stdout.flush()
}

pub fn stable_json_record(record: &TranslationRecord) -> String {
    format!(
        "{{\"from\":{},\"to\":{},\"src\":{},\"dst\":{},\"conf\":{}}}",
        escape_json_str(&record.from),
        escape_json_str(&record.to),
        escape_json_str(&record.src),
        escape_json_str(&record.dst),
        format_number(record.conf)
    )
}

pub fn escape_json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
}

pub fn format_number(val: f64) -> String {
    if !val.is_finite() {
        return "null".to_string();
    }
    let mut s = format!("{:.6}", val);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}
