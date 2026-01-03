use super::MetricsSnapshot;
use anyhow::Result;
use csv::Writer;
use std::fs::File;
use std::path::Path;

pub struct MetricsLogger {
    writer: Writer<File>,
}

impl MetricsLogger {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let writer = Writer::from_path(path)?;
        Ok(Self { writer })
    }
    
    pub fn log(&mut self, snapshot: &MetricsSnapshot) -> Result<()> {
        self.writer.serialize(snapshot)?;
        self.writer.flush()?;
        Ok(())
    }
    
    pub fn log_batch(&mut self, snapshots: &[MetricsSnapshot]) -> Result<()> {
        for snapshot in snapshots {
            self.writer.serialize(snapshot)?;
        }
        self.writer.flush()?;
        Ok(())
    }
}
