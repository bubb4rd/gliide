//! What a call would cost at the labs' API list prices, for the agents that report tokens but no cost (Codex,
//! Antigravity). On a subscription it's a measure of use, not a bill.

use crate::Attempt;

/// When the prices below were read from the labs' lists (through LiteLLM's model catalog).
pub const PRICES_AS_OF: &str = "2026-11-03";

/// Tokens of each kind, as [`crate::Usage`] splits them (no kind counted twice).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    /// Every turn's tokens together (a repair turn costs too); kinds an agent didn't report count as none.
    pub fn of(attempts: &[Attempt]) -> Self {
        attempts.iter().fold(Self::default(), |t, a| {
            let u = &a.usage;
            Self {
                input: t.input + u.input_tokens.unwrap_or(0),
                output: t.output + u.output_tokens.unwrap_or(0),
                cache_write: t.cache_write + u.cache_creation_input_tokens.unwrap_or(0),
                cache_read: t.cache_read + u.cache_read_input_tokens.unwrap_or(0),
            }
        })
    }
}

/// What the agent said the turns cost, when it said so for every one of them (Claude Code does).
pub fn reported_cost(attempts: &[Attempt]) -> Option<f64> {
    attempts.iter().map(|a| a.usage.cost_usd).sum()
}

/// A model's list price: US dollars per million tokens of each kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

impl Price {
    /// What `t` costs at this price, by kind: input, output, cache write, cache read.
    pub fn cost(&self, t: &Tokens) -> [f64; 4] {
        let at = |n: u64, per_million: f64| n as f64 * per_million / 1_000_000.0;
        [
            at(t.input, self.input),
            at(t.output, self.output),
            at(t.cache_write, self.cache_write),
            at(t.cache_read, self.cache_read),
        ]
    }
}

/// Whose model it is: what its cache costs against fresh input.
#[derive(Clone, Copy)]
enum Lab {
    /// Cache writes at 1.25× input (five-minute cache), reads at 0.1×.
    Anthropic,
    /// No charge for a cache write; reads at 0.1×.
    OpenAi,
    /// Implicit caching: no write charge; reads at 0.1×.
    Google,
}

/// Model id (dots and underscores as dashes, no date, effort or `[1m]` suffix) → lab, input and output per million.
const PRICES: &[(&str, Lab, f64, f64)] = &[
    ("claude-opus-5-5", Lab::Anthropic, 4.0, 20.0),
    ("claude-opus-5", Lab::Anthropic, 5.0, 25.0),
    ("claude-opus-4-8", Lab::Anthropic, 5.0, 25.0),
    ("claude-opus-4-7", Lab::Anthropic, 5.0, 25.0),
    ("claude-opus-4-6", Lab::Anthropic, 5.0, 25.0),
    ("claude-opus-4-5", Lab::Anthropic, 5.0, 25.0),
    ("claude-sonnet-5-5", Lab::Anthropic, 2.0, 10.0),
    ("claude-sonnet-5", Lab::Anthropic, 2.0, 10.0),
    ("claude-sonnet-4-6", Lab::Anthropic, 3.0, 15.0),
    ("claude-sonnet-4-5", Lab::Anthropic, 3.0, 15.0),
    ("claude-haiku-5-5", Lab::Anthropic, 0.10, 0.50),
    ("claude-haiku-4-5", Lab::Anthropic, 1.0, 5.0),
    ("claude-fable-5-1", Lab::Anthropic, 10.0, 50.0),
    ("claude-fable-5", Lab::Anthropic, 10.0, 50.0),
    ("claude-mythos-5-1", Lab::Anthropic, 10.0, 50.0),
    ("claude-mythos-5", Lab::Anthropic, 10.0, 50.0),
    ("gpt-6-luna", Lab::OpenAi, 0.10, 0.50),
    ("gpt-6-sol", Lab::OpenAi, 2.0, 10.0),
    ("gpt-6-astra", Lab::OpenAi, 10.0, 50.0),
    ("gpt-6-1-sol", Lab::OpenAi, 2.0, 10.0),
    ("gpt-6-1-terra", Lab::OpenAi, 1.0, 5.0),
    ("gpt-5-6", Lab::OpenAi, 4.0, 20.0),
    ("gpt-5-6-luna", Lab::OpenAi, 0.20, 1.20),
    ("gpt-5-6-sol", Lab::OpenAi, 4.0, 20.0),
    ("gpt-5-6-terra", Lab::OpenAi, 2.0, 12.0),
    ("gpt-5-5", Lab::OpenAi, 5.0, 30.0),
    ("gpt-5-4", Lab::OpenAi, 2.50, 15.0),
    ("gpt-5-4-mini", Lab::OpenAi, 0.75, 4.50),
    ("gpt-5-4-nano", Lab::OpenAi, 0.20, 1.25),
    ("gpt-5-3-codex", Lab::OpenAi, 1.75, 14.0),
    ("gpt-5-2-codex", Lab::OpenAi, 1.75, 14.0),
    ("gpt-5-2", Lab::OpenAi, 1.75, 14.0),
    ("gpt-5-1-codex-max", Lab::OpenAi, 1.25, 10.0),
    ("gpt-5-1-codex-mini", Lab::OpenAi, 0.25, 2.0),
    ("gpt-5-1-codex", Lab::OpenAi, 1.25, 10.0),
    ("gpt-5-1", Lab::OpenAi, 1.25, 10.0),
    ("gpt-5-codex", Lab::OpenAi, 1.25, 10.0),
    ("gpt-5", Lab::OpenAi, 1.25, 10.0),
    ("gpt-5-mini", Lab::OpenAi, 0.25, 2.0),
    ("gemini-4-argon", Lab::Google, 2.0, 10.0),
    ("gemini-3-8-flash", Lab::Google, 0.75, 3.75),
    ("gemini-3-7-flash", Lab::Google, 0.75, 3.75),
    ("gemini-3-6-flash", Lab::Google, 0.75, 3.75),
    ("gemini-3-5-flash", Lab::Google, 1.50, 9.0),
    ("gemini-3-5-flash-lite", Lab::Google, 0.30, 2.50),
    ("gemini-3-1-flash-lite", Lab::Google, 0.25, 1.50),
    ("gemini-3-1-pro", Lab::Google, 2.0, 12.0),
    ("gemini-3-flash", Lab::Google, 0.50, 3.0),
    ("gemini-2-5-pro", Lab::Google, 1.25, 10.0),
    ("gemini-2-5-flash", Lab::Google, 0.30, 2.50),
];

