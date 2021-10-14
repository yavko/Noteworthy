use adw::subclass::prelude::*;
use gtk::{gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use std::cell::RefCell;

use super::{item_list::ItemList, Item, ItemKind, ItemRow};
use crate::model::{Tag, TagList};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/seadve/Noteworthy/ui/sidebar-view-switcher-popover.ui")]
    pub struct Popover {
        #[template_child]
        pub listview: TemplateChild<gtk::ListView>,

        pub selected_item: RefCell<Option<Item>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Popover {
        const NAME: &'static str = "NwtySidebarViewSwitcherPopover";
        type Type = super::Popover;
        type ParentType = gtk::Popover;

        fn class_init(klass: &mut Self::Class) {
            ItemRow::static_type();
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Popover {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "selected-item",
                        "Selected-item",
                        "The selected item in popover",
                        Item::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "tag-list",
                        "Tag List",
                        "The tag list in the view switcher",
                        TagList::static_type(),
                        glib::ParamFlags::WRITABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "selected-item" => {
                    let selected_item = value.get().unwrap();
                    self.selected_item.replace(selected_item);
                }
                "tag-list" => {
                    let tag_list = value.get().unwrap();
                    obj.set_tag_list(tag_list);
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "selected-item" => self.selected_item.borrow().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, list_item| {
                let item_row = ItemRow::new();

                let list_item_expression = gtk::ConstantExpression::new(list_item);

                let tree_list_row_expression = gtk::PropertyExpression::new(
                    gtk::ListItem::static_type(),
                    Some(&list_item_expression),
                    "item",
                );
                tree_list_row_expression.bind(&item_row, "list-row", None::<&gtk::Widget>);

                let selected_expression = gtk::PropertyExpression::new(
                    gtk::ListItem::static_type(),
                    Some(&list_item_expression),
                    "selected",
                );
                selected_expression.bind(&item_row, "selected", None::<&gtk::Widget>);

                list_item.set_child(Some(&item_row));
            });

            factory.connect_bind(|_, list_item| {
                let item: Option<Item> = list_item
                    .item()
                    .unwrap()
                    .downcast::<gtk::TreeListRow>()
                    .unwrap()
                    .item()
                    .and_then(|o| o.downcast().ok());

                if let Some(item) = item {
                    match item.item_kind() {
                        ItemKind::Separator | ItemKind::Category | ItemKind::EditTags => {
                            list_item.set_selectable(false);
                        }
                        _ => (),
                    }
                }
            });

            self.listview.set_factory(Some(&factory));

            // FIXME popdown this popover when something is clicked
        }
    }

    impl WidgetImpl for Popover {}
    impl PopoverImpl for Popover {}
}

glib::wrapper! {
    pub struct Popover(ObjectSubclass<imp::Popover>)
        @extends gtk::Widget, gtk::Popover,
        @implements gtk::Accessible;
}

impl Popover {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Popover.")
    }

    pub fn selected_item(&self) -> Option<Item> {
        self.property("selected-item").unwrap().get().unwrap()
    }

    pub fn select_item(&self, index: u32) {
        let imp = imp::Popover::from_instance(self);
        let model = imp.listview.model().unwrap();
        model.select_item(index, true);
    }

    pub fn set_tag_list(&self, tag_list: TagList) {
        let imp = imp::Popover::from_instance(self);

        let item_list = ItemList::new(&tag_list);
        let tree_model = gtk::TreeListModel::new(&item_list, false, true, |item| {
            item.clone().downcast::<gio::ListModel>().ok()
        });

        let selection_model = gtk::SingleSelection::new(Some(&tree_model));
        selection_model
            .bind_property("selected-item", self, "selected-item")
            .transform_to(|_, value| {
                let selected_item: Option<Item> = value
                    .get::<Option<glib::Object>>()
                    .unwrap()
                    .map(|o| o.downcast::<gtk::TreeListRow>().unwrap())
                    .map(|tlr| tlr.item().unwrap())
                    .map(|si| {
                        if let Some(item) = si.downcast_ref::<Item>() {
                            item.clone()
                        } else if let Some(tag) = si.downcast_ref::<Tag>() {
                            let item = Item::new(ItemKind::Tag(tag.clone()), None, None);
                            tag.bind_property("name", &item, "display-name")
                                .flags(glib::BindingFlags::SYNC_CREATE)
                                .build();
                            item
                        } else {
                            panic!("Wrong row item: {:?}", si);
                        }
                    });
                Some(selected_item.to_value())
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.listview.set_model(Some(&selection_model));

        self.notify("tag-list");
    }
}
