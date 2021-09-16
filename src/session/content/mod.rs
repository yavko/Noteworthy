mod view;

use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use std::cell::{Cell, RefCell};

use self::view::View;
use super::Note;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Noteworthy/ui/content.ui")]
    pub struct Content {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub view: TemplateChild<View>,
        #[template_child]
        pub no_selected_view: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub is_pinned_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub is_trashed_button: TemplateChild<gtk::ToggleButton>,

        pub bindings: RefCell<Vec<glib::Binding>>,

        pub compact: Cell<bool>,
        pub note: RefCell<Option<Note>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Content {
        const NAME: &'static str = "NwtyContent";
        type Type = super::Content;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            View::static_type();
            Self::bind_template(klass);

            klass.install_action("content.go-back", None, move |obj, _, _| {
                obj.set_note(None);
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Content {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let self_expression = gtk::ConstantExpression::new(&obj);
            let note_expression = gtk::PropertyExpression::new(
                Self::Type::static_type(),
                Some(&self_expression),
                "note",
            );
            let is_some_note_expression = gtk::ClosureExpression::new(
                |args| {
                    let note: Option<Note> = args[1].get().unwrap();
                    note.is_some()
                },
                &[note_expression.upcast()],
            );
            is_some_note_expression.bind(&self.is_pinned_button.get(), "visible", None);
            is_some_note_expression.bind(&self.is_trashed_button.get(), "visible", None);
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
                        "note",
                        "Note",
                        "Current note in the view",
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
                "note" => {
                    let note = value.get().unwrap();
                    obj.set_note(note);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "compact" => self.compact.get().to_value(),
                "note" => obj.note().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for Content {}
    impl BoxImpl for Content {}
}

glib::wrapper! {
    pub struct Content(ObjectSubclass<imp::Content>)
        @extends gtk::Widget, gtk::Box;
}

impl Content {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Content.")
    }

    pub fn note(&self) -> Option<Note> {
        let imp = imp::Content::from_instance(self);
        imp.note.borrow().clone()
    }

    pub fn set_note(&self, note: Option<Note>) {
        if self.note() == note {
            return;
        }

        let imp = imp::Content::from_instance(self);

        for binding in imp.bindings.borrow_mut().drain(..) {
            binding.unbind();
        }

        if let Some(ref note) = note {
            let mut bindings = imp.bindings.borrow_mut();
            let note_metadata = note.metadata();

            let is_pinned = note_metadata
                .bind_property("is-pinned", &imp.is_pinned_button.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build()
                .unwrap();
            bindings.push(is_pinned);

            let is_trashed = note_metadata
                .bind_property("is-trashed", &imp.is_trashed_button.get(), "active")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build()
                .unwrap();
            bindings.push(is_trashed);

            imp.stack.set_visible_child(&imp.view.get());
        } else {
            imp.stack.set_visible_child(&imp.no_selected_view.get());
        }

        imp.note.replace(note);
        self.notify("note");
    }
}
