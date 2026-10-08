//! Frames and notes: the parts of a patch that explain it.
//!
//! A [`Frame`] is a titled, tinted backdrop behind a group of modules, like
//! a section printed on a synth's front panel. A [`Note`] is a card of text.
//! Neither makes a sound. They're drawn under the modules, through the
//! editor's backdrop hook (see [`super::annotation_ui`]).
//!
//! Both are kept in patch space: the unzoomed points that saved node
//! positions use. A node at patch position `p` sits at editor position
//! `view_origin + p * zoom`, so frames and notes stay put while the view
//! zooms, with no rescaling and no rounding to undo.

use std::collections::{BTreeMap, BTreeSet};

use egui::{Color32, Pos2, Rect, Vec2};

use crate::dsp::ModuleCategory;
use crate::persistence::{FrameData, NoteData};

/// Names a frame or note for as long as the patch is open. Undo brings a
/// deleted one back under the same ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnotationId(u64);

/// The colour a frame is tinted with. The module categories' colours are
/// here, so a frame can wear the colour of what it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tint {
    #[default]
    Slate,
    Blue,
    Teal,
    Green,
    Amber,
    Rose,
    Violet,
    Cyan,
}

impl Tint {
    pub const ALL: [Tint; 8] = [
        Tint::Slate,
        Tint::Blue,
        Tint::Teal,
        Tint::Green,
        Tint::Amber,
        Tint::Rose,
        Tint::Violet,
        Tint::Cyan,
    ];

    /// The name a patch file stores, e.g. "blue".
    pub fn key(self) -> &'static str {
        match self {
            Tint::Slate => "slate",
            Tint::Blue => "blue",
            Tint::Teal => "teal",
            Tint::Green => "green",
            Tint::Amber => "amber",
            Tint::Rose => "rose",
            Tint::Violet => "violet",
            Tint::Cyan => "cyan",
        }
    }

    /// The tint a patch file names, or the default for a name this version
    /// doesn't know.
    pub fn from_key(key: &str) -> Self {
        Self::ALL.into_iter().find(|t| t.key() == key).unwrap_or_default()
    }

    /// The name shown in menus, e.g. "Blue".
    pub fn label(self) -> &'static str {
        match self {
            Tint::Slate => "Slate",
            Tint::Blue => "Blue",
            Tint::Teal => "Teal",
            Tint::Green => "Green",
            Tint::Amber => "Amber",
            Tint::Rose => "Rose",
            Tint::Violet => "Violet",
            Tint::Cyan => "Cyan",
        }
    }

    /// The full-strength colour. Frames draw it faint, and letter their
    /// titles in a lighter shade of it.
    pub fn color(self) -> Color32 {
        match self {
            Tint::Slate => Color32::from_rgb(148, 163, 184),
            Tint::Blue => ModuleCategory::Source.color(),
            Tint::Teal => ModuleCategory::Filter.color(),
            Tint::Green => Color32::from_rgb(102, 187, 106),
            Tint::Amber => ModuleCategory::Modulation.color(),
            Tint::Rose => Color32::from_rgb(240, 98, 146),
            Tint::Violet => Color32::from_rgb(149, 117, 205),
            Tint::Cyan => ModuleCategory::Effect.color(),
        }
    }

    /// The tint matching a module category's header.
    pub fn for_category(category: ModuleCategory) -> Self {
        match category {
            ModuleCategory::Source => Tint::Blue,
            ModuleCategory::Filter => Tint::Teal,
            ModuleCategory::Modulation => Tint::Amber,
            ModuleCategory::Effect => Tint::Cyan,
            ModuleCategory::Utility => Tint::Slate,
            ModuleCategory::Output => Tint::Violet,
        }
    }

    /// The tint for a frame around modules of these categories: the one
    /// most of them share, or slate when no category leads.
    pub fn for_contents(categories: impl IntoIterator<Item = ModuleCategory>) -> Self {
        let mut counts: Vec<(Tint, usize)> = Vec::new();
        for tint in categories.into_iter().map(Self::for_category) {
            match counts.iter_mut().find(|(t, _)| *t == tint) {
                Some((_, n)) => *n += 1,
                None => counts.push((tint, 1)),
            }
        }
        let most = counts.iter().map(|(_, n)| *n).max().unwrap_or(0);
        let mut leaders = counts.iter().filter(|(_, n)| *n == most);
        match (leaders.next(), leaders.next()) {
            (Some((tint, _)), None) => *tint,
            _ => Tint::Slate,
        }
    }
}

