mod wrapper;

use gtk::{gio, glib, prelude::*, subclass::prelude::*};
use once_cell::{sync::Lazy, unsync::OnceCell};
use regex::Regex;

use std::thread;

static DEFAULT_AUTHOR_NAME: Lazy<String> = Lazy::new(|| String::from("NoteworthyApp"));
static DEFAULT_AUTHOR_EMAIL: Lazy<String> = Lazy::new(|| String::from("app@noteworthy.io"));

static RE_VALIDATE_URL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(git@[\w\.]+)(:(//)?)([\w\.@:/\-~]+)(\.git)(/)?").unwrap());

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Repository {
        pub base_path: OnceCell<gio::File>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Repository {
        const NAME: &'static str = "NwtyRepository";
        type Type = super::Repository;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for Repository {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "base-path",
                    "Base Path",
                    "Where the repository is stored locally",
                    gio::File::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                )]
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
                "base-path" => {
                    let base_path = value.get().unwrap();
                    self.base_path.set(base_path).unwrap();
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "base-path" => self.base_path.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
        }
    }
}

glib::wrapper! {
    pub struct Repository(ObjectSubclass<imp::Repository>);
}

impl Repository {
    pub fn new(base_path: &gio::File) -> Self {
        glib::Object::new::<Self>(&[("base-path", base_path)])
            .expect("Failed to create Repository.")
    }

    pub fn validate_remote_url(remote_url: &str) -> bool {
        if remote_url.is_empty() {
            return false;
        }

        RE_VALIDATE_URL.is_match(remote_url)
    }

    pub fn base_path(&self) -> gio::File {
        self.property("base-path").unwrap().get().unwrap()
    }

    pub async fn clone(&self, remote_url: String, passphrase: String) -> anyhow::Result<()> {
        let base_path = self.base_path().path().unwrap();

        Self::run_async(move || wrapper::clone(&base_path, &remote_url, &passphrase)).await?;

        Ok(())
    }

    pub async fn push(&self, remote_name: String, passphrase: String) -> anyhow::Result<()> {
        let base_path = self.base_path().path().unwrap();

        Self::run_async(move || wrapper::push(&base_path, &remote_name, &passphrase)).await?;

        Ok(())
    }

    pub async fn commit(&self, message: String) -> anyhow::Result<()> {
        let base_path = self.base_path().path().unwrap();

        Self::run_async(move || {
            wrapper::commit(
                &base_path,
                &message,
                &DEFAULT_AUTHOR_NAME,
                &DEFAULT_AUTHOR_EMAIL,
            )
        })
        .await?;

        Ok(())
    }

    pub async fn fetch(&self, remote_name: String, passphrase: String) -> anyhow::Result<()> {
        let base_path = self.base_path().path().unwrap();

        Self::run_async(move || wrapper::fetch(&base_path, &remote_name, &passphrase)).await?;

        Ok(())
    }

    pub async fn merge(&self, source_branch: String) -> anyhow::Result<()> {
        let base_path = self.base_path().path().unwrap();

        Self::run_async(move || {
            wrapper::merge(
                &base_path,
                &source_branch,
                &DEFAULT_AUTHOR_NAME,
                &DEFAULT_AUTHOR_EMAIL,
            )
        })
        .await?;

        Ok(())
    }

    async fn run_async<F>(f: F) -> anyhow::Result<()>
    where
        F: FnOnce() -> anyhow::Result<()> + Send + 'static,
    {
        let (sender, receiver) = futures::channel::oneshot::channel();

        thread::spawn(move || {
            let res = f();
            sender.send(res).unwrap();
        });

        receiver.await.unwrap()?;

        Ok(())
    }
}
