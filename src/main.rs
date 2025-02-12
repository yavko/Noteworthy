#![allow(clippy::new_without_default)]
#![warn(clippy::doc_markdown)]
#![warn(clippy::or_fun_call)]
#![warn(clippy::unused_self)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::explicit_iter_loop)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![warn(clippy::match_wildcard_for_single_variants)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::await_holding_refcell_ref)]
#![warn(clippy::map_unwrap_or)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::struct_excessive_bools)]
#![warn(clippy::trivially_copy_pass_by_ref)]
#![warn(clippy::option_if_let_else)]
#![warn(clippy::unreadable_literal)]
#![warn(clippy::if_not_else)]
#![warn(clippy::doc_markdown)]

mod application;
mod config;
mod core;
mod model;
mod session;
mod setup;
mod utils;
mod widgets;
mod window;

use gettextrs::{gettext, LocaleCategory};
use gtk::{gio, glib};
use once_cell::sync::Lazy;

use self::application::Application;
use self::config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};

static THREAD_POOL: Lazy<glib::ThreadPool> =
    Lazy::new(|| glib::ThreadPool::shared(None).expect("Unable to create thread pool"));

fn main() {
    pretty_env_logger::init_timed();

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain(GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    glib::set_application_name(&gettext("Noteworthy"));

    gst::init().expect("Unable to start GStreamer");

    gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    let app = Application::new();
    app.run();
}
