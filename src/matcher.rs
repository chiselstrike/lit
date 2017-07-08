use regex::{self, Regex};

use std::collections::HashMap;
use std::{fmt, mem};

lazy_static! {
    static ref IDENTIFIER_REGEX: Regex = Regex::new("^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap();
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Matcher {
    components: Vec<Component>,
}

/// A component in a matcher.
#[derive(Clone,Debug,PartialEq,Eq)]
enum Component {
    Text(String),
    Variable(String),
    Regex(String),
    NamedRegex { name: String, regex: String },
}

impl Matcher {
    pub fn parse(s: &str) -> Self {
        let mut components: Vec<Component> = vec![];
        let mut chars = s.chars().peekable();

        let mut current_text = vec![];

        loop {
            let complete_text = |current_text: &mut Vec<char>, components: &mut Vec<Component>| {
                let text = mem::replace(current_text, Vec::new())
                    .into_iter().collect();
                components.push(Component::Text(text));
            };

            match (chars.next(), chars.peek().cloned()) {
                // Variable.
                (Some('$'), Some('$')) => {
                    chars.next(); // Eat second '$'.

                    let name: String = chars.clone()
                                            .take_while(|c| c.is_alphanumeric())
                                            .collect();
                    chars.nth(name.len() - 1); // Skip the variable name.
                    components.push(Component::Variable(name));
                },
                // Named or unnamed regex.
                (Some('['), Some('[')) => {
                    complete_text(&mut current_text, &mut components);
                    chars.next(); // Eat second `[`

                    let mut current_regex = vec![];
                    let mut bracket_level = 0i32;
                    loop {
                        match (chars.next(), chars.peek().cloned()) {
                            (Some(']'), Some(']')) if bracket_level == 0=> {
                                chars.next(); // Eat second `]`.
                                break;
                            },
                            (Some(c), _) => {
                                match c {
                                    '[' => bracket_level += 1,
                                    ']' => bracket_level -= 1,
                                    _ => (),
                                }

                                current_regex.push(c);
                            },
                            (None, _) => {
                                break;
                            },
                        }
                    }

                    let regex: String = current_regex.into_iter().collect();

                    let first_colon_idx = regex.chars().position(|c| c == ':');
                    let (name, regex): (Option<&str>, &str) = match first_colon_idx {
                        Some(first_colon_idx) => {
                            let substr = &regex[0..first_colon_idx];

                            if IDENTIFIER_REGEX.is_match(&substr) {
                                (Some(substr), &regex[first_colon_idx+1..])
                            } else {
                                (None, &regex)
                            }
                        },
                        None => (None, &regex),
                    };

                    match name {
                        Some(name) => components.push(Component::NamedRegex { name: name.to_owned(), regex: regex.to_owned() }),
                        None => components.push(Component::Regex(regex.to_owned())),
                    }

                },
                (Some(c), _) => {
                    current_text.push(c);
                },
                (None, _) => {
                    complete_text(&mut current_text, &mut components);
                    break;
                }
            }
        }

        Matcher { components: components }
    }
    pub fn resolve(&self, variables: &HashMap<String, String>) -> Regex {
        let regex_parts: Vec<_> = self.components.iter().map(|comp| match *comp {
            Component::Text(ref text) => regex::escape(text),
            Component::Variable(ref name) => {
                // FIXME: proper error handling.
                let value = variables.get(name).expect("no variable with that name");
                value.clone()
            },
            Component::Regex(ref regex) => regex.clone(),
            Component::NamedRegex { ref name, ref regex } => format!("(?P<{}>{})", name, regex),
        }).collect();
        Regex::new(&regex_parts.join("")).expect("generated invalid line match regex")
    }
}

impl fmt::Display for Matcher {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for component in self.components.iter() {
            match *component {
                Component::Text(ref text) => write!(fmt, "{}", text)?,
                Component::Variable(ref name) => write!(fmt, "$${}", name)?,
                Component::Regex(ref regex) => write!(fmt, "[[{}]]", regex)?,
                Component::NamedRegex { ref name, ref regex } => write!(fmt, "[[{}:{}]]", name, regex)?,
            }
        }

        Ok(())
    }
}