/// Reasoning-effort suffixes the agents add to a model's id (`gemini-3.8-flash-low`, `claude-sonnet-5-5-high`).
const EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// `model`'s list price, from its id as an agent names it (`claude-sonnet-5-5-20261001`, `gpt-6-luna`,
/// `gemini-3.8-flash-low`); `None` for a model not on the list, or a bare alias (`sonnet`).
pub fn price(model: &str) -> Option<Price> {
    let id = family(model);
    let &(_, lab, input, output) = PRICES.iter().find(|(name, ..)| *name == id)?;
    let write = match lab {
        Lab::Anthropic => 1.25,
        Lab::OpenAi | Lab::Google => 1.0,
    };
    Some(Price {
        input,
        output,
        cache_write: input * write,
        cache_read: input * 0.1,
    })
}

/// `model` as [`PRICES`] keys it: lowercase, dots and underscores as dashes, without a `[…]` tag, a `-preview`, a
/// trailing date (`-20261001`) or an effort.
fn family(model: &str) -> String {
    let mut id = model.trim().to_lowercase();
    if let Some(at) = id.find('[') {
        id.truncate(at);
    }
    let mut id = id.replace(['.', '_'], "-");
    loop {
        let Some((head, tail)) = id.rsplit_once('-') else {
            return id;
        };
        let date = tail.len() == 8 && tail.bytes().all(|b| b.is_ascii_digit());
        if !(date || tail == "preview" || tail == "latest" || EFFORTS.contains(&tail)) {
            return id;
        }
        id = head.to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_as_the_agents_name_them_find_their_price() {
        let sonnet = price("claude-sonnet-5-5-20261001").unwrap();
        assert_eq!((sonnet.input, sonnet.output), (2.0, 10.0));
        assert_eq!(price("claude-sonnet-5-5-low"), Some(sonnet));
        assert_eq!(price("gemini-3.8-flash-low").unwrap().input, 0.75);
        assert_eq!(price("GPT-6-Luna").unwrap().output, 0.50);
        assert_eq!(price("claude-opus-5-5[1m]").unwrap().input, 4.0);
        // A family's sibling isn't its price.
        assert_eq!(price("gpt-5-4-mini").unwrap().input, 0.75);
    }

    #[test]
    fn aliases_and_unknown_models_have_no_price() {
        assert_eq!(price("sonnet"), None);
        assert_eq!(price("gpt-reserve"), None);
    }

    #[test]
    fn cache_reads_cost_a_tenth_and_anthropic_writes_a_quarter_more() {
        let t = Tokens {
            input: 1_000_000,
            output: 0,
            cache_write: 1_000_000,
            cache_read: 1_000_000,
        };
        assert_eq!(price("claude-sonnet-5-5").unwrap().cost(&t), [2.0, 0.0, 2.5, 0.2]);
        let luna = price("gpt-6-luna").unwrap().cost(&t);
        assert_eq!(luna[2], 0.10);
        assert!((luna[3] - 0.01).abs() < 1e-12);
    }
}
