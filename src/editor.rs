use std::{future::Future, pin::Pin};

use crossterm::{
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    style::{Color, ContentStyle},
};
use promkit::{pane::Pane, style::StyleBuilder, text, text_editor, PaneFactory};

use crate::search::IncrementalSearcher;

pub struct Editor {
    keybind: Keybind,
    state: text_editor::State,
    focus_theme: EditorTheme,
    defocus_theme: EditorTheme,
    guide: text::State,
    searcher: IncrementalSearcher,
    keybinds: Keybinds,
}

pub struct EditorTheme {
    // Prefix for the prompt string.
    pub prefix: String,
    /// Style applied to the prompt string.
    pub prefix_style: ContentStyle,
    /// Style applied to the currently selected character.
    pub active_char_style: ContentStyle,
    /// Style applied to characters that are not currently selected.
    pub inactive_char_style: ContentStyle,
}

pub struct Keybinds {
    pub move_to_tail: KeyEvent,
    pub backward: KeyEvent,
    pub forward: KeyEvent,
    pub completion: KeyEvent,
    pub move_to_head: KeyEvent,
    pub move_to_previous_nearest: KeyEvent,
    pub move_to_next_nearest: KeyEvent,
    pub erase: KeyEvent,
    pub erase_all: KeyEvent,
    pub erase_to_previous_nearest: KeyEvent,
    pub erase_to_next_nearest: KeyEvent,
    pub search_up: KeyEvent,
}

impl Editor {
    pub fn new(
        state: text_editor::State,
        searcher: IncrementalSearcher,
        focus_theme: EditorTheme,
        defocus_theme: EditorTheme,
        keybinds: Keybinds,
    ) -> Self {
        Self {
            keybind: BOXED_EDITOR_KEYBIND,
            state,
            focus_theme,
            defocus_theme,
            guide: text::State {
                text: Default::default(),
                style: Default::default(),
            },
            searcher,
            keybinds,
        }
    }

    pub fn focus(&mut self) {
        self.state.prefix = self.focus_theme.prefix.clone();
        self.state.prefix_style = self.focus_theme.prefix_style;
        self.state.inactive_char_style = self.focus_theme.inactive_char_style;
        self.state.active_char_style = self.focus_theme.active_char_style;
    }

    pub fn defocus(&mut self) {
        self.state.prefix = self.defocus_theme.prefix.clone();
        self.state.prefix_style = self.defocus_theme.prefix_style;
        self.state.inactive_char_style = self.defocus_theme.inactive_char_style;
        self.state.active_char_style = self.defocus_theme.active_char_style;

        self.searcher.leave_search();
        self.keybind = BOXED_EDITOR_KEYBIND;

        self.guide.text = Default::default();
    }

    pub fn text(&self) -> String {
        self.state.texteditor.text_without_cursor().to_string()
    }

    pub fn create_editor_pane(&self, width: u16, height: u16) -> Pane {
        self.state.create_pane(width, height)
    }

    pub fn create_searcher_pane(&self, width: u16, height: u16) -> Pane {
        self.searcher.create_pane(width, height)
    }

    pub fn create_guide_pane(&self, width: u16, height: u16) -> Pane {
        self.guide.create_pane(width, height)
    }

    pub async fn operate(&mut self, event: &Event) -> anyhow::Result<()> {
        (self.keybind)(event, self).await
    }
}

pub type Keybind = for<'a> fn(
    &'a Event,
    &'a mut Editor,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

const BOXED_EDITOR_KEYBIND: Keybind =
    |event, editor| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(edit(event, editor))
    };
const BOXED_SEARCHER_KEYBIND: Keybind =
    |event, editor| -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(search(event, editor))
    };

