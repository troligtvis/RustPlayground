use std::collections::HashMap;
use std::io::{BufReader, Cursor as IoCursor};
use std::ops::Range;

use syntect::highlighting::{Highlighter, Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, ScopedMetadata, SyntaxSet};
use xi_core_lib::selection::Selection;
use xi_rope::spans::{Spans, SpansBuilder};
use xi_rope::{Cursor, LinesMetric, Rope};

use crate::style::{Style, StyleId};

//const DEFAULT_THEME: &str = "../assets/InspiredGitHub.tmTheme";

pub(crate) struct HighlightState {
    syntax_set: SyntaxSet,
    theme: Theme,
    state: Internal,
}

#[derive(Debug, Default, Clone)]
struct Internal {
    parse_state: Vec<ScopeStack>,
    style_table: HashMap<Style, StyleId>,
    new_styles: Option<Vec<(StyleId, Style)>>,
    next_style_id: StyleId,
}

impl HighlightState {
    pub(crate) fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_nonewlines();
        let theme_data = include_str!("../assets/InspiredGitHub.tmTheme");
        let mut reader = BufReader::new(IoCursor::new(theme_data));
        let theme = ThemeSet::load_from_reader(&mut reader).expect("failed to load default theme");

        HighlightState { syntax_set, theme, state: Internal::default() }
    }

    pub(crate) fn highlight_all(&mut self, text: &Rope) -> Spans<StyleId> {
        let HighlightState { syntax_set, theme, state } = self;
        state.highlight_all(text, syntax_set, &theme)
    }

    /// Returns any newly defined styles. These should be sent to the client.
    pub(crate) fn take_new_styles(&mut self) -> Option<Vec<(StyleId, Style)>> {
        self.state.new_styles.take()
    }

    pub(crate) fn metadata_for_line(&self, line: usize) -> Option<ScopedMetadata> {
        let scope = &self.state.parse_state.get(line)?;
        Some(self.syntax_set.metadata().metadata_for_scope(scope.as_slice()))
    }
}

impl Internal {
    pub(crate) fn highlight_all(
        &mut self,
        text: &Rope,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Spans<StyleId> {
        self.parse_state.clear();

        let syntax = syntax_set.find_syntax_by_name("Rust").expect("no syntax 'rust' found");
        let highlighter = Highlighter::new(theme);

        let mut b = SpansBuilder::new(text.len());
        let mut parse_state = ParseState::new(syntax);
        let mut scope_state = ScopeStack::new();
        let mut cursor = Cursor::new(text, 0);
        let mut total_offset = 0;

        while total_offset < text.len() {
            let next_break = cursor.next::<LinesMetric>().unwrap_or(text.len());
            let line = text.slice_to_cow(total_offset..next_break);
            let mut last_pos = 0;
            let ops = parse_state.parse_line(line.trim_end_matches('\n'), syntax_set);
            for (pos, batch) in ops {
                if !scope_state.is_empty() {
                    let start = total_offset + last_pos;
                    let end = start + (pos - last_pos);
                    if start != end {
                        let style = highlighter.style_for_stack(scope_state.as_slice());
                        let id = self.id_for_style(style);
                        b.add_span(start..end, id);
                    }
                }
                last_pos = pos;
                scope_state.apply(&batch);
            }
            // add EOL span:
            let start = total_offset + last_pos;
            let end = start + (line.len() - last_pos);
            if start != end {
                let style = highlighter.style_for_stack(scope_state.as_slice());
                let id = self.id_for_style(style);
                b.add_span(start..end, id);
            }
            total_offset += line.len();
            self.parse_state.push(scope_state.clone());
        }
        b.build()
    }

    fn id_for_style(&mut self, style: SyntectStyle) -> StyleId {
        use std::collections::hash_map::Entry;

        let style = Style::from(style);
        match self.style_table.entry(style) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let value = self.next_style_id;
                self.next_style_id += 1;
                entry.insert(value);
                self.new_styles.get_or_insert(Vec::new()).push((value, style));
                value
            }
        }
    }
}

//TODO: no idea where this should go
//TODO: rewrite using std::iter::from_fn?
/// Returns the set of line ranges that include a selection. (Lines that include
/// multiple selections are only included once.)
pub(crate) fn lines_for_selection(text: &Rope, sel: &Selection) -> Vec<(Range<usize>)> {
    let mut prev_range: Option<Range<usize>> = None;
    let mut line_ranges = Vec::new();
    // we send selection state to syntect in the form of a vec of line ranges,
    // so we combine overlapping selections to get the minimum set of ranges.
    for region in sel.iter() {
        let start = text.line_of_offset(region.min());
        let end = text.line_of_offset(region.max()) + 1;
        let line_range = start..end;
        let prev = prev_range.take();
        match (prev, line_range) {
            (None, range) => prev_range = Some(range),
            (Some(ref prev), ref range) if range.start <= prev.end => {
                let combined =
                    Range { start: prev.start.min(range.start), end: prev.end.max(range.end) };
                prev_range = Some(combined);
            }
            (Some(prev), range) => {
                line_ranges.push(prev);
                prev_range = Some(range);
            }
        }
    }

    if let Some(prev) = prev_range {
        line_ranges.push(prev);
    }

    line_ranges
}
