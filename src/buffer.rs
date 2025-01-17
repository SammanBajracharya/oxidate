pub struct Buffer {
    pub file: Option<String>,
    pub lines: Vec<String>,
}

impl Buffer {
    pub fn from_file(file: Option<String>) -> Self {
        let lines = match &file {
            Some(file) => std::fs::read_to_string(file)
                .unwrap()
                .lines()
                .map(|s| s.to_string())
                .collect(),
            None => vec![String::new()],
        };

        Self { file, lines }
    }

    pub fn get(&self, line: usize) -> Option<String> {
        if self.lines.len() > line {
            return Some(self.lines[line].clone());
        }

        None
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn insert(&mut self, x: u16, y: u16, c: char) {
        if let Some(line) = self.lines.get_mut(y as usize) {
            (*line).insert(x as usize, c);
        }
    }

    pub fn delete(&mut self, x: u16, y: u16) {
        if let Some(line) = self.lines.get_mut(y as usize) {
            (*line).remove(x as usize);
        }
    }

    pub fn remove_line(&mut self, line: u16) {
        if self.len() > line as usize {
            self.lines.remove(line as usize);
        }
    }

    pub fn save(&self) -> std::io::Result<String> {
        if let Some(file) = &self.file {
            let contents = self.lines.join("\n");
            std::fs::write(file, &contents)?;
            let message = format!(
                "{:?} {}L, {}B written",
                file,
                self.lines.len(),
                contents.as_bytes().len()
            );
            Ok(message)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No file specified to save the buffer.",
            ))
        }
    }
}
