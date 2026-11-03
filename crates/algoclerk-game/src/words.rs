// SPDX-License-Identifier: Apache-2.0
//! Strings of the C++ era UI that the shared catalogue (`algoclerk-core/lang/en.toml`) does not hold yet.

/// Speech bubble when the answer depends on unordered container order.
pub const UNORDERED_BUBBLE: &str = "My answer changes with the order of the unordered shelf!";
/// Brief page 1: label of the read-only prelude.
pub const OFFICE_SUPPLIES: &str = "The office supplies (read only)";
/// Brief page 1: label of the part the player writes.
pub const YOU_WRITE: &str = "You write";
/// Brief page 3: headers the player may include.
pub const HEADERS: &str = "Headers you may include";
/// Brief page 3: headers already included by the prelude.
pub const INCLUDED: &str = "Already included";
/// One-time note after an old save was converted.
pub const MIGRATION_TITLE: &str = "The office moved";
/// Label of the OK button of the notice.
pub const GOT_IT: &str = "Got it";
/// Editor: the read-only prelude line shown above the editable text.
pub const PRELUDE_BAR: &str = "prelude";
/// Result screen: there is no accepted program to translate.
pub const NOTHING_TO_TRANSLATE: &str = "there is no program that compiles to translate yet";
/// Result screen: lead-in of the message that a tab cannot be translated.
pub const CANNOT_TRANSLATE: &str = "The clerk could not translate this solution to";
/// Result screen: the reassurance under the message.
pub const STILL_VALID: &str = "Your C++ is unchanged and the other tabs still work.";
/// Result screen: toggle to wrap long lines.
pub const WRAP: &str = "Wrap";
/// Result screen: toggle to scroll long lines sideways.
pub const SCROLL: &str = "Scroll";
/// Result screen: label before the technique note.
pub const TECHNIQUE: &str = "Technique";
/// Result screen: what the tab shows, per language.
pub const TAB_C: &str = "C11 with a small typed-container prelude, same algorithm.";
/// Result screen: what the Rust tab shows.
pub const TAB_RUST: &str = "Idiomatic Rust, same algorithm and names.";
/// Result screen: what the C++ tab shows.
pub const TAB_CPP: &str = "Your own program, formatted, with the level's includes.";
/// Autocomplete: no suggestion.
pub const NO_SUGGESTION: &str = "no suggestions";
/// Card editor: statement that cards cannot express.
pub const RAW_CARD: &str = "raw C++ (read only card)";
/// Card editor: a dragged card cannot go where it was dropped.
pub const CANT_DROP: &str = "A card cannot go there.";
/// Card editor: tab with the member functions of the value left of the cursor.
pub const MEMBERS: &str = ".fn";
/// Shown instead of an error while the answer is still untouched.
pub const NUDGE: &str = "Write your answer between the braces.";
/// Status line: the problem is described in the note beside the code.
pub const SEE_NOTE: &str = "The clerk left a red note by your code. Fix it, then press play.";
/// Status line: a run stopped with an error, which the editor note also describes.
pub const SEE_NOTE_RUN: &str = "The clerk stopped here. The note by your code says why.";
/// Card editor: the button on an empty body.
pub const ADD_STATEMENT: &str = "+ Add a statement";
/// Card editor: first-use hint under the button.
pub const FIRST_CARD_HINT: &str =
    "Tap the button, then pick a card: a variable, an if, a loop, a return.";
/// Card editor: title of the palette on an empty body.
pub const ADD_FIRST: &str = "Add your first card";
