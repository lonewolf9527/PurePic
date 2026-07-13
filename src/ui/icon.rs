use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IconPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IconSegment {
    pub start: IconPoint,
    pub end: IconPoint,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Icon {
    pub width: f32,
    pub height: f32,
    pub segments: Vec<IconSegment>,
}

impl Icon {
    pub fn load(path: &Path) -> io::Result<Self> {
        let source = fs::read_to_string(path)?;
        let mut segments = Vec::new();
        let mut remaining = source.as_str();
        while let Some(index) = remaining.find(" d=\"") {
            remaining = &remaining[index + 4..];
            let end = remaining.find('"').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "SVG path is missing a quote")
            })?;
            segments.extend(parse_path(&remaining[..end])?);
            remaining = &remaining[end + 1..];
        }
        if segments.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SVG contains no supported path segments",
            ));
        }
        Ok(Self {
            width: 24.0,
            height: 24.0,
            segments,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Token {
    Command(char),
    Number(f32),
}

fn parse_path(path: &str) -> io::Result<Vec<IconSegment>> {
    let tokens = tokenize(path)?;
    let mut segments = Vec::new();
    let mut index = 0;
    let mut current = IconPoint::default();
    let mut start = IconPoint::default();

    while index < tokens.len() {
        let Token::Command(command) = tokens[index] else {
            return invalid_path("expected an SVG command");
        };
        index += 1;
        match command {
            'M' => {
                current = read_point(&tokens, &mut index)?;
                start = current;
            }
            'L' => {
                let next = read_point(&tokens, &mut index)?;
                segments.push(IconSegment {
                    start: current,
                    end: next,
                });
                current = next;
            }
            'H' => {
                let x = read_number(&tokens, &mut index)?;
                let next = IconPoint { x, y: current.y };
                segments.push(IconSegment {
                    start: current,
                    end: next,
                });
                current = next;
            }
            'V' => {
                let y = read_number(&tokens, &mut index)?;
                let next = IconPoint { x: current.x, y };
                segments.push(IconSegment {
                    start: current,
                    end: next,
                });
                current = next;
            }
            'Z' => {
                segments.push(IconSegment {
                    start: current,
                    end: start,
                });
                current = start;
            }
            _ => return invalid_path("only M, L, H, V and Z SVG commands are supported"),
        }
    }
    Ok(segments)
}

fn tokenize(path: &str) -> io::Result<Vec<Token>> {
    let mut result = Vec::new();
    let mut number = String::new();
    let flush = |number: &mut String, result: &mut Vec<Token>| -> io::Result<()> {
        if !number.is_empty() {
            let value = number.parse::<f32>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid SVG path number")
            })?;
            result.push(Token::Number(value));
            number.clear();
        }
        Ok(())
    };

    for character in path.chars() {
        if character.is_ascii_alphabetic() {
            flush(&mut number, &mut result)?;
            result.push(Token::Command(character));
        } else if character.is_ascii_digit() || matches!(character, '-' | '+' | '.') {
            if matches!(character, '-' | '+') && !number.is_empty() {
                flush(&mut number, &mut result)?;
            }
            number.push(character);
        } else if character == ',' || character.is_whitespace() {
            flush(&mut number, &mut result)?;
        } else {
            return invalid_path("unsupported SVG path character");
        }
    }
    flush(&mut number, &mut result)?;
    Ok(result)
}

fn read_point(tokens: &[Token], index: &mut usize) -> io::Result<IconPoint> {
    Ok(IconPoint {
        x: read_number(tokens, index)?,
        y: read_number(tokens, index)?,
    })
}

fn read_number(tokens: &[Token], index: &mut usize) -> io::Result<f32> {
    let Some(Token::Number(value)) = tokens.get(*index).copied() else {
        return invalid_path("expected an SVG path number");
    };
    *index += 1;
    Ok(value)
}

fn invalid_path<T>(message: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_closed_line_paths() {
        let segments = parse_path("M 4 4 L 20 4 L 20 20 L 4 20 Z").unwrap();
        assert_eq!(segments.len(), 4);
        assert_eq!(segments[3].end, IconPoint { x: 4.0, y: 4.0 });
    }
}
