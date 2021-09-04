use crate::ast::Call;
use crate::engine::Command;
use crate::engine::EvaluationContext;
use crate::BlockId;
use crate::SyntaxShape;
use crate::Value;
use crate::VarId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    pub long: String,
    pub short: Option<char>,
    pub arg: Option<SyntaxShape>,
    pub required: bool,
    pub desc: String,
    // For custom commands
    pub var_id: Option<VarId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionalArg {
    pub name: String,
    pub desc: String,
    pub shape: SyntaxShape,
    // For custom commands
    pub var_id: Option<VarId>,
}

#[derive(Clone, Debug)]
pub struct Signature {
    pub name: String,
    pub usage: String,
    pub extra_usage: String,
    pub required_positional: Vec<PositionalArg>,
    pub optional_positional: Vec<PositionalArg>,
    pub rest_positional: Option<PositionalArg>,
    pub named: Vec<Flag>,
    pub is_filter: bool,
}

impl PartialEq for Signature {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.usage == other.usage
            && self.required_positional == other.required_positional
            && self.optional_positional == other.optional_positional
            && self.rest_positional == other.rest_positional
            && self.is_filter == other.is_filter
    }
}

impl Eq for Signature {}

impl Signature {
    pub fn new(name: impl Into<String>) -> Signature {
        Signature {
            name: name.into(),
            usage: String::new(),
            extra_usage: String::new(),
            required_positional: vec![],
            optional_positional: vec![],
            rest_positional: None,
            named: vec![],
            is_filter: false,
        }
    }
    pub fn build(name: impl Into<String>) -> Signature {
        Signature::new(name.into())
    }

    /// Add a description to the signature
    pub fn desc(mut self, usage: impl Into<String>) -> Signature {
        self.usage = usage.into();
        self
    }

    /// Add a required positional argument to the signature
    pub fn required(
        mut self,
        name: impl Into<String>,
        shape: impl Into<SyntaxShape>,
        desc: impl Into<String>,
    ) -> Signature {
        self.required_positional.push(PositionalArg {
            name: name.into(),
            desc: desc.into(),
            shape: shape.into(),
            var_id: None,
        });

        self
    }

    /// Add a required positional argument to the signature
    pub fn optional(
        mut self,
        name: impl Into<String>,
        shape: impl Into<SyntaxShape>,
        desc: impl Into<String>,
    ) -> Signature {
        self.optional_positional.push(PositionalArg {
            name: name.into(),
            desc: desc.into(),
            shape: shape.into(),
            var_id: None,
        });

        self
    }

    pub fn rest(mut self, shape: impl Into<SyntaxShape>, desc: impl Into<String>) -> Signature {
        self.rest_positional = Some(PositionalArg {
            name: "rest".into(),
            desc: desc.into(),
            shape: shape.into(),
            var_id: None,
        });

        self
    }

    /// Add an optional named flag argument to the signature
    pub fn named(
        mut self,
        name: impl Into<String>,
        shape: impl Into<SyntaxShape>,
        desc: impl Into<String>,
        short: Option<char>,
    ) -> Signature {
        let (name, s) = self.check_names(name, short);

        self.named.push(Flag {
            long: name,
            short: s,
            arg: Some(shape.into()),
            required: false,
            desc: desc.into(),
            var_id: None,
        });

        self
    }

    /// Add a required named flag argument to the signature
    pub fn required_named(
        mut self,
        name: impl Into<String>,
        shape: impl Into<SyntaxShape>,
        desc: impl Into<String>,
        short: Option<char>,
    ) -> Signature {
        let (name, s) = self.check_names(name, short);

        self.named.push(Flag {
            long: name,
            short: s,
            arg: Some(shape.into()),
            required: true,
            desc: desc.into(),
            var_id: None,
        });

        self
    }

    /// Add a switch to the signature
    pub fn switch(
        mut self,
        name: impl Into<String>,
        desc: impl Into<String>,
        short: Option<char>,
    ) -> Signature {
        let (name, s) = self.check_names(name, short);

        self.named.push(Flag {
            long: name,
            short: s,
            arg: None,
            required: false,
            desc: desc.into(),
            var_id: None,
        });

        self
    }

    /// Get list of the short-hand flags
    pub fn get_shorts(&self) -> Vec<char> {
        self.named.iter().filter_map(|f| f.short).collect()
    }

