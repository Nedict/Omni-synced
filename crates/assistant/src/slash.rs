//! Quick actions: `/explain`, `/fix`, `/params` and `/check` are ready-made prompts that pull in
//! what is failing in the design and what is selected, so a person need not describe it. They work
//! with every backend: the text is sent as an ordinary message.
//!
//! Only these names are SolidCraft's. Any other `/word` is returned as typed: coding agents have
//! slash commands of their own and must get them unchanged.

/// What the composer's text means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// A message, to send as it is (unknown `/words` included).
    Message(String),
    /// A quick action: `shown` is what the person typed, `prompt` what is sent.
    Prompt { shown: String, prompt: String },
    /// `/help`: list the quick actions.
    Help,
}

/// A quick action, for the suggestions and the help text.
pub struct Quick {
    pub name: &'static str,
    /// The message id of its one-line summary (the UI's catalogs).
    pub summary: &'static str,
}

pub const QUICK: &[Quick] = &[
    Quick { name: "explain", summary: "ui.agent_chat_quick_explain" },
    Quick { name: "fix", summary: "ui.agent_chat_quick_fix" },
    Quick { name: "params", summary: "ui.agent_chat_quick_params" },
    Quick { name: "check", summary: "ui.agent_chat_quick_check" },
    Quick { name: "help", summary: "ui.agent_chat_quick_help" },
];

/// What the prompt templates may refer to.
#[derive(Clone, Debug, Default)]
pub struct Situation {
    /// Features with an error or a warning, one `name: message` each.
    pub failing: Vec<String>,
    /// How many things are selected.
    pub selected: usize,
}

/// Quick actions whose name starts with `prefix` (without the slash).
pub fn matching(prefix: &str) -> Vec<&'static Quick> {
    let p = prefix.to_ascii_lowercase();
    QUICK.iter().filter(|q| q.name.starts_with(&p)).collect()
}

/// The first `/word` of `input` if it is only that (a person still typing a quick action).
pub fn typing(input: &str) -> Option<&str> {
    let rest = input.trim_start().strip_prefix('/')?;
    (!rest.contains(char::is_whitespace)).then_some(rest)
}

/// Interpret the composer's text.
pub fn parse(input: &str, s: &Situation) -> Input {
    let text = input.trim();
    let Some(rest) = text.strip_prefix('/') else { return Input::Message(text.to_string()) };
    let (word, extra) = match rest.split_once(char::is_whitespace) {
        Some((w, e)) => (w, e.trim()),
        None => (rest, ""),
    };
    let more = if extra.is_empty() { String::new() } else { format!("\n\nMy additional request: {extra}") };
    let selection = if s.selected == 0 {
        String::new()
    } else {
        format!("\n\nSomething is selected ({} item(s)); `selection` in the design state says what.", s.selected)
    };
    let prompt = match word.to_ascii_lowercase().as_str() {
        "help" if extra.is_empty() => return Input::Help,
        "explain" => format!(
            "Explain this design: what each feature in the timeline does, how the parameters drive the shape, and what the part is for if that is \
             clear. Keep it short. If something is selected, concentrate on that.{selection}{more}"
        ),
        "fix" if s.failing.is_empty() => format!(
            "Look for problems in this design and fix what you can: features with errors or warnings, over- or under-constrained sketches, \
             suppressed or rolled back features that matter. If you find none, say so. Tell me what you changed.{selection}{more}"
        ),
        "fix" => format!(
            "Fix the failing features. These have errors or warnings:\n{}\n\nFind the cause of each (inspect the feature and the sketches it uses), \
             fix it if you can, and check the result. Tell me what you changed.{selection}{more}",
            s.failing.iter().take(20).map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
        ),
        "params" => format!(
            "Review the design's user parameters: which are unused, which numbers repeat in the features and should be parameters, and whether the \
             names are clear. Suggest improvements first; change the design only if I agree.{more}"
        ),
        "check" => format!(
            "Check this design without changing it: measure the bodies and compare with what the features should produce, look at thin walls and \
             tiny features, and report errors, warnings and sketches that are not fully constrained. Give me a short list, most important first.{selection}{more}"
        ),
        // Not ours: an agent's own slash command, or plain text that starts with a slash.
        _ => return Input::Message(text.to_string()),
    };
    Input::Prompt { shown: text.to_string(), prompt }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_and_unknown_words_are_sent_as_typed() {
        assert_eq!(parse("  make a box  ", &Situation::default()), Input::Message("make a box".into()));
        assert_eq!(parse("a / b", &Situation::default()), Input::Message("a / b".into()));
        // An agent's own command goes through untouched.
        assert_eq!(parse("/compact keep the plan", &Situation::default()), Input::Message("/compact keep the plan".into()));
        assert_eq!(parse("/", &Situation::default()), Input::Message("/".into()));
        assert_eq!(parse("/ explain", &Situation::default()), Input::Message("/ explain".into()));
    }

    #[test]
    fn quick_actions_expand_with_what_is_failing_and_selected() {
        let s = Situation { failing: vec!["Fillet 1: radius too large".into()], selected: 2 };
        let Input::Prompt { shown, prompt } = parse("/fix keep the wall thick", &s) else { panic!("not a prompt") };
        assert_eq!(shown, "/fix keep the wall thick");
        assert!(prompt.starts_with("Fix the failing features"));
        assert!(prompt.contains("- Fillet 1: radius too large") && prompt.contains("2 item(s)") && prompt.contains("keep the wall thick"));
        // With nothing failing, /fix looks for problems.
        let Input::Prompt { prompt, .. } = parse("/FIX", &Situation::default()) else { panic!("not a prompt") };
        assert!(prompt.starts_with("Look for problems"));
        for w in ["explain", "params", "check"] {
            assert!(matches!(parse(&format!("/{w}"), &Situation::default()), Input::Prompt { .. }), "{w}");
        }
    }

    #[test]
    fn help_is_ours_only_without_more_text() {
        assert_eq!(parse("/help", &Situation::default()), Input::Help);
        assert_eq!(parse("/help me with this", &Situation::default()), Input::Message("/help me with this".into()));
    }

    #[test]
    fn suggestions_follow_what_is_typed() {
        assert_eq!(typing("/ex"), Some("ex"));
        assert_eq!(typing("  /"), Some(""));
        assert_eq!(typing("/fix more"), None);
        assert_eq!(typing("fix"), None);
        assert_eq!(matching("e").iter().map(|q| q.name).collect::<Vec<_>>(), ["explain"]);
        assert_eq!(matching("").len(), QUICK.len());
        assert!(matching("zzz").is_empty());
    }

    #[test]
    fn a_long_list_of_failures_is_capped() {
        let s = Situation { failing: (0..500).map(|i| format!("F{i}: bad")).collect(), selected: 0 };
        let Input::Prompt { prompt, .. } = parse("/fix", &s) else { panic!("not a prompt") };
        assert!(prompt.contains("F19: bad") && !prompt.contains("F20: bad"));
    }
}
