use futures::stream::TryStreamExt;
use leptos::{
    component, create_effect, create_resource, mount_to_body, view, window, For, IntoView, Memo,
    Signal, SignalGet, SignalGetUntracked, SignalWithUntracked, Suspense, WriteSignal,
};
use leptos_router::{
    use_navigate, use_query_map, NavigateOptions, ParamsMap, Route, Router, Routes,
};
use leptos_use::{storage::use_local_storage, utils::JsonCodec};
use rspotify::{
    clients::{BaseClient, OAuthClient},
    scopes, AuthCodePkceSpotify, Credentials, OAuth, Token,
};

const SPOTIFY_API_ID: &'static str = "e88dbb278f734122875172d70978e455";

fn init_spotify() -> AuthCodePkceSpotify {
    AuthCodePkceSpotify::new(
        Credentials::new_pkce(SPOTIFY_API_ID),
        OAuth {
            // TODO: Should be dynamic
            redirect_uri: "http://localhost:8080/callback".to_owned(),
            scopes: scopes!("playlist-read-collaborative"),
            ..Default::default()
        },
    )
}

async fn get_token(query_map: Memo<ParamsMap>, spotify: AuthCodePkceSpotify) -> Option<Token> {
    let code = query_map.with_untracked(|querys| querys.get("code").cloned())?;
    spotify.request_token(code.as_str()).await.ok()?;

    spotify.get_token().lock().await.ok()?.clone()
}

#[component]
fn Callback(
    #[prop(into)] oauth_flow_state: Signal<OAuthFlowState>,
    #[prop(into)] spotify: Signal<AuthCodePkceSpotify>,
    set_oauth_flow: WriteSignal<OAuthFlow>,
) -> impl IntoView {
    create_resource(
        move || use_query_map(),
        move |query_map| async move {
            let navigate = use_navigate();
            match oauth_flow_state.get_untracked() {
                OAuthFlowState::RequestedUserAuthorization => {
                    let spotify = spotify.get_untracked();

                    match get_token(query_map, spotify).await {
                        Some(token) => {
                            set_oauth_flow(OAuthFlow::GotToken { token });
                            navigate("/", NavigateOptions::default())
                        }
                        None => navigate("/login", NavigateOptions::default()),
                    }
                }
                _ => navigate("/login", NavigateOptions::default()),
            }
        },
    );
}

#[component]
fn App(
    #[prop(into)] oauth_flow_state: Signal<OAuthFlowState>,
    #[prop(into)] spotify: Signal<AuthCodePkceSpotify>,
) -> impl IntoView {
    create_effect(move |_| {
        let navigate = use_navigate();
        match oauth_flow_state.get() {
            OAuthFlowState::FirstVisit => navigate("/login", NavigateOptions::default()),
            OAuthFlowState::RequestedUserAuthorization => {
                navigate("/login", NavigateOptions::default())
            }
            OAuthFlowState::GotToken => {}
        }
    });

    let playlists = create_resource(
        || (),
        move |_| async move {
            let spotify = spotify.get_untracked();
            let playlists_stream = spotify.current_user_playlists();

            let v: Vec<_> = playlists_stream.try_collect().await.unwrap();

            v
        },
    );

    view! {
        <Suspense fallback=|| view! { <h1>Loading</h1> }>
            <h1>Playlists:</h1>
            <For
                each=move || playlists.get().unwrap_or_default()
                key=|playlist| playlist.id.clone()
                let:playlist
            >
                <p>
                    {playlist.name.clone()} ": "
                    {if playlist.collaborative { "collaborative" } else { "solo" }}

                </p>
            </For>
        </Suspense>
    }
}

#[component]
fn Login(set_oauth_flow: WriteSignal<OAuthFlow>) -> impl IntoView {
    let click = move |_| {
        let mut spotify = init_spotify();

        let url = spotify.get_authorize_url(None).unwrap();
        let verifier = spotify.verifier.unwrap();

        set_oauth_flow(OAuthFlow::RequestedUserAuthorization { verifier });

        window().location().set_href(&url).ok();
    };

    view! { <button on:click=click>"Connect to spotify"</button> }
}

#[derive(Default, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone)]
enum OAuthFlow {
    #[default]
    FirstVisit,
    RequestedUserAuthorization {
        verifier: String,
    },
    GotToken {
        token: Token,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OAuthFlowState {
    FirstVisit,
    RequestedUserAuthorization,
    GotToken,
}

fn main() {
    let (oauth_flow, set_oauth_flow, _) =
        use_local_storage::<OAuthFlow, JsonCodec>("spotify_token");

    let oauth_flow_state = Signal::derive(move || match oauth_flow.get() {
        OAuthFlow::FirstVisit => OAuthFlowState::FirstVisit,
        OAuthFlow::RequestedUserAuthorization { .. } => OAuthFlowState::RequestedUserAuthorization,
        OAuthFlow::GotToken { .. } => OAuthFlowState::GotToken,
    });

    let spotify = Signal::derive(move || match oauth_flow.get() {
        OAuthFlow::FirstVisit => init_spotify(),
        OAuthFlow::RequestedUserAuthorization { verifier } => {
            let mut s = init_spotify();
            s.verifier = Some(verifier);
            s
        }
        OAuthFlow::GotToken { token } => AuthCodePkceSpotify::from_token(token),
    });

    console_error_panic_hook::set_once();

    mount_to_body(move || {
        view! {
            <div id="root">
                <Router>
                    <nav></nav>
                    <main>
                        <Routes>
                            <Route
                                path="/login"
                                view=move || {
                                    view! { <Login set_oauth_flow=set_oauth_flow/> }
                                }
                            />

                            <Route
                                path="/callback"
                                view=move || {
                                    view! {
                                        <Callback
                                            oauth_flow_state=oauth_flow_state
                                            spotify=spotify
                                            set_oauth_flow=set_oauth_flow
                                        />
                                    }
                                }
                            />

                            <Route
                                path=""
                                view=move || {
                                    view! {
                                        <App oauth_flow_state=oauth_flow_state spotify=spotify/>
                                    }
                                }
                            />

                        </Routes>
                    </main>
                </Router>
            </div>
        }
    })
}
