//! The widget of one row: icon, title and subtitle.

use std::path::Path;

use gtk::prelude::*;
use gtk::{gio, pango};
use petlauncher_core::{Icon, Row, RowKind};

const ICON_SIZE: i32 = 32;

pub fn row_widget(row: &Row) -> gtk::ListBoxRow {
    let image = icon_image(&row.icon, row.kind);
    image.set_pixel_size(ICON_SIZE);
    let title = label(&row.title, pango::EllipsizeMode::End, "result-title");
    let subtitle = label(
        &row.subtitle,
        subtitle_ellipsis(row.kind),
        "result-subtitle",
    );
    subtitle.add_css_class("dim-label");
    subtitle.set_visible(!row.subtitle.is_empty());
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    text.append(&title);
    text.append(&subtitle);
    let content = gtk::Box::builder().spacing(12).build();
    content.append(&image);
    content.append(&text);
    // The query line keeps the keyboard focus, so a row never takes it.
    gtk::ListBoxRow::builder()
        .child(&content)
        .focusable(false)
        .build()
}

/// The subtitle of a file or a directory is a path: the ellipsis is in its
/// middle, so the start and the file name stay. A different subtitle is a
/// text with its ellipsis at the end.
fn subtitle_ellipsis(kind: RowKind) -> pango::EllipsizeMode {
    match kind {
        RowKind::File | RowKind::Directory => pango::EllipsizeMode::Middle,
        _ => pango::EllipsizeMode::End,
    }
}

/// A label that takes the width of the row and does not make the window
/// wider: `max_width_chars` limits its natural width, and the text is
/// shortened with an ellipsis.
fn label(text: &str, ellipsize: pango::EllipsizeMode, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .ellipsize(ellipsize)
        .max_width_chars(1)
        .hexpand(true)
        .css_classes([class])
        .build()
}

fn icon_image(icon: &Icon, kind: RowKind) -> gtk::Image {
    match icon {
        Icon::None => gtk::Image::new(),
        Icon::Name(name) => gtk::Image::from_icon_name(name),
        Icon::Path(path) => gtk::Image::from_gicon(&gio::FileIcon::new(&gio::File::for_path(path))),
        Icon::ForFile(path) => gtk::Image::from_gicon(&file_icon(path, kind)),
    }
}

/// The icon of the content type, from the file name only. The main thread
/// must not read the disk for each row.
fn file_icon(path: &Path, kind: RowKind) -> gio::Icon {
    if kind == RowKind::Directory {
        return gio::ThemedIcon::new("folder").upcast();
    }
    let (content_type, _) = gio::content_type_guess(Some(path), None);
    gio::content_type_get_icon(&content_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_cut_in_its_middle_and_a_text_at_its_end() {
        assert_eq!(
            subtitle_ellipsis(RowKind::File),
            pango::EllipsizeMode::Middle
        );
        assert_eq!(
            subtitle_ellipsis(RowKind::Directory),
            pango::EllipsizeMode::Middle
        );
        assert_eq!(
            subtitle_ellipsis(RowKind::Application),
            pango::EllipsizeMode::End
        );
        assert_eq!(
            subtitle_ellipsis(RowKind::Dictionary),
            pango::EllipsizeMode::End
        );
    }
}
