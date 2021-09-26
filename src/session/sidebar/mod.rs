mod note_row;
mod view_switcher;

use gtk::{
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
    CompositeTemplate,
};
use once_cell::unsync::OnceCell;

use std::cell::{Cell, RefCell};

use self::{
    note_row::NoteRow,
    view_switcher::{ItemKind, ViewSwitcher},
};
use super::{tag_list::TagList, Note, NoteList, Session};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Noteworthy/ui/sidebar.ui")]
    pub struct Sidebar {
        #[template_child]
        pub listview: TemplateChild<gtk::ListView>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub view_switcher: TemplateChild<ViewSwitcher>,

        pub compact: Cell<bool>,
        pub selected_note: RefCell<Option<Note>>,

        pub session: OnceCell<Session>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "NwtySidebar";
        type Type = super::Sidebar;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            ViewSwitcher::static_type();
            NoteRow::static_type();
            Self::bind_template(klass);

            klass.install_action("sidebar.create-note", None, move |obj, _, _| {
                // FIXME more proper way to create note
                let imp = imp::Sidebar::from_instance(obj);
                imp.session
                    .get()
                    .unwrap()
                    .note_manager()
                    .create_note()
                    .expect("Failed to create note");
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Sidebar {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let listview_expression = gtk::ConstantExpression::new(&self.listview.get());
            let model_expression = gtk::PropertyExpression::new(
                gtk::ListView::static_type(),
                Some(&listview_expression),
                "model",
            );
            let model_is_some_expression = gtk::ClosureExpression::new(
                |args| {
                    let model: Option<gtk::SelectionModel> = args[1].get().unwrap();

                    if model.is_some() {
                        "filled-view"
                    } else {
                        "empty-view"
                    }
                },
                &[model_expression.upcast()],
            );
            model_is_some_expression.bind(
                &self.stack.get(),
                "visible-child-name",
                None::<&gtk::Widget>,
            );
        }

        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_boolean(
                        "compact",
                        "Compact",
                        "Whether it is compact view mode",
                        false,
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "note-list",
                        "Note List",
                        "Note list represented by self",
                        NoteList::static_type(),
                        glib::ParamFlags::WRITABLE,
                    ),
                    glib::ParamSpec::new_object(
                        "selected-note",
                        "Selected Note",
                        "The selected note in this sidebar",
                        Note::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "compact" => {
                    let compact = value.get().unwrap();
                    self.compact.set(compact);
                }
                "note-list" => {
                    let note_list = value.get().unwrap();
                    obj.set_note_list(note_list);
                }
                "selected-note" => {
                    let selected_note = value.get().unwrap();
                    obj.set_selected_note(selected_note);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "selected-note" => obj.selected_note().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Sidebar {}
    impl BoxImpl for Sidebar {}
}

glib::wrapper! {
    pub struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends gtk::Widget, gtk::Box;
}

impl Sidebar {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Sidebar.")
    }

    pub fn set_note_list(&self, note_list: NoteList) {
        let imp = imp::Sidebar::from_instance(self);

        let sorter = gtk::CustomSorter::new(move |obj1, obj2| {
            let note_1 = obj1.downcast_ref::<Note>().unwrap().metadata();
            let note_2 = obj2.downcast_ref::<Note>().unwrap().metadata();

            // Sort is pinned first before classifying by last modified
            if note_1.is_pinned() == note_2.is_pinned() {
                note_2.last_modified().cmp(&note_1.last_modified()).into()
            } else if note_1.is_pinned() && !note_2.is_pinned() {
                gtk::Ordering::Smaller
            } else {
                gtk::Ordering::Larger
            }
        });
        let sorter_model = gtk::SortListModel::new(Some(&note_list), Some(&sorter));

        let filter_expression = gtk::ClosureExpression::new(
            clone!(@weak self as obj => @default-return true, move |value| {
                let note = value[0].get::<Note>().unwrap().metadata();
                let imp = imp::Sidebar::from_instance(&obj);

                match imp.view_switcher.selected_type() {
                    ItemKind::AllNotes => !note.is_trashed(),
                    ItemKind::Trash => note.is_trashed(),
                    ItemKind::Tag(ref tag) => {
                        note.tag_list().contains(tag) && !note.is_trashed()
                    }
                    ItemKind::Separator | ItemKind::Category | ItemKind::EditTags => {
                        // FIXME handle this inside view_switcher
                        log::warn!("Trying to select an unselectable row");
                        imp.view_switcher.set_selected_item_to_default();
                        true
                    }
                }
            }),
            &[],
        );
        let filter = gtk::BoolFilterBuilder::new()
            .expression(&filter_expression)
            .build();
        let filter_model = gtk::FilterListModel::new(Some(&sorter_model), Some(&filter));

        imp.view_switcher.connect_selected_type_notify(move |_, _| {
            filter.changed(gtk::FilterChange::Different);
        });

        let selection = gtk::SingleSelection::new(Some(&filter_model));
        selection
            .bind_property("selected-item", self, "selected-note")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.listview.set_model(Some(&selection));
    }

    pub fn set_selected_note(&self, selected_note: Option<Note>) {
        if self.selected_note() == selected_note {
            return;
        }

        let imp = imp::Sidebar::from_instance(self);
        imp.selected_note.replace(selected_note);
        self.notify("selected-note");
    }

    pub fn selected_note(&self) -> Option<Note> {
        let imp = imp::Sidebar::from_instance(self);
        imp.selected_note.borrow().clone()
    }

    pub fn set_tag_list(&self, tag_list: TagList) {
        let imp = imp::Sidebar::from_instance(self);
        imp.view_switcher.set_tag_list(tag_list);
    }

    // TODO remove this in the future
    pub fn set_session(&self, session: Session) {
        let imp = imp::Sidebar::from_instance(self);
        imp.session.set(session).unwrap();
    }
}