pub async fn edit<'a>(event: &'a Event, editor: &'a mut Editor) -> anyhow::Result<()> {
    editor.guide.text = Default::default();

    match event {
        key if key == &Event::Key(editor.keybinds.completion) => {
            let prefix = editor.state.texteditor.text_without_cursor().to_string();
            match editor.searcher.start_search(&prefix) {
                Ok(result) => match result.head_item {
                    Some(head) => {
                        if result.load_state.loaded {
                            editor.guide.text = format!(
                                "Loaded all ({}) suggestions",
                                result.load_state.loaded_item_len
                            );
                            editor.guide.style = StyleBuilder::new().fgc(Color::Green).build();
                        } else {
                            editor.guide.text = format!(
                                "Loaded partially ({}) suggestions",
                                result.load_state.loaded_item_len
                            );
                            editor.guide.style = StyleBuilder::new().fgc(Color::Green).build();
                        }
                        editor.state.texteditor.replace(&head);
                        editor.keybind = BOXED_SEARCHER_KEYBIND;
                    }
                    None => {
                        editor.guide.text = format!("No suggestion found for '{}'", prefix);
                        editor.guide.style = StyleBuilder::new().fgc(Color::Yellow).build();
                    }
                },
                Err(e) => {
                    editor.guide.text = format!("Failed to lookup suggestions: {}", e);
                    editor.guide.style = StyleBuilder::new().fgc(Color::Yellow).build();
                }
            }
        }

        // Move cursor.
        key if key == &Event::Key(editor.keybinds.backward) => {
            editor.state.texteditor.backward();
        }
        key if key == &Event::Key(editor.keybinds.forward) => {
            editor.state.texteditor.forward();
        }
        key if key == &Event::Key(editor.keybinds.move_to_head) => {
            editor.state.texteditor.move_to_head();
        }

        key if key == &Event::Key(editor.keybinds.move_to_tail) => {
            editor.state.texteditor.move_to_tail();
        }

        // Move cursor to the nearest character.
        key if key == &Event::Key(editor.keybinds.move_to_previous_nearest) => {
            editor
                .state
                .texteditor
                .move_to_previous_nearest(&editor.state.word_break_chars);
        }

        key if key == &Event::Key(editor.keybinds.move_to_next_nearest) => {
            editor
                .state
                .texteditor
                .move_to_next_nearest(&editor.state.word_break_chars);
        }

        // Erase char(s).
        key if key == &Event::Key(editor.keybinds.erase) => {
            editor.state.texteditor.erase();
        }
        key if key == &Event::Key(editor.keybinds.erase_all) => {
            editor.state.texteditor.erase_all();
        }

        // Erase to the nearest character.
        key if key == &Event::Key(editor.keybinds.erase_to_previous_nearest) => {
            editor
                .state
                .texteditor
                .erase_to_previous_nearest(&editor.state.word_break_chars);
        }

        key if key == &Event::Key(editor.keybinds.erase_to_next_nearest) => {
            editor
                .state
                .texteditor
                .erase_to_next_nearest(&editor.state.word_break_chars);
        }

        // Input char.
        Event::Key(KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char(ch),
            modifiers: KeyModifiers::SHIFT,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => match editor.state.edit_mode {
            text_editor::Mode::Insert => editor.state.texteditor.insert(*ch),
            text_editor::Mode::Overwrite => editor.state.texteditor.overwrite(*ch),
        },

        _ => {}
    }
    Ok(())
}

pub async fn search<'a>(event: &'a Event, editor: &'a mut Editor) -> anyhow::Result<()> {
    match event {
        // TODO: Implement the search down keybinds as a collection
        Event::Key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }) => {
            editor.searcher.down_with_load();
            editor
                .state
                .texteditor
                .replace(&editor.searcher.get_current_item());
        }

        key if key == &Event::Key(editor.keybinds.search_up) => {
            editor.searcher.up();
            editor
                .state
                .texteditor
                .replace(&editor.searcher.get_current_item());
        }

        _ => {
            editor.searcher.leave_search();
            editor.keybind = BOXED_EDITOR_KEYBIND;
            return edit(event, editor).await;
        }
    }

    Ok(())
}