/// A titled backdrop behind a group of modules.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub title: String,
    /// In patch space.
    pub rect: Rect,
    pub tint: Tint,
}

/// A card of text. `**bold**` marks emphasis.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub text: String,
    /// Top-left corner, in patch space.
    pub position: Pos2,
    /// The width the text wraps at, in patch points.
    pub width: f32,
}

/// The smallest a frame can be resized to, in patch points.
pub const MIN_FRAME_SIZE: Vec2 = Vec2::new(140.0, 80.0);

/// The narrowest and widest a note can be, in patch points.
pub const NOTE_WIDTHS: std::ops::RangeInclusive<f32> = 120.0..=640.0;

/// A new note's width, in patch points.
pub const DEFAULT_NOTE_WIDTH: f32 = 240.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Frame(Frame),
    Note(Note),
}

impl Annotation {
    /// The top-left corner, in patch space.
    pub fn position(&self) -> Pos2 {
        match self {
            Annotation::Frame(frame) => frame.rect.min,
            Annotation::Note(note) => note.position,
        }
    }

    /// Moves it by `delta` patch points.
    pub fn translate(&mut self, delta: Vec2) {
        match self {
            Annotation::Frame(frame) => frame.rect = frame.rect.translate(delta),
            Annotation::Note(note) => note.position += delta,
        }
    }

    /// "frame Voice", "frame" or "note", for undo steps and the status bar.
    pub fn describe(&self) -> String {
        match self {
            Annotation::Frame(frame) if !frame.title.trim().is_empty() => format!("frame {}", frame.title.trim()),
            Annotation::Frame(_) => "frame".to_string(),
            Annotation::Note(_) => "note".to_string(),
        }
    }
}

/// A text being edited in place: a frame's title or a note's text.
#[derive(Clone, Debug)]
pub struct Editing {
    pub id: AnnotationId,
    /// The text as typed so far. It's written back when editing ends, so a
    /// whole edit is one undo step.
    pub draft: String,
    /// Select all the text when the field opens, so typing replaces it.
    pub select_all: bool,
    /// Whether it opened yet: the field takes the keyboard on its first frame.
    pub opened: bool,
}

/// The patch's frames and notes, which are selected, and which is being
/// edited.
#[derive(Default)]
pub struct Annotations {
    items: BTreeMap<AnnotationId, Annotation>,
    next_id: u64,
    pub selected: BTreeSet<AnnotationId>,
    pub editing: Option<Editing>,
    /// Where each was last drawn, on screen. Notes are as tall as their
    /// text, which is only known once it has been laid out.
    pub drawn: BTreeMap<AnnotationId, Rect>,
}

impl Annotations {
    pub fn add(&mut self, annotation: Annotation) -> AnnotationId {
        let id = AnnotationId(self.next_id);
        self.next_id += 1;
        self.items.insert(id, annotation);
        id
    }

    /// Puts one back under the ID it had, as undo does.
    pub fn restore(&mut self, id: AnnotationId, annotation: Annotation) {
        self.next_id = self.next_id.max(id.0 + 1);
        self.items.insert(id, annotation);
    }

    pub fn remove(&mut self, id: AnnotationId) -> Option<Annotation> {
        self.selected.remove(&id);
        self.drawn.remove(&id);
        if self.editing.as_ref().is_some_and(|e| e.id == id) {
            self.editing = None;
        }
        self.items.remove(&id)
    }

    pub fn get(&self, id: AnnotationId) -> Option<&Annotation> {
        self.items.get(&id)
    }