    /// Get list of the long-hand flags
    pub fn get_names(&self) -> Vec<&str> {
        self.named.iter().map(|f| f.long.as_str()).collect()
    }

    /// Checks if short or long are already present
    /// Panics if one of them is found
    fn check_names(&self, name: impl Into<String>, short: Option<char>) -> (String, Option<char>) {
        let s = short.map(|c| {
            debug_assert!(
                !self.get_shorts().contains(&c),
                "There may be duplicate short flags, such as -h"
            );
            c
        });

        let name = {
            let name: String = name.into();
            debug_assert!(
                !self.get_names().contains(&name.as_str()),
                "There may be duplicate name flags, such as --help"
            );
            name
        };

        (name, s)
    }

    pub fn get_positional(&self, position: usize) -> Option<PositionalArg> {
        if position < self.required_positional.len() {
            self.required_positional.get(position).cloned()
        } else if position < (self.required_positional.len() + self.optional_positional.len()) {
            self.optional_positional
                .get(position - self.required_positional.len())
                .cloned()
        } else {
            self.rest_positional.clone()
        }
    }

    pub fn num_positionals(&self) -> usize {
        let mut total = self.required_positional.len() + self.optional_positional.len();

        for positional in &self.required_positional {
            if let SyntaxShape::Keyword(..) = positional.shape {
                // Keywords have a required argument, so account for that
                total += 1;
            }
        }
        for positional in &self.optional_positional {
            if let SyntaxShape::Keyword(..) = positional.shape {
                // Keywords have a required argument, so account for that
                total += 1;
            }
        }
        total
    }

    pub fn num_positionals_after(&self, idx: usize) -> usize {
        let mut total = 0;

        for (curr, positional) in self.required_positional.iter().enumerate() {
            match positional.shape {
                SyntaxShape::Keyword(..) => {
                    // Keywords have a required argument, so account for that
                    if curr > idx {
                        total += 2;
                    }
                }
                _ => {
                    if curr > idx {
                        total += 1;
                    }
                }
            }
        }
        total
    }

    /// Find the matching long flag
    pub fn get_long_flag(&self, name: &str) -> Option<Flag> {
        for flag in &self.named {
            if flag.long == name {
                return Some(flag.clone());
            }
        }
        None
    }

    /// Find the matching long flag
    pub fn get_short_flag(&self, short: char) -> Option<Flag> {
        for flag in &self.named {
            if let Some(short_flag) = &flag.short {
                if *short_flag == short {
                    return Some(flag.clone());
                }
            }
        }
        None
    }

    /// Set the filter flag for the signature
    pub fn filter(mut self) -> Signature {
        self.is_filter = true;
        self
    }

    /// Create a placeholder implementation of Command as a way to predeclare a definition's
    /// signature so other definitions can see it. This placeholder is later replaced with the
    /// full definition in a second pass of the parser.
    pub fn predeclare(self) -> Box<dyn Command> {
        Box::new(Predeclaration { signature: self })
    }

    /// Combines a signature and a block into a runnable block
    pub fn into_block_command(self, block_id: BlockId) -> Box<dyn Command> {
        Box::new(BlockCommand {
            signature: self,
            block_id,
        })
    }
}

struct Predeclaration {
    signature: Signature,
}

impl Command for Predeclaration {
    fn name(&self) -> &str {
        &self.signature.name
    }

    fn signature(&self) -> Signature {
        self.signature.clone()
    }

    fn usage(&self) -> &str {
        &self.signature.usage
    }

    fn run(
        &self,
        _context: &EvaluationContext,
        _call: &Call,
        _input: Value,
    ) -> Result<crate::Value, crate::ShellError> {
        panic!("Internal error: can't run a predeclaration without a body")
    }
}

struct BlockCommand {
    signature: Signature,
    block_id: BlockId,
}

impl Command for BlockCommand {
    fn name(&self) -> &str {
        &self.signature.name
    }

    fn signature(&self) -> Signature {
        self.signature.clone()
    }

    fn usage(&self) -> &str {
        &self.signature.usage
    }

    fn run(
        &self,
        _context: &EvaluationContext,
        _call: &Call,
        _input: Value,
    ) -> Result<crate::Value, crate::ShellError> {
        panic!("Internal error: can't run custom command with 'run', use block_id");
    }

    fn get_custom_command(&self) -> Option<BlockId> {
        Some(self.block_id)
    }
}
