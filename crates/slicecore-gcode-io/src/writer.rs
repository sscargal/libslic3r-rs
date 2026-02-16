//! G-code writer with dialect-aware output.
//!
//! The [`GcodeWriter`] formats [`GcodeCommand`] values into text and writes
//! them to any [`std::io::Write`] destination. It dispatches start and end
//! sequences to the appropriate dialect module.

use std::io::Write;

use crate::commands::GcodeCommand;
use crate::dialect::{EndConfig, GcodeDialect, StartConfig};
use crate::error::GcodeError;
use crate::marlin;

/// A dialect-aware G-code writer.
///
/// Wraps any [`Write`] destination and formats structured [`GcodeCommand`]
/// values as G-code text lines. Start and end sequences are generated
/// according to the selected [`GcodeDialect`].
pub struct GcodeWriter<W: Write> {
    writer: W,
    dialect: GcodeDialect,
    line_count: u32,
}

impl<W: Write> GcodeWriter<W> {
    /// Create a new writer targeting the given dialect.
    pub fn new(writer: W, dialect: GcodeDialect) -> Self {
        Self {
            writer,
            dialect,
            line_count: 0,
        }
    }

    /// Write a single G-code command.
    pub fn write_command(&mut self, cmd: &GcodeCommand) -> Result<(), GcodeError> {
        let line = cmd.to_string();
        writeln!(self.writer, "{line}")?;
        self.line_count += 1;
        Ok(())
    }

    /// Write multiple G-code commands in order.
    pub fn write_commands(&mut self, cmds: &[GcodeCommand]) -> Result<(), GcodeError> {
        for cmd in cmds {
            self.write_command(cmd)?;
        }
        Ok(())
    }

    /// Write dialect-specific start G-code.
    pub fn write_start_gcode(&mut self, config: &StartConfig) -> Result<(), GcodeError> {
        let cmds = match self.dialect {
            GcodeDialect::Marlin => marlin::start_gcode(config),
            // Other dialects will be wired in Task 2
            GcodeDialect::Klipper => marlin::start_gcode(config),
            GcodeDialect::RepRapFirmware => marlin::start_gcode(config),
            GcodeDialect::Bambu => marlin::start_gcode(config),
        };
        self.write_commands(&cmds)
    }

    /// Write dialect-specific end G-code.
    pub fn write_end_gcode(&mut self, config: &EndConfig) -> Result<(), GcodeError> {
        let cmds = match self.dialect {
            GcodeDialect::Marlin => marlin::end_gcode(config),
            // Other dialects will be wired in Task 2
            GcodeDialect::Klipper => marlin::end_gcode(config),
            GcodeDialect::RepRapFirmware => marlin::end_gcode(config),
            GcodeDialect::Bambu => marlin::end_gcode(config),
        };
        self.write_commands(&cmds)
    }

    /// Return the number of lines written so far.
    pub fn line_count(&self) -> u32 {
        self.line_count
    }

    /// Consume the writer and return the inner `Write` destination.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_single_command() {
        let buf = Vec::new();
        let mut writer = GcodeWriter::new(buf, GcodeDialect::Marlin);
        writer
            .write_command(&GcodeCommand::SetRelativeExtrusion)
            .unwrap();
        assert_eq!(writer.line_count(), 1);
        let output = String::from_utf8(writer.into_inner()).unwrap();
        assert_eq!(output, "M83\n");
    }

    #[test]
    fn write_multiple_commands() {
        let buf = Vec::new();
        let mut writer = GcodeWriter::new(buf, GcodeDialect::Marlin);
        let cmds = vec![
            GcodeCommand::Comment("test".to_string()),
            GcodeCommand::SetAbsolutePositioning,
            GcodeCommand::FanOff,
        ];
        writer.write_commands(&cmds).unwrap();
        assert_eq!(writer.line_count(), 3);
        let output = String::from_utf8(writer.into_inner()).unwrap();
        assert_eq!(output, "; test\nG90\nM107\n");
    }

    #[test]
    fn write_start_and_end_gcode() {
        let buf = Vec::new();
        let mut writer = GcodeWriter::new(buf, GcodeDialect::Marlin);
        writer
            .write_start_gcode(&StartConfig {
                bed_temp: 60.0,
                nozzle_temp: 200.0,
                bed_x: 220.0,
                bed_y: 220.0,
            })
            .unwrap();
        writer
            .write_command(&GcodeCommand::LinearMove {
                x: Some(100.0),
                y: Some(100.0),
                z: Some(0.3),
                e: Some(0.5),
                f: Some(1800.0),
            })
            .unwrap();
        writer
            .write_end_gcode(&EndConfig {
                retract_distance: 5.0,
            })
            .unwrap();
        let output = String::from_utf8(writer.into_inner()).unwrap();
        assert!(output.contains("M83"));
        assert!(output.contains("G28"));
        assert!(output.contains("G1 X100.000"));
        assert!(output.contains("M107"));
        assert!(output.contains("M84"));
    }

    #[test]
    fn into_inner_returns_buffer() {
        let buf: Vec<u8> = Vec::new();
        let writer = GcodeWriter::new(buf, GcodeDialect::Marlin);
        let inner = writer.into_inner();
        assert!(inner.is_empty());
    }
}
