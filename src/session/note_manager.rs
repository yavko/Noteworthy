use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

use std::path::PathBuf;

use super::{note::Id, note_repository::NoteRepository, tag_list::TagList, Note, NoteList};
use crate::Result;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    tag_list: TagList,
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct NoteManager {
        pub directory: OnceCell<gio::File>,
        pub repository: OnceCell<NoteRepository>,
        pub note_list: OnceCell<NoteList>,
        pub tag_list: OnceCell<TagList>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NoteManager {
        const NAME: &'static str = "NwtyNoteManager";
        type Type = super::NoteManager;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for NoteManager {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
        }

        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpec::new_object(
                        "directory",
                        "Directory",
                        "Directory where the notes are stored",
                        gio::File::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_object(
                        "repository",
                        "Repository",
                        "Repository where the notes are stored",
                        NoteRepository::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpec::new_object(
                        "note-list",
                        "Note List",
                        "List of notes",
                        NoteList::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpec::new_object(
                        "tag-list",
                        "Tag List",
                        "List of tags",
                        TagList::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "directory" => {
                    let directory = value.get().unwrap();
                    self.directory.set(directory).unwrap();
                }
                "repository" => {
                    let repository = value.get().unwrap();
                    self.repository.set(repository).unwrap();
                }
                "note-list" => {
                    let note_list = value.get().unwrap();
                    self.note_list.set(note_list).unwrap();
                }
                "tag-list" => {
                    let tag_list = value.get().unwrap();
                    self.tag_list.set(tag_list).unwrap();
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "directory" => obj.directory().to_value(),
                "repository" => obj.repository().to_value(),
                "note-list" => obj.note_list().to_value(),
                "tag-list" => obj.tag_list().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    pub struct NoteManager(ObjectSubclass<imp::NoteManager>);
}

impl NoteManager {
    pub async fn for_directory(directory: &gio::File) -> Self {
        let repository = {
            let res =
                NoteRepository::clone("git@github.com:SeaDve/test.git".into(), directory).await;

            if let Err(err) = res {
                log::warn!("Failed to clone repo: {}", err);
                log::warn!("Opening existing instead...");
                let repo = NoteRepository::open(directory).await.unwrap();
                // TODO dont update here
                repo.update().await.unwrap();
                repo
            } else {
                res.unwrap()
            }
        };

        glib::Object::new::<Self>(&[("directory", directory), ("repository", &repository)])
            .expect("Failed to create NoteManager.")
    }

    pub fn directory(&self) -> gio::File {
        let imp = imp::NoteManager::from_instance(self);
        imp.directory.get().unwrap().clone()
    }

    pub fn repository(&self) -> NoteRepository {
        let imp = imp::NoteManager::from_instance(self);
        Clone::clone(imp.repository.get().unwrap())
    }

    pub fn note_list(&self) -> NoteList {
        let imp = imp::NoteManager::from_instance(self);
        imp.note_list
            .get()
            .expect("Please call `load_notes` first")
            .clone()
    }

    pub fn tag_list(&self) -> TagList {
        let imp = imp::NoteManager::from_instance(self);
        imp.tag_list
            .get()
            .expect("Please call `load_data_file` first")
            .clone()
    }

    async fn load_notes(&self) -> anyhow::Result<()> {
        let directory = self.directory();
        let files = directory
            .enumerate_children_async_future(
                &gio::FILE_ATTRIBUTE_STANDARD_NAME,
                gio::FileQueryInfoFlags::NONE,
                glib::PRIORITY_HIGH_IDLE,
            )
            .await?;
        let note_list = NoteList::new();

        for file in files.flatten() {
            let file_name = file.name();

            if file_name.extension().unwrap_or_default() != "md" {
                log::info!(
                    "The file {} doesn't have an md extension, skipping...",
                    file_name.display()
                );
                continue;
            }

            let mut file_path = directory.path().unwrap();
            file_path.push(file_name);

            log::info!("Loading file: {}", file_path.display());

            // TODO consider using sourcefile here
            let file = gio::File::for_path(file_path);
            let note = Note::deserialize(&file).await?;
            note_list.append(note);
        }

        self.set_property("note-list", note_list).unwrap();

        Ok(())
    }

    async fn load_data_file(&self) -> Result<()> {
        let data_file_path = self.data_file_path();
        let file = gio::File::for_path(&data_file_path);

        let data: Data = match file.load_contents_async_future().await {
            Ok((file_content, _)) => {
                log::info!(
                    "Data file found at {} is loaded successfully",
                    data_file_path.display()
                );
                serde_yaml::from_slice(&file_content).unwrap_or_default()
            }
            Err(e) => {
                log::warn!(
                    "Falling back to default data, Failed to load data file: {}",
                    e
                );
                Data::default()
            }
        };

        self.set_property("tag-list", data.tag_list).unwrap();

        Ok(())
    }

    pub async fn save_note(&self, note: Note) -> anyhow::Result<()> {
        // self.sync().await?;

        if note.is_saved() {
            log::info!("Note is already saved returning");
            return Ok(());
        }

        let note_bytes = note.serialize()?;

        note.file()
            .replace_contents_async_future(note_bytes, None, false, gio::FileCreateFlags::NONE)
            .await
            .unwrap();

        note.set_is_saved(true);

        log::info!(
            "Saved note with title of {} and path of {:?}",
            note.metadata().title(),
            note.file().path().unwrap().display()
        );

        Ok(())
    }

    pub fn save_all_notes(&self) -> Result<()> {
        for note in self.note_list().iter() {
            if note.is_saved() {
                log::info!("Note already saved, skipping...");
                continue;
            }

            let note_bytes = note.serialize()?;

            note.file().replace_contents(
                &note_bytes,
                None,
                false,
                gio::FileCreateFlags::NONE,
                None::<&gio::Cancellable>,
            )?;

            note.set_is_saved(true);

            log::info!(
                "Saved note synchronously with title of {} and path of {:?}",
                note.metadata().title(),
                note.file().path().unwrap().display()
            );
        }

        Ok(())
    }

    pub fn save_data_file(&self) -> Result<()> {
        let data = Data {
            tag_list: self.tag_list(),
        };
        let data_bytes = serde_yaml::to_vec(&data)?;

        let data_file = gio::File::for_path(self.data_file_path());
        data_file.replace_contents(
            &data_bytes,
            None,
            false,
            gio::FileCreateFlags::NONE,
            None::<&gio::Cancellable>,
        )?;

        Ok(())
    }

    pub fn create_note(&self) -> Result<()> {
        let mut file_path = self.directory().path().unwrap();
        file_path.push(Self::generate_unique_file_name());
        file_path.set_extension("md");

        let file = gio::File::for_path(&file_path);
        let new_note = Note::create_default(&file);

        self.note_list().append(new_note);

        log::info!("Created note {}", file_path.display());

        Ok(())
    }

    pub fn delete_note(&self, note_id: &Id) -> Result<()> {
        let note_list = self.note_list();
        note_list.remove(note_id);

        let note = note_list.get(note_id).unwrap();
        note.delete().unwrap();

        log::info!("Deleted note {}", note.file().path().unwrap().display());

        Ok(())
    }

    pub async fn load(&self) -> anyhow::Result<()> {
        // let repo = self.repository();

        // repo.fetch("origin".into()).await?;
        // repo.merge("origin/main".into()).await?;

        // dbg!(repo.current_branch().await);

        self.load_data_file().await?;
        self.load_notes().await?;

        Ok(())
    }

    // async fn sync(&self) -> anyhow::Result<()> {
    //     let repo = self.repository();

    //     if let Err(err) = repo.clone("git@github.com:SeaDve/test.git".into()).await {
    //         log::error!("Error cloning {}", err);
    //     }

    //     repo.fetch("origin".into()).await?;
    //     repo.merge("origin/main".into()).await?;

    //     repo.add(vec![".".into()]).await?;
    //     repo.commit("Sync commit".into()).await?;
    //     repo.push("origin".into()).await?;

    //     Ok(())
    // }

    fn data_file_path(&self) -> PathBuf {
        let mut data_file_path = self.directory().path().unwrap();
        data_file_path.push("data.nwty");
        data_file_path
    }

    fn generate_unique_file_name() -> String {
        // This is also the note's id
        chrono::Local::now()
            .format("Noteworthy-%Y-%m-%d-%H-%M-%S-%f")
            .to_string()
    }
}
