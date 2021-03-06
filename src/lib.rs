// (Lines like the one below ignore selected Clippy rules
//  - it's useful when you want to check your code with `cargo make verify`
// but some rules are too "annoying" or are not applicable for your case.)
#![allow(clippy::wildcard_imports)]

use seed::{prelude::*, *};
use common::{Member, Division};
use common::search::{SimSearch, SearchOptions};
use log::Level;
use crate::ui::Page;
use lazy_static::lazy_static;
use regex::Regex;
use crate::i18n::LangExt;
use common::Lang;
use i18n_embed::LanguageLoader;

macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LOADER, $message_id, $($args), *)
    }};
}

mod api;
mod ui;
mod i18n;
mod nav;

lazy_static! {
    static ref POSTAL_CODE_RE: Regex = Regex::new(r#"[a-zA-Z][0-9][a-zA-Z]\s?[0-9][a-zA-Z][0-9]"#).unwrap();
}

// ------ ------
//     Init
// ------ ------

// `init` describes what should happen when your app started.
fn init(mut url: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders.subscribe(Msg::UrlChanged);
    orders.perform_cmd(async {
        let data = Request::new("/data/members")
            .method(Method::Get)
            .fetch().await.unwrap()
            .json::<Vec<Member>>()
            .await.unwrap();
        Msg::MembersFetched(data)
    });
    orders.perform_cmd(async {
        let data = Request::new("/data/divisions")
            .method(Method::Get)
            .fetch().await.unwrap()
            .json::<Vec<Division>>()
            .await.unwrap();
        Msg::DivisionsFetched(data)
    });

    let (lang, page) = nav::decode_url(url);
    i18n::LOADER.load_languages(&i18n::Localizations, &[&lang.to_language_identifier()]);

    Model {
        members_search: SimSearch::new_with(SearchOptions::new().case_sensitive(false).threshold(0.6)),
        members: vec![],
        display_members: None,
        query: "".to_string(),
        divisions: vec![],
        display_divisions: None,
        current_page: page,
        lang,
        rdy: false,
        searching: false,
        navbar_active: false,
        displaying_search_error: false,
    }
}

// ------ ------
//     Model
// ------ ------

// `Model` describes our app state.
pub struct Model {
    members_search: SimSearch<Member>,
    members: Vec<Member>,
    display_members: Option<Vec<Member>>,
    query: String,
    divisions: Vec<Division>,
    display_divisions: Option<Vec<Division>>,
    current_page: Page,
    rdy: bool,
    searching: bool,
    navbar_active: bool,
    displaying_search_error: bool,
    lang: Lang,
}

// ------ ------
//    Update
// ------ ------

// `Msg` describes the different events you can modify state with.
pub enum Msg {
    MembersFetched(Vec<Member>),
    DivisionsFetched(Vec<Division>),
    UrlChanged(subs::UrlChanged),
    QueryChanged(String),
    ChangeLang(Lang),
    MemberSearchComplete(Vec<Member>),
    DivisionSearchComplete(Vec<Division>),
    DismissError,
    Submit,
    NavbarClick
}

// `update` describes how to handle each `Msg`.
fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    match msg {
        Msg::NavbarClick => model.navbar_active = !model.navbar_active,
        Msg::ChangeLang(lang) => {
            model.lang = lang;
            i18n::LOADER.load_languages(&i18n::Localizations, &[&model.lang.to_language_identifier()]);
            Url::go_and_load_with_str(model.current_page.to_link(&model.lang));
        }
        Msg::Submit => {
            model.searching = true;
            let query = model.query.clone();

            match model.current_page {
                Page::MppList => {
                    let search = model.members_search.clone();
                    let members = model.members.clone();
                    orders.perform_cmd(async move {
                        if POSTAL_CODE_RE.is_match(query.trim()) {
                            use gloo_timers::future::TimeoutFuture;
                            let _ = TimeoutFuture::new(50).await;
                            let result = api::lookup_postal_code(query.trim()).await;
                            Msg::MemberSearchComplete(members.into_iter().filter(|m| m.riding == result).collect())
                        } else {
                            cmds::timeout(50, move || Msg::MemberSearchComplete(search.search(&query))).await
                        }
                    });
                }
                Page::VoteList => {
                    orders.perform_cmd(async move {
                        use gloo_timers::future::TimeoutFuture;
                        let _ = TimeoutFuture::new(50).await;
                        let response = Request::new(&format!("/api/search?query={}", query))
                            .method(Method::Get)
                            .fetch()
                            .await.unwrap()
                            .json::<Vec<Division>>()
                            .await.unwrap();
                        Msg::DivisionSearchComplete(response)
                    });
                }
                _ => unreachable!()
            }
        },
        Msg::QueryChanged(query) => model.query = query,
        Msg::UrlChanged(subs::UrlChanged(url)) => {
            let (new_lang, new_page) = nav::decode_url(url);
            web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0);
            match (&model.current_page, &new_page) {
                (&Page::MppList, &Page::Mpp(_)) | (&Page::Mpp(_), &Page::MppList) => {},
                (&Page::VoteList, &Page::Vote(_)) | (&Page::Vote(_), &Page::VoteList) => {},
                _ => {
                    model.display_members = None;
                    model.query = "".to_string();
                }
            }
            model.navbar_active = false;
            model.current_page = new_page;
            if model.lang != new_lang {
                i18n::LOADER.load_languages(&i18n::Localizations, &[&new_lang.to_language_identifier()]);
                model.lang = new_lang;
            }
        }
        Msg::MembersFetched(members) => {
            model.members = members;
            let m = model.members.clone();
            for member in m {
                let toks = &[member.full_name.as_str(), member.riding.as_str(), member.party.as_str(&model.lang)];
                let member = member.clone();
                model.members_search.insert_tokens(member, toks);
            }
        }
        Msg::DivisionsFetched(divisions) => {
            model.divisions = divisions;
            model.rdy = true;
        }
        Msg::MemberSearchComplete(result) => {
            log::info!("{:?}", result);
            if result.is_empty() {
                if !model.query.is_empty() {
                    model.displaying_search_error = true;
                    orders.perform_cmd(cmds::timeout(1500, || Msg::DismissError));
                }
                model.display_members = None;
            } else {
                model.display_members = Some(result);
            }
            model.searching = false;
        }
        Msg::DivisionSearchComplete(result) => {
            if result.is_empty() {
                if !model.query.is_empty() {
                    model.displaying_search_error = true;
                    orders.perform_cmd(cmds::timeout(1500, || Msg::DismissError));
                }
                model.display_divisions = None;
            } else {
                model.display_divisions = Some(result);
            }
            model.searching = false;
        }
        Msg::DismissError => model.displaying_search_error = false,
    }
}

// ------ ------
//     View
// ------ ------

// `view` describes what to display.
fn view(model: &Model) -> Node<Msg> {
    if model.rdy {
        div![C![IF!(model.current_page == Page::Home => "root")],
            ui::navbar(model),
            ui::page(model),
            ui::footer::content(),
        ]
    } else {
        div![C!["root"],
            ui::navbar(model),
            div![C!["container"],
                h2!["Loading..."],
            ],
            ui::footer::content(),
        ]
    }
}

// ------ ------
//     Start
// ------ ------

// (This function is invoked by `init` function in `index.html`.)
#[wasm_bindgen(start)]
pub fn start() {
    console_log::init_with_level(Level::Info).unwrap();
    log::info!("Starting app...");
    // Mount the `app` to the element with the `id` "app".
    App::start("app", init, update, view);
}
