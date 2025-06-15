use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::PrologError;

/// Represents a Prolog term using Serde JSON Value for flexibility.
/// More specific Rust types could be defined for stricter parsing if needed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)] // Allows direct deserialization into basic types or the map structure
pub enum PrologTerm {
    Atom(String), // Includes atoms, strings that aren't variables
    Variable(String),
    Integer(i64),
    Float(f64),
    Bool(bool), // Prolog true/false atoms are often represented as bools in JSON
    List(Vec<PrologTerm>),
    Compound(PrologCompound),
    // Add other specific types like Rational, etc. if needed
    // Fallback for any other valid JSON value
    Other(Value),
}

/// Represents a Prolog compound term (functor and arguments).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PrologCompound {
    pub functor: String,
    pub args: Vec<PrologTerm>,
}

/// Represents the result of a Prolog query.
pub type Solution = HashMap<String, PrologTerm>;

#[derive(Debug, Clone, PartialEq)]
pub enum QueryResult {
    /// Query succeeded with no variable bindings (e.g., `atom(a)`).
    Success(bool), // true for success, false for failure
    /// Query succeeded with one or more solutions (variable bindings).
    Solutions(Vec<Solution>),
}

impl QueryResult {
    /// Parses the solutions array from a `true([[...], [...]])` response.
    #[doc(hidden)]
    pub fn parse_solutions(solutions_json: &[Value]) -> Result<Self, PrologError> {
        Self::parse_solutions_internal(solutions_json)
    }
    
    fn parse_solutions_internal(solutions_json: &[Value]) -> Result<Self, PrologError> {
        let mut solutions = Vec::new();
        for solution_json in solutions_json {
            if let Some(assignments_json) = solution_json.as_array() {
                if assignments_json.is_empty() {
                    // An empty list within solutions indicates a success with no bindings,
                    // equivalent to QueryResult::Success(true), but we might be getting multiple.
                    // Let's represent it as an empty solution map for consistency within Solutions.
                    solutions.push(HashMap::new());
                } else {
                    let mut solution = HashMap::new();
                    for assignment_json in assignments_json {
                        // Expect structure like {"functor": "=", "args": ["VarName", <Value>]}
                        if let Some(functor) = assignment_json.get("functor").and_then(|f| f.as_str()) {
                            if functor == "=" {
                                if let Some(args) = assignment_json.get("args").and_then(|a| a.as_array()) {
                                    if args.len() == 2 {
                                        if let Some(var_name) = args[0].as_str() {
                                            let term_value = serde_json::from_value(args[1].clone())?;
                                            solution.insert(var_name.to_string(), term_value);
                                            continue; // Move to next assignment
                                        }
                                    }
                                }
                            }
                        }
                        // If parsing fails, return error
                        return Err(PrologError::InvalidState(format!(
                            "Unexpected assignment structure in solution: {}",
                            assignment_json
                        )));
                    }
                    solutions.push(solution);
                }
            } else {
                return Err(PrologError::InvalidState(format!(
                    "Expected an array for solution bindings, found: {}",
                    solution_json
                )));
            }
        }
        Ok(QueryResult::Solutions(solutions))
    }
}

// --- Helper functions for working with Prolog JSON (similar to Python's) ---

/// Checks if a JSON value represents a Prolog functor.
pub fn is_prolog_functor(json: &Value) -> bool {
    matches!(json, Value::Object(map) if map.contains_key("functor") && map.contains_key("args"))
}

/// Checks if a JSON value represents a Prolog list.
pub fn is_prolog_list(json: &Value) -> bool {
    matches!(json, Value::Array(_))
}

/// Checks if a JSON value represents a Prolog variable (starts with uppercase or _).
pub fn is_prolog_variable(json: &Value) -> bool {
    matches!(json, Value::String(s) if !s.is_empty() && (s.chars().next().unwrap().is_uppercase() || s.starts_with('_')))
}

/// Checks if a JSON value represents a Prolog atom (string that isn't a variable).
pub fn is_prolog_atom(json: &Value) -> bool {
    matches!(json, Value::String(s) if s.is_empty() || !(s.chars().next().unwrap().is_uppercase() || s.starts_with('_')))
}

/// Gets the name (functor, atom, or variable) from a Prolog JSON value.
pub fn prolog_name(json: &Value) -> Option<&str> {
    match json {
        Value::String(s) => Some(s),
        Value::Object(map) => map.get("functor").and_then(|v| v.as_str()),
        _ => None,
    }
}

/// Gets the arguments from a Prolog functor JSON value.
pub fn prolog_args(json: &Value) -> Option<&Vec<Value>> {
     match json {
        Value::Object(map) => map.get("args").and_then(|v| v.as_array()),
        _ => None,
    }
}

/// Converts a PrologTerm back into a Prolog-syntax string (best effort).
pub fn prolog_term_to_string(term: &PrologTerm) -> String {
    match term {
        PrologTerm::Atom(s) => quote_prolog_identifier(s),
        PrologTerm::Variable(s) => s.clone(),
        PrologTerm::Integer(i) => i.to_string(),
        PrologTerm::Float(f) => f.to_string(),
        PrologTerm::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        PrologTerm::List(items) => {
            let inner: Vec<String> = items.iter().map(prolog_term_to_string).collect();
            format!("[{}]", inner.join(", "))
        }
        PrologTerm::Compound(c) => {
             let inner: Vec<String> = c.args.iter().map(prolog_term_to_string).collect();
             format!("{}({})", quote_prolog_identifier(&c.functor), inner.join(", "))
        }
        PrologTerm::Other(v) => v.to_string(), // Fallback to JSON string representation
    }
}

/// Quotes a Prolog identifier (atom) if necessary.
fn quote_prolog_identifier(identifier: &str) -> String {
    if identifier.is_empty() {
        return "''".to_string();
    }
    let first_char = identifier.chars().next().unwrap();
    let needs_quote = !first_char.is_lowercase()
        || identifier.contains(|c: char| !(c.is_alphanumeric() || c == '_'))
        // Check for keywords or special atoms if necessary (e.g., '!', ';')
        || identifier == "true" || identifier == "false" || identifier == "fail" || identifier == "!";

    if needs_quote {
        // Basic escaping: replace internal quotes with double quotes
        format!("'{}'", identifier.replace('\'', "\'\'"))
    } else {
        identifier.to_string()
    }
} 