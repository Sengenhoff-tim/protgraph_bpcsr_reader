use std::collections::HashMap;

use anyhow::{Context, Result};

/// Global accession → id mapping, ids in order of first appearance.
#[derive(Debug, Default)]
pub struct AccessionTable {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl AccessionTable {
    pub fn intern(&mut self, accession: String) -> Result<u32> {
        if let Some(&id) = self.ids.get(&accession) {
            return Ok(id);
        }
        let id = u32::try_from(self.names.len()).context("more than 2^32 accessions")?;
        self.names.push(accession.clone());
        self.ids.insert(accession, id);
        Ok(id)
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}
