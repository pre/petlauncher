//! The widgets of a word page: the headword, the word classes and the
//! numbered meanings. The text comes from `markup`.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib::{self, markup_escape_text};
use gtk::pango;
use petlauncher_dict::{Entry, Found, Meaning};

use super::markup::{self, Link};
use super::page::groups;

/// Opens a word in the window: the target of a link.
pub type OpenWord = Rc<dyn Fn(&str)>;

/// The page of a word: each entry with its meanings, the typed word first
/// and then its base forms.
///
/// Each meaning is one selectable label, so the owner can select the
/// definition, the examples and the synonyms of a meaning together. A
/// selection cannot go from one label into the next.
pub fn word_page(word: &str, entries: &[Entry], open: &OpenWord) -> gtk::Box {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["dictionary-page"])
        .build();
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            page.append(
                &gtk::Separator::builder()
                    .css_classes(["dictionary-entry-separator"])
                    .build(),
            );
        }
        let headword = selectable(&markup_escape_text(&entry.word));
        headword.add_css_class(if index == 0 { "title-1" } else { "title-2" });
        headword.add_css_class("dictionary-headword");
        page.append(&headword);
        if entry.found == Found::BaseForm {
            let note = gtk::Label::builder()
                .label(format!("Base form of “{word}”"))
                .xalign(0.0)
                .css_classes(["dim-label", "dictionary-note"])
                .build();
            page.append(&note);
        }
        for group in groups(&entry.meanings) {
            page.append(&class_heading(group[0].word_class.name()));
            for (number, meaning) in group.iter().enumerate() {
                page.append(&meaning_row(number + 1, meaning, &entry.word, open));
            }
        }
    }
    page
}

fn class_heading(name: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .css_classes(["dictionary-class"])
        .build()
}

fn meaning_row(number: usize, meaning: &Meaning, headword: &str, open: &OpenWord) -> gtk::Box {
    let number = gtk::Label::builder()
        .label(number.to_string())
        .xalign(1.0)
        .yalign(0.0)
        .valign(gtk::Align::Start)
        .css_classes(["dictionary-number"])
        .build();
    let text = selectable(&markup::meaning(meaning, headword, false));
    text.add_css_class("dictionary-meaning");
    let open = Rc::clone(open);
    let meaning = meaning.clone();
    let headword = headword.to_owned();
    text.connect_activate_link(move |label, uri| {
        match markup::parse_link(uri) {
            Some(Link::Word(word)) => open(&word),
            Some(Link::More) => label.set_markup(&markup::meaning(&meaning, &headword, true)),
            None => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    let row = gtk::Box::builder()
        .css_classes(["dictionary-meaning-row"])
        .build();
    row.append(&number);
    row.append(&text);
    row
}

/// A label with markup that wraps at the width of the page and that the
/// owner can select.
pub fn selectable(markup: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(markup)
        .use_markup(true)
        .selectable(true)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .xalign(0.0)
        .hexpand(true)
        .build()
}

/// A label with links to words, for the "not found" page.
pub fn word_links(words: &[String], open: &OpenWord) -> gtk::Label {
    let label = gtk::Label::builder()
        .label(markup::word_links(words, "    "))
        .use_markup(true)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dictionary-nearest"])
        .build();
    let open = Rc::clone(open);
    label.connect_activate_link(move |_, uri| match markup::parse_link(uri) {
        Some(Link::Word(word)) => {
            open(&word);
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    label
}