    pub fn get_mut(&mut self, id: AnnotationId) -> Option<&mut Annotation> {
        self.items.get_mut(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (AnnotationId, &Annotation)> {
        self.items.iter().map(|(&id, a)| (id, a))
    }

    pub fn items(&self) -> &BTreeMap<AnnotationId, Annotation> {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Forgets every frame and note, as a new or loaded patch does.
    pub fn clear(&mut self) {
        *self = Self { next_id: self.next_id, ..Self::default() };
    }

    /// Opens a frame's title or a note's text for editing.
    pub fn start_editing(&mut self, id: AnnotationId, select_all: bool) {
        let draft = match self.items.get(&id) {
            Some(Annotation::Frame(frame)) => frame.title.clone(),
            Some(Annotation::Note(note)) => note.text.clone(),
            None => return,
        };
        self.editing = Some(Editing { id, draft, select_all, opened: false });
    }

    /// Writes the edited text back. A note left empty is removed.
    pub fn finish_editing(&mut self) {
        let Some(editing) = self.editing.take() else { return };
        match self.items.get_mut(&editing.id) {
            Some(Annotation::Frame(frame)) => frame.title = editing.draft.trim().to_string(),
            Some(Annotation::Note(note)) => {
                let text = editing.draft.trim_end().to_string();
                if text.trim().is_empty() {
                    self.remove(editing.id);
                } else {
                    note.text = text;
                }
            }
            None => {}
        }
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// The selected frames and notes, in ID order.
    pub fn selection(&self) -> Vec<AnnotationId> {
        self.selected.iter().copied().filter(|id| self.items.contains_key(id)).collect()
    }

    /// Selects every frame and note drawn wholly inside a rectangle on screen,
    /// as dragging out a selection box does.
    pub fn select_within(&mut self, screen: Rect) {
        self.selected = self.drawn.iter().filter(|(_, rect)| screen.contains_rect(**rect)).map(|(&id, _)| id).collect();
    }

    /// Frames and notes as a patch stores them, moved by `offset` patch points.
    pub fn to_patch(&self, ids: &[AnnotationId], offset: Vec2) -> (Vec<FrameData>, Vec<NoteData>) {
        let mut frames = Vec::new();
        let mut notes = Vec::new();
        for annotation in ids.iter().filter_map(|id| self.items.get(id)) {
            match annotation {
                Annotation::Frame(frame) => {
                    let min = frame.rect.min + offset;
                    frames.push(FrameData {
                        title: frame.title.clone(),
                        position: (min.x, min.y),
                        size: (frame.rect.width(), frame.rect.height()),
                        color: frame.tint.key().to_string(),
                    });
                }
                Annotation::Note(note) => {
                    let position = note.position + offset;
                    notes.push(NoteData { text: note.text.clone(), position: (position.x, position.y), width: note.width });
                }
            }
        }
        (frames, notes)
    }

    /// Adds a patch's frames and notes, moved by `offset` patch points, and
    /// returns their IDs.
    pub fn add_from_patch(&mut self, frames: &[FrameData], notes: &[NoteData], offset: Vec2) -> Vec<AnnotationId> {
        let frames = frames.iter().map(|data| {
            let min = Pos2::new(data.position.0, data.position.1) + offset;
            let size = Vec2::new(data.size.0, data.size.1).max(MIN_FRAME_SIZE);
            Annotation::Frame(Frame { title: data.title.clone(), rect: Rect::from_min_size(min, size), tint: Tint::from_key(&data.color) })
        });
        let notes = notes.iter().map(|data| {
            Annotation::Note(Note {
                text: data.text.clone(),
                position: Pos2::new(data.position.0, data.position.1) + offset,
                width: data.width.clamp(*NOTE_WIDTHS.start(), *NOTE_WIDTHS.end()),
            })
        });
        frames.chain(notes).collect::<Vec<_>>().into_iter().map(|a| self.add(a)).collect()
    }

    /// The top-left corner of some frames and notes, in patch space.
    pub fn top_left(&self, ids: &[AnnotationId]) -> Option<Pos2> {
        ids.iter().filter_map(|id| self.items.get(id)).map(Annotation::position).reduce(|a, b| a.min(b))
    }
}

/// Where zooming the editor moves a point kept in editor (zoomed)
/// coordinates: the editor scales every node position by `scale` about the
/// middle of the view, `half_size` from its corner, while panned by `pan`.
pub fn follow_zoom(point: Vec2, scale: f32, half_size: Vec2, pan: Vec2) -> Vec2 {
    (point - half_size + pan) * scale + half_size - pan
}

/// Splits a note's text into runs, each marked bold or not: `**` toggles
/// bold. An unpaired `**` is shown as written.
pub fn emphasis_runs(text: &str) -> Vec<(&str, bool)> {
    let mut runs = Vec::new();
    let mut rest = text;
    let mut bold = false;
    while let Some(at) = rest.find("**") {
        // An opening `**` with no partner is just text
        if !bold && !rest[at + 2..].contains("**") {
            break;
        }
        if at > 0 {
            runs.push((&rest[..at], bold));
        }
        bold = !bold;
        rest = &rest[at + 2..];
    }
    if !rest.is_empty() {
        runs.push((rest, bold));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    #[test]
    fn test_emphasis_runs() {
        assert_eq!(emphasis_runs("plain"), vec![("plain", false)]);
        assert_eq!(
            emphasis_runs("turn **Resonance** up"),
            vec![("turn ", false), ("Resonance", true), (" up", false)]
        );
        assert_eq!(emphasis_runs("**all**"), vec![("all", true)]);
        // An unpaired marker stays as text
        assert_eq!(emphasis_runs("a ** b"), vec![("a ** b", false)]);
        assert_eq!(emphasis_runs("**a** and **b"), vec![("a", true), (" and **b", false)]);
    }

    #[test]
    fn test_tint_names_round_trip_and_unknown_is_default() {
        for tint in Tint::ALL {
            assert_eq!(Tint::from_key(tint.key()), tint);
        }
        assert_eq!(Tint::from_key("chartreuse"), Tint::Slate);
        assert_eq!(Tint::from_key(""), Tint::Slate);
    }

    #[test]
    fn test_frame_takes_the_colour_most_of_its_modules_share() {
        use ModuleCategory::*;
        assert_eq!(Tint::for_contents([Source, Source, Utility]), Tint::Blue);
        assert_eq!(Tint::for_contents([Modulation]), Tint::Amber);
        // No leader, no guess
        assert_eq!(Tint::for_contents([Source, Filter]), Tint::Slate);
        assert_eq!(Tint::for_contents([]), Tint::Slate);
    }

    #[test]
    fn test_patch_round_trip_with_offset() {
        let mut annotations = Annotations::default();
        let frame = annotations.add(Annotation::Frame(Frame {
            title: "Voice".into(),
            rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(300.0, 200.0)),
            tint: Tint::Blue,
        }));
        let note = annotations.add(Annotation::Note(Note { text: "Hi".into(), position: pos2(50.0, 260.0), width: 200.0 }));

        let (frames, notes) = annotations.to_patch(&[frame, note], vec2(-10.0, -20.0));
        assert_eq!(frames[0].position, (0.0, 0.0));
        assert_eq!(frames[0].color, "blue");
        assert_eq!(notes[0].position, (40.0, 240.0));

        let mut copy = Annotations::default();
        let ids = copy.add_from_patch(&frames, &notes, vec2(10.0, 20.0));
        assert_eq!(ids.len(), 2);
        assert_eq!(copy.get(ids[0]), annotations.get(frame));
        assert_eq!(copy.get(ids[1]), annotations.get(note));
    }

    #[test]
    fn test_an_emptied_note_is_removed_when_editing_ends() {
        let mut annotations = Annotations::default();
        let note = annotations.add(Annotation::Note(Note { text: "Hi".into(), position: Pos2::ZERO, width: 200.0 }));
        annotations.start_editing(note, false);
        annotations.editing.as_mut().unwrap().draft = "  \n".into();
        annotations.finish_editing();
        assert!(annotations.get(note).is_none());

        // A frame keeps going without a title
        let frame = annotations.add(Annotation::Frame(Frame {
            title: "Voice".into(),
            rect: Rect::from_min_size(Pos2::ZERO, MIN_FRAME_SIZE),
            tint: Tint::Slate,
        }));
        annotations.start_editing(frame, true);
        annotations.editing.as_mut().unwrap().draft = " Mod ".into();
        annotations.finish_editing();
        assert!(matches!(annotations.get(frame), Some(Annotation::Frame(f)) if f.title == "Mod"));
    }

    #[test]
    fn test_restored_ids_are_not_handed_out_again() {
        let mut annotations = Annotations::default();
        annotations.restore(AnnotationId(7), Annotation::Note(Note { text: "a".into(), position: Pos2::ZERO, width: 200.0 }));
        let next = annotations.add(Annotation::Note(Note { text: "b".into(), position: Pos2::ZERO, width: 200.0 }));
        assert_eq!(next, AnnotationId(8));
    }
}
