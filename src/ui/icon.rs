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
    let mut command = None;
    let mut last_cubic_control = None;

    while index < tokens.len() {
        if let Some(Token::Command(next_command)) = tokens.get(index).copied() {
            command = Some(next_command);
            index += 1;
        }
        let Some(active_command) = command else {
            return invalid_path("expected an SVG command");
        };
        let relative = active_command.is_ascii_lowercase();
        match active_command {
            'M' | 'm' => {
                current = resolve_point(current, read_point(&tokens, &mut index)?, relative);
                start = current;
                command = Some(if relative { 'l' } else { 'L' });
                last_cubic_control = None;
            }
            'L' | 'l' => {
                let next = resolve_point(current, read_point(&tokens, &mut index)?, relative);
                append_line(&mut segments, &mut current, next);
                last_cubic_control = None;
            }
            'H' | 'h' => {
                let x = read_number(&tokens, &mut index)?;
                let next = IconPoint {
                    x: if relative { current.x + x } else { x },
                    y: current.y,
                };
                append_line(&mut segments, &mut current, next);
                last_cubic_control = None;
            }
            'V' | 'v' => {
                let y = read_number(&tokens, &mut index)?;
                let next = IconPoint {
                    x: current.x,
                    y: if relative { current.y + y } else { y },
                };
                append_line(&mut segments, &mut current, next);
                last_cubic_control = None;
            }
            'C' | 'c' => {
                let origin = current;
                let control_one = resolve_point(origin, read_point(&tokens, &mut index)?, relative);
                let control_two = resolve_point(origin, read_point(&tokens, &mut index)?, relative);
                let next = resolve_point(origin, read_point(&tokens, &mut index)?, relative);
                append_cubic(&mut segments, &mut current, control_one, control_two, next);
                last_cubic_control = Some(control_two);
            }
            'S' | 's' => {
                let origin = current;
                let control_one = last_cubic_control
                    .map(|previous| reflect_point(origin, previous))
                    .unwrap_or(origin);
                let control_two = resolve_point(origin, read_point(&tokens, &mut index)?, relative);
                let next = resolve_point(origin, read_point(&tokens, &mut index)?, relative);
                append_cubic(&mut segments, &mut current, control_one, control_two, next);
                last_cubic_control = Some(control_two);
            }
            'A' | 'a' => {
                for _ in 0..5 {
                    read_number(&tokens, &mut index)?;
                }
                let next = resolve_point(current, read_point(&tokens, &mut index)?, relative);
                append_line(&mut segments, &mut current, next);
                last_cubic_control = None;
            }
            'Z' | 'z' => {
                append_line(&mut segments, &mut current, start);
                command = None;
                last_cubic_control = None;
            }
            _ => return invalid_path("unsupported SVG path command"),
        }
    }
    Ok(segments)
}

fn resolve_point(origin: IconPoint, point: IconPoint, relative: bool) -> IconPoint {
    if relative {
        IconPoint {
            x: origin.x + point.x,
            y: origin.y + point.y,
        }
    } else {
        point
    }
}

fn reflect_point(origin: IconPoint, point: IconPoint) -> IconPoint {
    IconPoint {
        x: origin.x * 2.0 - point.x,
        y: origin.y * 2.0 - point.y,
    }
}

fn append_line(segments: &mut Vec<IconSegment>, current: &mut IconPoint, end: IconPoint) {
    segments.push(IconSegment {
        start: *current,
        end,
    });
    *current = end;
}

fn append_cubic(
    segments: &mut Vec<IconSegment>,
    current: &mut IconPoint,
    control_one: IconPoint,
    control_two: IconPoint,
    end: IconPoint,
) {
    const STEPS: usize = 8;
    let start = *current;
    let mut previous = start;
    for step in 1..=STEPS {
        let t = step as f32 / STEPS as f32;
        let inverse = 1.0 - t;
        let point = IconPoint {
            x: inverse.powi(3) * start.x
                + 3.0 * inverse.powi(2) * t * control_one.x
                + 3.0 * inverse * t.powi(2) * control_two.x
                + t.powi(3) * end.x,
            y: inverse.powi(3) * start.y
                + 3.0 * inverse.powi(2) * t * control_one.y
                + 3.0 * inverse * t.powi(2) * control_two.y
                + t.powi(3) * end.y,
        };
        segments.push(IconSegment {
            start: previous,
            end: point,
        });
        previous = point;
    }
    *current = end;
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

    #[test]
    fn parses_relative_curves_and_arcs() {
        let segments = parse_path("M 0 0 c 4 0 4 8 8 8 s 4 8 8 0 a 2 2 0 0 1 4 0 z").unwrap();
        assert!(segments.len() > 16);
        assert_eq!(segments.last().unwrap().end, IconPoint::default());
    }

    #[test]
    fn supplied_status_icons_load_successfully() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("Assets/icons");
        for name in ["actual-size.svg", "zoom-in.svg", "zoom-out.svg"] {
            let icon = Icon::load(&directory.join(name)).unwrap();
            assert!(
                !icon.segments.is_empty(),
                "{name} should contain drawable paths"
            );
        }
    }
}
