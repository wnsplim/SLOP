use std::collections::{HashMap, HashSet};

pub struct Alignment {
    rows: Vec<(String, Vec<u8>)>,
    sites: usize,
}

impl Alignment {
    pub fn read(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&text, path)
    }

    fn parse(text: &str, source: &str) -> Result<Self, String> {
        let text = text.trim_start_matches('\u{feff}').trim_start();
        let rows = if text.starts_with('>') {
            parse_fasta(text, source)?
        } else if text.to_ascii_lowercase().starts_with("#nexus") {
            parse_nexus(text, source)?
        } else {
            parse_phylip(text, source)?
        };
        validate(rows, source)
    }

    pub fn taxa(&self) -> Vec<String> {
        self.rows.iter().map(|(name, _)| name.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.sites
    }

    pub fn window(&self, width: usize, seed: u64, iteration: usize) -> String {
        let width = width.min(self.sites);
        let choices = self.sites - width + 1;
        let start =
            mix(seed ^ (iteration as u64).wrapping_mul(0x9e3779b97f4a7c15)) as usize % choices;
        let end = start + width;
        let name_width = self
            .rows
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);
        let body = self
            .rows
            .iter()
            .map(|(name, sequence)| {
                format!(
                    "{name:<name_width$}  {}",
                    String::from_utf8_lossy(&sequence[start..end])
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Alignment, sites {}-{end} of {}:\n{body}",
            start + 1,
            self.sites
        )
    }
}

fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn parse_fasta(text: &str, source: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut rows = Vec::new();
    for record in text.split('>').skip(1) {
        let (header, body) = record.split_once('\n').unwrap_or((record, ""));
        let name = header
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("{source}: FASTA record has an empty header"))?;
        let sequence = body
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        rows.push((name.to_string(), sequence));
    }
    Ok(rows)
}

fn parse_nexus(text: &str, source: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let lower = text.to_ascii_lowercase();
    let matrix = lower
        .find("matrix")
        .ok_or_else(|| format!("{source}: NEXUS file has no MATRIX block"))?
        + "matrix".len();
    let end = text[matrix..]
        .find(';')
        .map(|offset| matrix + offset)
        .ok_or_else(|| format!("{source}: unterminated NEXUS MATRIX block"))?;

    let mut rows: Vec<(String, Vec<u8>)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for line in text[matrix..end].lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let sequence: Vec<u8> = fields.flat_map(str::bytes).collect();
        if sequence.is_empty() {
            return Err(format!("{source}: taxon {name:?} has no sequence"));
        }
        if let Some(index) = positions.get(name).copied() {
            rows[index].1.extend(sequence);
        } else {
            positions.insert(name.to_string(), rows.len());
            rows.push((name.to_string(), sequence));
        }
    }
    Ok(rows)
}

fn parse_phylip(text: &str, source: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let (header, body) = text
        .split_once('\n')
        .ok_or_else(|| format!("{source}: expected PHYLIP header '<ntax> <nsite>'"))?;
    let mut dimensions = header.split_whitespace();
    let taxa: usize = dimensions
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("{source}: invalid PHYLIP taxon count"))?;
    let sites: usize = dimensions
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("{source}: invalid PHYLIP site count"))?;

    let mut rows = Vec::new();
    for line in body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(taxa)
    {
        let mut fields = line.split_whitespace();
        let name = fields
            .next()
            .ok_or_else(|| format!("{source}: malformed PHYLIP row"))?;
        let sequence: Vec<u8> = fields.flat_map(str::bytes).collect();
        rows.push((name.to_string(), sequence));
    }
    if rows.len() != taxa {
        return Err(format!(
            "{source}: PHYLIP header declares {taxa} taxa but {} rows were found",
            rows.len()
        ));
    }
    if rows.iter().any(|(_, sequence)| sequence.len() != sites) {
        return Err(format!(
            "{source}: PHYLIP header declares {sites} sites but row lengths differ"
        ));
    }
    Ok(rows)
}

fn validate(rows: Vec<(String, Vec<u8>)>, source: &str) -> Result<Alignment, String> {
    if rows.is_empty() {
        return Err(format!("{source}: no sequences found"));
    }
    let mut names = HashSet::new();
    for (name, _) in &rows {
        if !names.insert(name) {
            return Err(format!("{source}: repeated taxon name {name:?}"));
        }
        if name
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || b"(),:;[]'\"".contains(&byte))
        {
            return Err(format!(
                "{source}: taxon name {name:?} contains Newick punctuation or whitespace"
            ));
        }
    }

    let sites = rows[0].1.len();
    if sites == 0 {
        return Err(format!("{source}: sequences are empty"));
    }
    if let Some((name, sequence)) = rows.iter().find(|(_, sequence)| sequence.len() != sites) {
        return Err(format!(
            "{source}: taxon {name:?} has {} sites; expected {sites}",
            sequence.len()
        ));
    }
    Ok(Alignment { rows, sites })
}

#[cfg(test)]
mod tests {
    use super::Alignment;

    #[test]
    fn reads_fasta_and_window_is_reproducible() {
        let alignment = Alignment::parse(">A\nAACCGG\n>B note\nAATTGG\n", "test").unwrap();
        assert_eq!(alignment.taxa(), vec!["A", "B"]);
        assert_eq!(alignment.len(), 6);
        assert_eq!(alignment.window(3, 7, 2), alignment.window(3, 7, 2));
    }

    #[test]
    fn rejects_ragged_or_repeated_records() {
        assert!(Alignment::parse(">A\nAA\n>B\nAAA\n", "test").is_err());
        assert!(Alignment::parse(">A\nAA\n>A\nAA\n", "test").is_err());
    }

    #[test]
    fn reads_interleaved_nexus_matrix() {
        let text = "#NEXUS\nbegin data;\nmatrix\nA AA\nB AT\n\nA CC\nB CT\n;\nend;";
        let alignment = Alignment::parse(text, "test").unwrap();
        assert_eq!(alignment.len(), 4);
    }

    #[test]
    fn checks_phylip_dimensions() {
        assert!(Alignment::parse("2 3\nA AAA\nB AAT\n", "test").is_ok());
        assert!(Alignment::parse("2 4\nA AAA\nB AAT\n", "test").is_err());
    }
}
